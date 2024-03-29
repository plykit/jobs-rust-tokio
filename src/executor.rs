use crate::error::{Error, Result};
use crate::job::JobData;
use crate::repos::{LockStatus, Repo};
use crate::{Job, JobConfig, JobName};
use chrono::Utc;
use log::{error, info, trace};
use std::fmt::{Debug, Formatter};
use tokio::sync::oneshot::Receiver;
use tokio::time::{sleep, Duration};

struct Shared<R> {
    instance: String,
    name: JobName,
    repo: R,
    cancel: Receiver<()>,
    action: Box<dyn Job + Send>,
}

enum Executor<R: Repo> {
    Initial(Shared<R>, JobData, Duration),
    Sleeping(Shared<R>, Duration),
    Start(Shared<R>, JobData),
    CheckDue(Shared<R>, Duration),
    TryLock(Shared<R>, Duration),
    Run(Shared<R>, JobData, R::Lock),
    Done,
}

impl<R: Repo> Debug for Executor<R> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Executor::Initial(..) => f.write_str("--------------------------- initial"),
            Executor::Sleeping(_, delay) => f.write_str(
                format!("--------------------------- sleeping {}s", delay.as_secs()).as_str(),
            ),
            Executor::Start(..) => f.write_str("------------------------------------ start"),
            Executor::TryLock(..) => f.write_str("------------------------------------ trylock"),
            Executor::CheckDue(..) => f.write_str("------------------------------------ CheckDue"),
            Executor::Run(..) => f.write_str("------------------------------------ run"),
            Executor::Done => f.write_str("------------------------------------ done"),
        }
    }
}

pub(crate) async fn run<J: Repo + Clone + Send>(
    instance: String,
    config: JobConfig,
    action: Box<dyn Job + Send>,
    repo: J,
    cancel: Receiver<()>,
    delay: Duration,
) -> Result<()> {
    let mut executor = Executor::Initial(
        Shared {
            instance,
            name: config.name.clone(),
            repo,
            cancel,
            action,
        },
        JobData::from(config),
        delay,
    );
    loop {
        trace!("loop {:?}", executor);
        executor = match executor {
            Executor::Initial(shared, jdata, delay) => on_initial(shared, jdata, delay).await,
            Executor::Start(shared, jdata) => on_start(shared, jdata).await,
            Executor::Sleeping(shared, delay) => on_sleeping(shared, delay).await,
            Executor::CheckDue(shared, delay) => on_check_due(shared, delay).await,
            Executor::TryLock(shared, delay) => on_try_lock(shared, delay).await,
            Executor::Run(shared, jdata, lock) => on_run(shared, jdata, lock).await,
            Executor::Done => return Ok(()),
        }
    }
}

async fn on_initial<R: Repo>(shared: Shared<R>, jdata: JobData, delay: Duration) -> Executor<R> {
    sleep(delay).await;
    Executor::Start(shared, jdata)
}

async fn on_sleeping<R: Repo>(mut shared: Shared<R>, delay: Duration) -> Executor<R> {
    let done = tokio::select! {
        _ = sleep(delay) =>  false,
        _ = &mut shared.cancel => true
    };

    if done {
        Executor::Done
    } else {
        Executor::CheckDue(shared, delay)
    }
}

async fn on_start<R: Repo>(mut shared: Shared<R>, jdata: JobData) -> Executor<R> {
    match shared.repo.get(jdata.name.clone().into()).await {
        Err(e) => {
            error!("get job data: {:?}", e);
            Executor::Initial(shared, jdata, Duration::from_secs(1)) // TODO Backoff
        }
        Ok(None) => {
            match shared.repo.create(jdata.clone()).await {
                Err(e) => {
                    error!("create job data: {:?}", e);
                    Executor::Initial(shared, jdata, Duration::from_secs(1)) // TODO Backoff
                }
                Ok(()) => Executor::TryLock(shared, jdata.check_interval),
            }
        }
        Ok(Some(jdata)) if jdata.due(Utc::now()) => Executor::TryLock(shared, jdata.check_interval),
        Ok(Some(jdata)) => Executor::Sleeping(shared, jdata.check_interval),
    }
}

