#[cfg(all(feature = "pickledb", feature = "mongodb"))]
compile_error!("feature \"pickledb\" and feature \"mongodb\" cannot be enabled at the same time");

mod error;
mod executor;
mod job;
mod manager;
mod repos;
pub mod schedule;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::fmt::{Debug, Display, Formatter};
use std::time::Duration;

pub use manager::JobManager;
#[cfg(feature = "mongodb")]
pub use repos::mongo::MongoRepo;
#[cfg(feature = "pickledb")]
pub use repos::pickledb::PickleDbRepo;
use schedule::Schedule;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct JobName(pub String);

impl JobName {
    pub fn as_str(&self) -> &str {
        &self.0.as_str()
    }
}

impl AsRef<str> for crate::JobName {
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

pub struct JobError(String);

impl JobError {
    pub fn todo() -> Self {
        JobError("todo".into())
    }
    pub fn any(err: impl std::error::Error) -> Self {
        JobError(err.to_string())
    }
    pub fn data_corruption(err: impl std::error::Error) -> Self {
        JobError(format!("data corruption: {}", err))
    }
}

impl From<&str> for JobError {
    fn from(value: &str) -> Self {
        JobError(value.to_owned())
    }
}

impl From<String> for JobError {
    fn from(value: String) -> Self {
        JobError(value)
    }
}

impl Debug for JobError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("JobError('{}')", self.0))
    }
}

impl Display for JobError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("JobError('{}')", self.0))
    }
}
impl std::error::Error for JobError {}

#[async_trait]
pub trait Job {
    async fn call(&mut self, state: Vec<u8>) -> Result<Vec<u8>, JobError>;
}
