use std::convert::Infallible;
use std::fmt;
use std::future::Future;

use deadpool::managed::{Manager, Metrics, Pool, RecycleResult};

use crate::backend::BrowserPool;

pub struct BrowserManager {
    // builder: ClientBuilder<()>,
    // webdriver: Vec<()>
}

impl BrowserManager {
    pub fn new() -> Self {
        Self {}
    }

    pub fn with_instance(self, browser: ()) -> Self {
        todo!()
    }

    pub fn with_dynamic<F, Fut>(self, f: F) -> Self
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = Option<()>>,
    {
        todo!()
    }

    pub fn build(self) -> BrowserPool {
        BrowserPool::new(self.into_pool())
    }

    fn into_pool(self) -> Pool<Self> {
        Pool::builder(self).build().expect("should not timeout")
    }
}

impl Default for BrowserManager {
    fn default() -> Self {
        todo!()
    }
}

impl fmt::Debug for BrowserManager {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        todo!()
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
