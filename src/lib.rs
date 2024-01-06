#[cfg(all(feature = "pickledb", feature = "mongodb"))]
compile_error!("feature \"pickledb\" and feature \"mongodb\" cannot be enabled at the same time");

mod executor;
mod job;
mod manager;
mod repos;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use cron::Schedule;
use derive_more::Into;
use serde::{Deserialize, Serialize};
use std::future::Future;
use std::time::Duration;
use thiserror::Error;

pub mod schedule;
use job::JobData;
pub use manager::JobManager;
#[cfg(feature = "mongodb")]
pub use repos::mongo::MongoRepo;
#[cfg(feature = "pickledb")]
pub use repos::pickledb::PickleDbRepo;

#[async_trait]
pub trait Job {
    type Error: Sync + Send + 'static;
    async fn call(&mut self, state: Vec<u8>) -> std::result::Result<Vec<u8>, Self::Error>;
}

#[derive(Error, Debug)]
pub enum Error {
    // #[error("data store disconnected")]
    // Disconnect(#[from] io::Error),
    // #[error("the data for key `{0}` is not available")]
    // Redaction(String),
    #[error("invalid cron expression {expression:?}: {msg:?})")]
    InvalidCronExpression { expression: String, msg: String },
    #[error("Job is missing: {0:?}")]
    JobNotFound(JobName),
    #[error("Repository error: {0}")]
    Repo(String),
    #[error("Loack refresh failed: {0}")]
    LockRefreshFailed(String),
    #[error("canceling job {0:?} failed")]
    CancelFailed(JobName),

    #[error("TODO")]
    TODO,
}

pub type Result<T> = std::result::Result<T, Error>;

pub enum LockStatus<LOCK> {
    Acquired(JobData, LOCK),
    AlreadyLocked,
}

#[async_trait]
pub trait Repo {
    type Lock: Future<Output = Result<()>> + Send;
    // Transactionally create job config entry if it does not exist.
    async fn create(&mut self, data: JobData) -> Result<()>;
    // Obtain job data by name without locking
    async fn get(&mut self, name: JobName) -> Result<Option<JobData>>;
    // Save state without unlocking so jobs can do intermediate commits.
    async fn commit(&mut self, name: JobName, state: Vec<u8>) -> Result<()>;
    // Save the job state after the job ran and release the lock.
    async fn save(&mut self, name: JobName, last_run: DateTime<Utc>, state: Vec<u8>) -> Result<()>;
    // Get the job data if the lock can be obtained. Return job data and the lock future.
    async fn lock(
        &mut self,
        name: JobName,
        owner: String,
        ttl: Duration,
    ) -> Result<LockStatus<Self::Lock>>;
}

#[derive(Default, Clone, Into, Eq, Hash, PartialEq, Debug, Serialize, Deserialize)]
pub struct JobName(pub String);

impl JobName {
    pub fn as_str(&self) -> &str {
        &self.0.as_str()
    }
}

impl AsRef<str> for JobName {
    fn as_ref(&self) -> &str {
        self.0.as_str()
    }
}

#[derive(Clone)]
pub struct JobConfig {
    pub name: JobName,
    pub check_interval: Duration,
    pub lock_ttl: Duration,
    pub schedule: Schedule,
    pub enabled: bool,
}

impl JobConfig {
    pub fn new(name: impl Into<String>, schedule: Schedule) -> Self {
        JobConfig {
            name: JobName(name.into()),
            schedule,
            check_interval: Duration::from_secs(60),
            lock_ttl: Duration::from_secs(20),
            enabled: true,
        }
    }
    pub fn with_check_interval(mut self, interval: Duration) -> Self {
        self.check_interval = interval;
        self
    }
    pub fn with_lock_ttl(mut self, ttl: Duration) -> Self {
        self.lock_ttl = ttl;
        self
    }
}
