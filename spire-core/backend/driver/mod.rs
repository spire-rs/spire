use std::fmt;
use std::sync::Arc;

// use browser::Browser;
use manager::BrowserManager;

use crate::backend::Backend;
use crate::context::{Request, Response};
use crate::BoxError;

mod browser;
mod manager;

#[derive(Clone)]
pub struct WebDriverPool {
    inner: Arc<DriverInner>,
}

struct DriverInner {
    manager: BrowserManager,
}

impl WebDriverPool {
    pub fn new() -> Self {
        let inner = DriverInner {
            manager: BrowserManager::new(),
        };

        Self {
            inner: Arc::new(inner),
        }
    }
}

impl Default for WebDriverPool {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Debug for WebDriverPool {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Driver").finish_non_exhaustive()
    }
}

#[async_trait::async_trait]
impl Backend for WebDriverPool {
    type Client = fantoccini::Client;
    type Error = BoxError;

    async fn call(&mut self, req: Request) -> Result<Response, Self::Error> {
        todo!()
    }
}
