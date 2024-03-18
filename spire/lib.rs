#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("./README.md")]

#[doc(inline)]
pub use routing::Router;
pub use spire_core::{backend, context, dataset};
pub use spire_core::{Daemon, Error, Result};

pub mod extract;
mod handler;
pub mod routing;

#[doc(hidden)]
pub mod prelude {}

#[cfg(test)]
mod test {
    use spire_core::backend::BrowserPool;

    use crate::backend::HttpClient;
    use crate::context::RequestQueue;
    use crate::dataset::{Dataset as _, InMemDataset};
    use crate::extract::Dataset;
    use crate::{Daemon, Result, Router};

    #[test]
    fn http_client() {
        async fn handler(queue: RequestQueue, Dataset(dataset): Dataset<u64>) -> Result<()> {
            let u = dataset.get().await?;
            dataset.add(1).await?;

            Ok(())
        }

        let router = Router::new()
            .route("main$", handler)
            .route("page*", handler)
            .fallback(handler);

        let backend = HttpClient::default();
        let daemon = Daemon::new(backend, router)
            .with_queue(InMemDataset::stack())
            .with_dataset(InMemDataset::<u64>::new());

        let _ = daemon.run();
    }

    #[test]
    fn browser() {
        async fn handler() -> Result<()> {
            Ok(())
        }

        let router = Router::new()
            .route("main$", handler)
            .route("page*", handler)
            .fallback(handler);

        let backend = BrowserPool::builder().build();
        let daemon = Daemon::new(backend, router)
            .with_queue(InMemDataset::stack())
            .with_dataset(InMemDataset::<u64>::new());

        let _ = daemon.run();
    }
}
