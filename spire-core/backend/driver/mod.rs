use std::fmt;
use std::future::{Future, Ready};
use std::task::{Context, Poll};

use deadpool::managed::Pool;
use fantoccini::Client;
use tower::Service;

// use browser::Browser;
use manager::BrowserManager;

use crate::backend::Backend;
use crate::context::{Request, Response};
use crate::Error;

mod browser;
mod manager;

#[derive(Clone)]
pub struct BrowserPool {
    pool: Pool<BrowserManager>,
}

impl BrowserPool {
    pub fn new(pool: Pool<BrowserManager>) -> Self {
        Self { pool }
    }

    pub fn builder() -> BrowserManager {
        BrowserManager::new()
    }
}

impl Default for BrowserPool {
    fn default() -> Self {
        // Self::builder()
        todo!()
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
        Poll::Ready(Ok(()))
    }

    #[inline]
    fn call(&mut self, req: Request) -> Self::Future {
        todo!()
    }
}

impl Backend for BrowserPool {
    type Client = Client;
}
