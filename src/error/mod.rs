use crate::schedule::InvalidCronExpression;
use crate::JobName;
use thiserror::Error;

#[derive(Error, Debug)]
pub(crate) enum Error {
    // #[error("data store disconnected")]
    // Disconnect(#[from] io::Error),
    // #[error("the data for key `{0}` is not available")]
    // Redaction(String),
    #[error(transparent)]
    InvalidCronExpression(#[from] InvalidCronExpression),
    // #[error("Job is missing: {0:?}")]
    // JobNotFound(JobName),
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
