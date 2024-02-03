use std::convert::Infallible;
use std::time::Duration;

use crate::collect::Metrics;

/// Unrecoverable failure during [`Spire`] execution.
///
/// This may be extended in the future so exhaustive matching is discouraged.
///
/// [`Spire`]: crate
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("execution was user interrupted")]
    Interrupted(Metrics),
}

/// A specialized [`Result`] type for [`Spire`] operations.
///
/// [`Result`]: std::result::Result
/// [`Spire`]: crate
pub type Result<T> = std::result::Result<T, Error>;

///
/// [`ControlFlow`]: std::ops::ControlFlow
#[derive(Debug, Default, Clone)]
pub enum Signal {
    /// Task handled successfully, immediately proceed with another task.
    #[default]
    Continue,
    /// Task handled successfully, wait before the next task.
    Wait(Duration),
    /// Task failed, wait before retrying this task.
    Repeat(Duration),
    /// Task failed, stop the collector.
    Fatal,
}

pub trait IntoSignal {
    fn into_signal(self) -> Signal;
}

impl IntoSignal for () {
    fn into_signal(self) -> Signal {
        Signal::Continue
    }
}

impl IntoSignal for Infallible {
    fn into_signal(self) -> Signal {
        Signal::Continue
    }
}
