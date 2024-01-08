use crate::schedule::Schedule;
use crate::{JobConfig, JobName};
use chrono::{DateTime, Utc};
use std::fmt::Debug;
use std::time::Duration;

#[derive(Clone, Debug)]
pub(crate) struct JobData {
    pub name: JobName,
    pub check_interval: Duration,
    pub lock_ttl: Duration,
    pub state: Vec<u8>,
    pub schedule: Schedule,
    pub enabled: bool,
    pub last_run: DateTime<Utc>,
}

impl JobData {
    pub(crate) fn due(&self, now: DateTime<Utc>) -> bool {
        self.enabled && self.schedule.due(&self.last_run, now)
    }
}

impl From<JobConfig> for JobData {
    fn from(value: JobConfig) -> Self {
        Self {
            name: value.name,
            check_interval: value.check_interval,
            lock_ttl: value.lock_ttl,
            state: Vec::default(),
            schedule: value.schedule,
            enabled: value.enabled,
            last_run: DateTime::default(),
        }
    }
}
