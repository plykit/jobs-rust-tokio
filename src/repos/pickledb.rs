use crate::job::JobData;
use crate::{Error, JobName, LockStatus, Repo};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use cron::Schedule;
use futures::future::BoxFuture;
use futures::FutureExt;
use log::trace;
use pickledb::PickleDb;
use serde::{Deserialize, Serialize};
use std::future::Future;
use std::pin::Pin;
use std::str::FromStr;
use std::sync::Arc;
use std::task::{Context, Poll};
use std::time::{Duration, UNIX_EPOCH};
use tokio::sync::RwLock;
use tokio::time::sleep;
use AsRef;

#[derive(Clone)]
pub struct PickleDbRepo {
    pub(crate) db: Arc<RwLock<PickleDb>>,
}

impl PickleDbRepo {
    pub fn new(db: PickleDb) -> Self {
        Self {
            db: Arc::new(RwLock::new(db)),
        }
    }
}

#[derive(Clone, Serialize, Debug, Deserialize, PartialEq)]
struct JobDto {
    pub name: JobName,
    pub check_interval: u64,
    pub lock_ttl: u64,
    pub state: Vec<u8>,
    pub schedule: String,
    pub enabled: bool,
    pub last_run: u64,
    pub owner: String,
    pub expires: i64,
    pub version: i8,
}

impl From<JobData> for JobDto {
    fn from(value: JobData) -> Self {
        Self {
            name: value.name,
            check_interval: value.check_interval.as_secs(),
            lock_ttl: value.lock_ttl.as_secs(),
            state: value.state,
            schedule: value.schedule.to_string(),
            enabled: value.enabled,
            last_run: value.last_run.timestamp() as u64,
            owner: "".to_string(),
            expires: 0,
            version: 0,
        }
    }
}

impl TryFrom<JobDto> for JobData {
    type Error = Error;

    fn try_from(value: JobDto) -> std::result::Result<Self, Self::Error> {
        let schedule = Schedule::from_str(value.schedule.as_str()).map_err(|e| {
            Error::InvalidCronExpression {
                expression: value.schedule,
                msg: e.to_string(),
            }
        })?;
        Ok(Self {
            name: value.name,
            check_interval: Duration::from_secs(value.check_interval),
            lock_ttl: Duration::from_secs(value.lock_ttl),
            state: value.state,
            schedule,
            enabled: value.enabled,
            last_run: DateTime::<Utc>::from(UNIX_EPOCH + Duration::from_secs(value.last_run)),
        })
    }
}

pub struct Lock {
    fut: BoxFuture<'static, crate::Result<()>>,
}

#[async_trait]
impl Repo for PickleDbRepo {
    type Lock = Lock;

    async fn create(&mut self, job_config: JobData) -> crate::Result<()> {
        let job: JobDto = job_config.into();
        self.db
            .write()
            .await
            .set(job.name.as_ref(), &job)
            .map(|_| Ok(()))
            .map_err(|e| Error::Repo(e.to_string()))?
    }

    async fn get(&mut self, name: JobName) -> crate::Result<Option<JobData>> {
        let j = self.db.write().await.get::<JobDto>(name.as_ref());

        match j {
            None => Ok(None),
            Some(d) => {
                let jd: crate::Result<JobData> = d.try_into();
                match jd {
                    Ok(k) => Ok(Some(k)),
                    Err(e) => Err(e),
                }
            }
        }
    }

    async fn commit(&mut self, _name: JobName, _state: Vec<u8>) -> crate::Result<()> {
        todo!()
    }

    async fn save(
        &mut self,
        name: JobName,
        last_run: DateTime<Utc>,
        state: Vec<u8>,
    ) -> crate::Result<()> {
        let mut w = self.db.write().await;

        let mut j = w.get::<JobDto>(name.as_ref()).ok_or(Error::TODO)?;
        j.last_run = last_run.timestamp() as u64;
        j.owner = String::default();
        j.state = state;
        j.expires = 0;
        j.version = 0;

        w.set(name.as_ref(), &j)
            .map_err(|e| Error::Repo(e.to_string()))
    }

    async fn lock(
        &mut self,
        name: JobName,
        owner: String,
        ttl: Duration,
    ) -> crate::Result<LockStatus<Self::Lock>> {
        let mut w = self.db.write().await;

        let mut jdto = w.get::<JobDto>(name.as_ref()).ok_or(Error::TODO)?;
        if jdto.expires > Utc::now().timestamp() {
            Ok(LockStatus::AlreadyLocked)
        } else {
            jdto.owner = owner;
            jdto.expires = Utc::now().timestamp() + ttl.as_secs() as i64;
            jdto.version = 0;
            w.set(name.as_ref(), &jdto)
                .map_err(|e| Error::Repo(e.to_string()))
                .unwrap();

            let name = jdto.name.clone();
            let owner = jdto.owner.clone();
            let db = self.db.clone();

            let fut = async move {
                trace!("starting lock refresh");
                loop {
                    let refresh_interval = Duration::from_secs(ttl.as_secs() / 2);
                    sleep(refresh_interval).await;
                    let mut w = db.write().await;
                    let mut j = w.get::<JobDto>(name.as_ref()).ok_or(Error::TODO).unwrap();
                    j.expires = Utc::now().timestamp() + ttl.as_secs() as i64;
                    j.owner = owner.clone();
                    match w.set(name.0.as_str(), &j) {
                        Ok(()) => {}
                        Err(e) => return Err(Error::LockRefreshFailed(e.to_string())),
                    }
                    trace!("lock refreshed");
                }
            }
            .boxed();

            let lock = Lock { fut };

            let job_config: JobData = jdto.try_into()?;
            Ok(LockStatus::Acquired(job_config, lock))
        }
    }
}

impl Future for Lock {
    type Output = crate::Result<()>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        self.fut.poll_unpin(cx)
    }
}
