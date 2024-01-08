use log::{info, trace, warn};
use rand::Rng;
use std::convert::Infallible;
use std::time::Duration;
use tokio::sync::oneshot;
use tokio::sync::oneshot::Sender;

use crate::error::Error;
use crate::repos::Repo;
use crate::{executor, Job, JobConfig, JobName};

/// JobManager holds the job + lock repo along with the list of jobs
pub struct JobManager<J> {
    instance: String,
    job_repo: J,
    jobs: Vec<ManagedJob>,
}

#[allow(private_bounds)]
impl<J: Repo + Clone + Send + 'static> JobManager<J> {
    pub fn new(instance: String, job_repo: J) -> Self {
        JobManager {
            instance,
            job_repo,
            jobs: Default::default(),
        }
    }
    /// Add a new
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
    pub fn register(&mut self, data: JobConfig, action: impl Job + Send + 'static) {
        self.jobs.push(ManagedJob::new(data, action)); // TODO: add validation during registration??
    }

    /// start_all will spawn the jobs and run the job for ever until the job is stopped or aborted
    pub fn start_all(&mut self) -> () {
        for job in self.jobs.iter_mut().filter(|jb| jb.registered()) {
            let (tx, rx) = oneshot::channel();
            let job_repo = self.job_repo.clone();
            let action = job
                .action
                .take()
                .expect("Registered job must have some action because it cannot be taken.");
            let config = job.config.clone();

            job.status = Status::Running(tx);
            let instance = self.instance.clone();
            let mut rng = rand::thread_rng();
            let delay = Duration::from_millis(rng.gen_range(10..100));
            tokio::spawn(async move {
                let name = config.name.clone();
                match executor::run(instance, config, action, job_repo, rx, delay).await {
                    Ok(()) => trace!("job {:?} stopped", &name),
                    Err(e) => warn!("job {:?} stopped with an error: {:?}", &name, e),
                };
            });
        }
        ()
    }
    /// stop_by_name will stop the job which is started as part of start_all
    pub async fn stop_by_name(self, name: JobName) -> std::result::Result<(), Infallible> {
        for job in self.jobs.into_iter().filter(|j| j.config.name == name) {
            return match job.status {
                Status::Running(s) => {
                    info!("received stop signal. Stopping job: {:?}", name.clone());
                    let _xx = s.send(()).map_err(|()| Error::CancelFailed(name)).unwrap();
                    Ok(())
                }
                _ => Ok(()),
            };
        }
        Ok(())
    }
}

impl ManagedJob {
    pub fn new(config: JobConfig, action: impl Job + Send + 'static) -> Self {
        ManagedJob {
            config,
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

pub(crate) struct ManagedJob {
    pub config: JobConfig,
    pub action: Option<Box<dyn Job + Send>>,
    pub status: Status,
}

#[derive(Debug)]
pub(crate) enum Status {
    Registered,
    //Suspended,
    Running(Sender<()>),
}
