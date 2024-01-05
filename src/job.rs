use chrono::{DateTime, Utc};
use cron::Schedule;
use log::trace;
use std::fmt;
use std::fmt::Display;
use std::time::Duration;
use crate::{JobConfig, JobName};

#[derive(Clone, Debug)]
pub struct JobData {
    pub name: JobName,
    pub check_interval: Duration,
    pub lock_ttl: Duration,
    pub state: Vec<u8>,
    pub schedule: Schedule,
    pub enabled: bool,
    pub last_run: DateTime<Utc>,
}

impl JobData {
    pub fn due(&self, now: DateTime<Utc>) -> bool {
        if !self.enabled {
            return false;
        }
        trace!("last run: {:?}", self.last_run);
        let next = self
            .schedule
            .after(&self.last_run)
            .next()
            .unwrap_or_else(|| DateTime::default());
        trace!("next run: {:?}", next);
        next.lt(&now)
    }
}

impl Display for JobData {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "[{} schedule='{}' lastrun={} interval={}s ttl={}s]",
            self.name.0,
            self.schedule,
            self.last_run,
            self.check_interval.as_secs(),
            self.lock_ttl.as_secs()
        )
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
