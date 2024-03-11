use std::convert::Infallible;

use deadpool::managed::{Manager, Metrics, RecycleResult};

pub struct BrowserManager {
    // builder: ClientBuilder<()>,
    // webdriver: Vec<()>
}

impl BrowserManager {
    pub fn new() -> Self {
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
