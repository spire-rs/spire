use std::collections::HashMap;

use deadpool::managed::{Manager, Metrics, PoolError, RecycleResult as RecResult};

use crate::backend::driver::BrowserConnection;
use crate::backend::BrowserClient;
use crate::{Error, Result};

/// [`BrowserPool`] manager. Creates browser process and establishes connection.
#[must_use]
pub struct BrowserManager {
    connections: HashMap<u64, BrowserConnection>,
}

impl BrowserManager {
    /// Creates a new [`BrowserManager`].
    #[inline]
    pub fn new() -> Self {
        Self::default()
    }

    /// TODO.
    pub fn with(&self, conn: BrowserConnection) -> Result<()> {
        todo!()
    }

    /// TODO.
    fn autoscale(&self) -> Result<()> {
        todo!()
    }
}

impl Default for BrowserManager {
    fn default() -> Self {
        // TODO: BLOCKED: https://github.com/jonhoo/fantoccini/pull/245

        Self {
            connections: HashMap::new(),
        }
    }
}

impl Manager for BrowserManager {
    type Type = BrowserClient;
    type Error = Error;

    async fn create(&self) -> Result<Self::Type, Self::Error> {
        todo!()
    }

    async fn recycle(&self, obj: &mut Self::Type, metrics: &Metrics) -> RecResult<Self::Error> {
        todo!()
    }
}

impl From<PoolError<Error>> for Error {
    fn from(value: PoolError<Error>) -> Self {
        todo!()
    }
}
