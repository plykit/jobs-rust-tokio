use log::{info, trace, warn};
use rand::Rng;
use std::time::Duration;
use tokio::sync::oneshot;
use tokio::sync::oneshot::Sender;

use crate::{executor, Error, Job, JobConfig, JobName, Repo, Result};

/// JobManager holds the job + lock repo along with the list of jobs
pub struct JobManager<J>
where
    J: Repo + Sync + Send + Clone,
{
    instance: String,
    job_repo: J,
    jobs: Vec<ManagedJob>,
}

impl<J: Repo + Clone + Send + Sync + 'static> JobManager<J> {
    pub fn new(instance: String, job_repo: J) -> Self {
        JobManager {
            instance,
            job_repo,
            jobs: Default::default(),
        }
    }
    /// register will add the job to the vector of jobs in JobManager
    /// ```rust,ignore
    ///     let mut manager = JobManager::<Repo, Repo>::new(db_repo, lock_repo);
    ///     manager.register(
    ///         String::from("project-updater"),
    ///         job.clone(),
    ///         Schedule {
    ///             expr: "* */3 * * * *".to_string(),
    ///        },
    ///     );
    pub fn register(&mut self, data: JobConfig, action: impl Job + Send + Sync + 'static) {
        self.jobs.push(ManagedJob::new(data, action)); // TODO: add validation during registration??
    }

    /// start_all will spawn the jobs and run the job for ever until the job is stopped or aborted
    pub async fn start_all(&mut self) -> Result<()> {
        for job in self.jobs.iter_mut().filter(|jb| jb.registered()) {
            let (tx, rx) = oneshot::channel();
            let job_repo = self.job_repo.clone();
            let action = job.action.take().unwrap();
            let data = job.data.clone();

            job.status = Status::Running(tx);
            let instance = self.instance.clone();
            let mut rng = rand::thread_rng();
            let delay = Duration::from_millis(rng.gen_range(10..100));
            tokio::spawn(async move {
                let name = data.name.clone();
                trace!("start executor for job {:?}", &name);
                // let ex = Executor::new(instance, data, action, job_repo, rx, delay);
                match executor::run(instance, data, action, job_repo, rx, delay).await {
                    Ok(()) => trace!("job {:?} stopped", &name),
                    Err(e) => warn!("job {:?} stopped with an error: {:?}", &name, e),
                };
            });
        }
        Ok(())
    }
    /// stop_by_name will stop the job which is started as part of start_all
    pub async fn stop_by_name(self, name: JobName) -> Result<()> {
        for job in self.jobs.into_iter().filter(|j| j.data.name == name) {
            return match job.status {
                Status::Running(s) => {
                    info!("received stop signal. Stopping job: {:?}", name.clone());
                    let _xx = s.send(()).map_err(|()| Error::CancelFailed(name))?;
                    Ok(())
                }
                _ => Ok(()),
            };
        }
        Ok(())
    }
}

impl ManagedJob {
    pub fn new(data: JobConfig, action: impl Job + Send + Sync + 'static) -> Self {
        ManagedJob {
            data,
            action: Some(Box::new(action)),
            status: Status::Registered,
        }
    }
    pub fn registered(&self) -> bool {
        match self.status {
            Status::Registered => true,
            _ => false,
        }
    }
}

pub struct ManagedJob {
    pub data: JobConfig,
    pub action: Option<Box<dyn Job + Send + Sync>>,
    pub status: Status,
}

#[derive(Debug)]
pub(crate) enum Status {
    Registered,
    //Suspended,
    Running(Sender<()>),
}