async fn on_check_due<R: Repo>(mut shared: Shared<R>, delay: Duration) -> Executor<R> {
    match shared.repo.get(shared.name.clone()).await {
        // TODO split these two cases for clarity
        Err(_) | Ok(None) => Executor::Sleeping(shared, delay), // TODO Retry interval, attempt counter, bbackoff },
        Ok(Some(jdata)) if jdata.due(Utc::now()) => Executor::TryLock(shared, jdata.check_interval),
        Ok(Some(_)) => Executor::Sleeping(shared, delay),
    }
}
async fn on_try_lock<R: Repo>(mut shared: Shared<R>, delay: Duration) -> Executor<R> {
    match shared
        .repo
        .lock(
            shared.name.clone(),
            shared.instance.clone(),
            Duration::from_secs(10),
        )
        .await
    {
        Err(_) => Executor::Sleeping(shared, delay), // TODO Retry interval, attempt counter, bbackoff },
        Ok(LockStatus::AlreadyLocked) => Executor::Sleeping(shared, delay),
        Ok(LockStatus::Acquired(jdata, lock)) if jdata.due(Utc::now()) => {
            Executor::Run(shared, jdata, lock)
        }
        Ok(LockStatus::Acquired(jdata, _)) => {
            // We hold the lock but job is not due, so we call save with existing data to
            // release the lock. Since we do a get lock and due check before even going to
            // TryLock, this is an edge case only and nt the normal mode of operation.
            // Usually the job shoud be due when we reach TryLock.
            match shared
                .repo
                .save(jdata.name, jdata.last_run, jdata.state)
                .await
            {
                Ok(()) => Executor::Sleeping(shared, delay),
                Err(e) => {
                    error!("unlock failed in try-lock-but-not-due edge case: {:?}", e);
                    Executor::Sleeping(shared, delay)
                }
            }
        }
    }
}
async fn on_run<R: Repo>(mut shared: Shared<R>, jdata: JobData, lock: R::Lock) -> Executor<R> {
    if !jdata.due(Utc::now()) {
        return Executor::Sleeping(shared, jdata.check_interval);
    }

    let job_fut = shared.action.call(jdata.state);
    let select_result = tokio::select! {
        job_result = job_fut => {
            match job_result {
                Ok(state) => {
                    trace!("callback done, got state");
                    match shared.repo.save(jdata.name.clone(), Utc::now(), state).await {
                        Ok(()) => RunSelectResult::Success,
                        Err(e) => RunSelectResult::SaveFailure(e)
                    }
                },
                Err(e) => RunSelectResult::JobFailure(e)
            }
        }
        Err(e) = lock => {
            RunSelectResult::LockFailure(e)
        }
        _ = &mut shared.cancel => {
            RunSelectResult::Canceled
         }
    };

    // TODO refine all the Done cases to proper sleeps + backoff
    match select_result {
        RunSelectResult::Success => Executor::Sleeping(shared, jdata.check_interval),
        RunSelectResult::JobFailure(e) => {
            error!("job failed: {}, seleeping", e);
            Executor::Sleeping(shared, jdata.check_interval)
        }
        RunSelectResult::LockFailure(e) => {
            error!("lock refresh failed: {}, exiting executor", e);
            Executor::Done
        }
        RunSelectResult::SaveFailure(e) => {
            error!("state saving failed: {}, exiting executor", e);
            Executor::Done
        }
        RunSelectResult::Canceled => {
            info!("executor canceled");
            Executor::Done
        }
    }
}

enum RunSelectResult<E> {
    Success,
    JobFailure(E),
    LockFailure(Error),
    SaveFailure(Error),
    Canceled,
}
