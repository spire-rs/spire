#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("./README.md")]

#[doc(inline)]
pub use routing::Router;
pub use spire_core::{backend, context, dataset};
pub use spire_core::{Error, Result};

pub mod extract;
mod handler;
pub mod middleware;
pub mod routing;

/// TODO.
pub type Daemon<B, W = Router<B>> = spire_core::Daemon<B, W>;

#[doc(hidden)]
pub mod prelude {}

#[cfg(test)]
mod test {
    use crate::context::RequestQueue;
    use crate::dataset::{Dataset as _, InMemDataset};
    use crate::extract::Dataset;
    use crate::{Daemon, Result, Router};

    #[test]
    #[cfg(feature = "client")]
    fn with_client() {
        async fn handler(
            queue: RequestQueue,
            Dataset(dataset): Dataset<u32>,
            Dataset(dataset): Dataset<u64>,
        ) -> Result<()> {
            let u = dataset.get().await?;
            dataset.add(1).await?;

            Ok(())
        }

        let router = Router::new()
            .route("main$", handler)
            .route("page*", handler)
            .fallback(handler);

        let backend = crate::backend::HttpClient::default();
        let daemon = Daemon::new(backend, router)
            .with_request_queue(InMemDataset::stack())
            .with_dataset(InMemDataset::<u64>::new());

        let _ = daemon.run();
    }

    #[test]
    #[cfg(feature = "driver")]
    fn with_driver() {
        async fn handler(
            queue: RequestQueue,
            Dataset(dataset): Dataset<u32>,
            Dataset(dataset): Dataset<u64>,
        ) -> Result<()> {
            Ok(())
        }

        let router = Router::new()
            .route("main$", handler)
            .route("page*", handler)
            .fallback(handler);

        let backend = crate::backend::BrowserPool::builder().build();
        let daemon = Daemon::new(backend, router)
            .with_request_queue(InMemDataset::stack())
            .with_dataset(InMemDataset::<u64>::new());

        let _ = daemon.run();
    }
}
