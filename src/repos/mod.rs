use crate::job::JobData;
use crate::{error, JobName};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use futures::FutureExt;
use futures_util::future::BoxFuture;
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::Duration;

#[cfg(feature = "mongodb")]
pub mod mongo;

#[cfg(feature = "pickledb")]
pub mod pickledb;

pub(crate) struct Lock {
    fut: BoxFuture<'static, crate::error::Result<()>>,
}

impl Future for Lock {
    type Output = crate::error::Result<()>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        self.fut.poll_unpin(cx)
    }
}

pub(crate) enum LockStatus<LOCK> {
    Acquired(JobData, LOCK),
    AlreadyLocked,
}

#[async_trait]
pub(crate) trait Repo {
    type Lock: Future<Output = error::Result<()>> + Send;
    // Transactionally create job config entry if it does not exist.
    async fn create(&mut self, data: JobData) -> error::Result<()>;
    // Obtain job data by name without locking
    async fn get(&mut self, name: JobName) -> error::Result<Option<JobData>>;
    // Save state without unlocking so jobs can do intermediate commits.
    async fn commit(&mut self, name: JobName, state: Vec<u8>) -> error::Result<()>;
    // Save the job state after the job ran and release the lock.
    async fn save(
        &mut self,
        name: JobName,
        last_run: DateTime<Utc>,
        state: Vec<u8>,
    ) -> error::Result<()>;
    // Get the job data if the lock can be obtained. Return job data and the lock future.
    async fn lock(
        &mut self,
        name: JobName,
        owner: String,
        ttl: Duration,
    ) -> error::Result<LockStatus<Self::Lock>>;
}
