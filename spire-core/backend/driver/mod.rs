//! TODO.
//!
//!

use std::fmt;
use std::future::Ready;
use std::task::{Context, Poll};

use deadpool::managed::Pool;
use tower::Service;

pub use builder::BrowserManager;
pub use handler::BrowserClient;
use process::BrowserProcess;

use crate::{Error, Result};
use crate::backend::{Backend, BrowserBackend};
use crate::context::{Request, Response};

mod builder;
mod handler;
mod process;

/// Web-driver [`Backend`].
#[derive(Clone)]
pub struct BrowserPool {
    pool: Pool<BrowserManager>,
}

impl BrowserPool {
    pub(crate) fn new(pool: Pool<BrowserManager>) -> Self {
        Self { pool }
    }

    pub fn builder() -> BrowserManager {
        BrowserManager::new()
    }
}

impl Default for BrowserPool {
    fn default() -> Self {
        // TODO. Add default processes.
        Self::builder().build()
    }
}

impl fmt::Debug for BrowserPool {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Driver").finish_non_exhaustive()
    }
}

impl Service<Request> for BrowserPool {
    type Response = Response;
    type Error = Error;
    type Future = Ready<Result<Response, Error>>;

    #[inline]
    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        // TODO: Check for available browsers.
        Poll::Ready(Ok(()))
    }

    #[inline]
    fn call(&mut self, req: Request) -> Self::Future {
        todo!()
    }
}

#[async_trait::async_trait]
impl Backend for BrowserPool {
    type Client = BrowserClient;

    async fn call(&self) -> Result<Self::Client> {
        todo!()
    }
}

impl BrowserBackend for BrowserPool {}

#[cfg(test)]
mod test {
    use crate::backend::BrowserManager;

    #[test]
    pub fn builder() {
        let manager = BrowserManager::default();
        let _ = manager.build();
    }
}
