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

/// Orchestrates the processing of [`Request`]s using provided [`Backend`] and [`Worker`].
///
/// [`Request`]: crate::context::Request
/// [`Backend`]: crate::backend::Backend
/// [`Worker`]: crate::backend::Worker
pub type Client<B, W = Router<B>> = spire_core::Client<B, W>;

#[doc(hidden)]
pub mod prelude {}

#[cfg(test)]
mod test {
    use crate::{Client, Result, Router};
    use crate::context::RequestQueue;
    use crate::dataset::{Dataset, InMemDataset};
    use crate::extract::Data;

    #[test]
    #[cfg(feature = "client")]
    fn with_client() {
        async fn handler(queue: RequestQueue, Data(dataset): Data<u64>) -> Result<()> {
            let u = dataset.get().await?;
            dataset.add(1).await?;

            Ok(())
        }

        let router = Router::new()
            .route("main$", handler)
            .route("page*", handler)
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
        async fn handler(queue: RequestQueue, Data(dataset): Data<u64>) -> Result<()> {
            let u = dataset.get().await?;
            dataset.add(1).await?;

            Ok(())
        }

        let router = Router::new()
            .route("main$", handler)
            .route("page*", handler)
            .fallback(handler);

        let backend = crate::backend::BrowserPool::builder().build();
        let client = Client::new(backend, router)
            .with_request_queue(InMemDataset::stack())
            .with_dataset(InMemDataset::<u64>::new());

        let _ = client.run();
    }
}
