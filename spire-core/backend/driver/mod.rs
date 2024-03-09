use std::fmt;
use std::sync::Arc;

use manager::BrowserManager;

use crate::backend::Backend;
use crate::context::{Request, Response};
use crate::BoxError;

mod manager;

#[derive(Clone)]
pub struct Driver {
    inner: Arc<DriverInner>,
}

struct DriverInner {
    manager: BrowserManager,
}

impl Driver {
    pub fn new() -> Self {
        let inner = DriverInner {
            manager: BrowserManager::new(),
        };

        Self {
            inner: Arc::new(inner),
        }
    }
}

impl Default for Driver {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Debug for Driver {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Driver").finish_non_exhaustive()
    }
}

#[async_trait::async_trait]
impl Backend for Driver {
    type Client = fantoccini::Client;
    type Error = BoxError;

    async fn try_resolve(&mut self, request: Request) -> Result<Response, Self::Error> {
        todo!()
    }
}
