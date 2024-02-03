#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("./README.md")]

pub(crate) use crate::handler::*;
pub use crate::process::*;

mod handler;
mod process;

// TODO: Download.
// TODO: Install.

/// Unrecoverable failure during [`Driver`] execution.
///
/// This may be extended in the future so exhaustive matching is discouraged.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("failed to spawn: already spawned")]
    AlreadySpawned,
    #[error("failed to spawn: {0}")]
    FailedToSpawn(std::io::Error),
    #[error("failed to kill: {0}")]
    FailedToAbort(std::io::Error),
}

/// A specialized [`Result`] type for [`Driver`] operations.
///
/// [`Result`]: std::result::Result
pub type Result<T> = std::result::Result<T, Error>;
