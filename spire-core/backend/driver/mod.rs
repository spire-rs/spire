use std::fmt;
use std::sync::Arc;

use manager::BrowserManager;

use crate::backend::Backend;
use crate::BoxError;
use crate::context::{Request, Response};

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

mod manager {
    use std::convert::Infallible;

    use deadpool::managed::{Manager, Metrics, RecycleResult};

    /// Extension trait for Backend::Client
    /// ... for backends that run actual browsers
    pub trait Browser {}

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

        async fn recycle(
            &self,
            obj: &mut Self::Type,
            metrics: &Metrics,
        ) -> RecycleResult<Self::Error> {
            todo!()
        }
    }
}
