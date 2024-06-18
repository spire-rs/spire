#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("./README.md")]
// #![warn(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

#[doc(no_inline)]
pub use async_trait::async_trait;

#[doc(inline)]
pub use routing::Router;
use spire_core::backend::Backend;
pub use spire_core::{backend, context, dataset};
pub use spire_core::{Error, Result};

pub mod extract;
mod handler;
pub mod middleware;
pub mod routing;

/// Orchestrates the processing of [`Request`]s using provided [`Backend`] and `State`.
///
/// [`Request`]: context::Request
/// [`Backend`]: backend::Backend
pub type Client<B, W = Router<<B as Backend>::Client>> = spire_core::Client<B, W>;

#[doc(hidden)]
pub mod prelude {}

#[cfg(test)]
mod test {
    use futures::SinkExt;

    use spire_core::dataset::future::DataSink;

    use crate::context::RequestQueue;
    use crate::dataset::InMemDataset;
    use crate::{Client, Result, Router};

    #[test]
    #[cfg(feature = "client")]
    fn with_client() {
        async fn handler(queue: RequestQueue, mut dataset: DataSink<u64>) -> Result<()> {
            dataset.feed(1).await?;
            Ok(())
        }

        let router = Router::new()
            .route("main", handler)
            .route("page", handler)
            .fallback(handler);

        let backend = crate::backend::HttpClient::default();
        let client = Client::new(backend, router)
            .with_request_queue(InMemDataset::stack())
            .with_dataset(InMemDataset::<u64>::new());

        let _ = client.run();
    }

    #[test]
    #[cfg(feature = "driver")]
    fn with_driver() {
        async fn handler(queue: RequestQueue, mut dataset: DataSink<u64>) -> Result<()> {
            dataset.feed(1).await?;
            Ok(())
        }

        let router = Router::new()
            .route("main", handler)
            .route("page", handler)
            .fallback(handler);

        let backend = crate::backend::BrowserPool::builder().build();
        let client = Client::new(backend, router)
            .with_request_queue(InMemDataset::stack())
            .with_dataset(InMemDataset::<u64>::new());

        let _ = client.run();
    }
}
