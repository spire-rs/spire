use std::convert::Infallible;
use std::fmt;
use std::sync::Arc;

use deadpool::managed::{Manager, Metrics, RecycleResult};

use crate::backend::Backend;

/// Extension trait for Backend::Client
/// ... for backends that run actual browsers
pub trait Browser {}

#[derive(Clone)]
pub struct Driver {
    inner: Arc<DriverInner>,
}

struct DriverInner {
    manager: BrowserManager,
}

impl Driver {}

impl fmt::Debug for Driver {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Driver").finish_non_exhaustive()
    }
}

impl Backend for Driver {}

// BrowserPool

pub struct BrowserManager {}

impl BrowserManager {
    pub fn new() -> Self {
        Self {}
    }
}

#[deadpool::async_trait]
impl Manager for BrowserManager {
    type Type = ();
    type Error = Infallible;

    async fn create(&self) -> Result<Self::Type, Self::Error> {
        todo!()
    }

    async fn recycle(&self, obj: &mut Self::Type, metrics: &Metrics) -> RecycleResult<Self::Error> {
        todo!()
    }
}
