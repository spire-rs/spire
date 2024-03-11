#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("./README.md")]

#[doc(inline)]
pub use routing::Router;
pub use spire_core::{backend, context, dataset};
pub use spire_core::{Daemon, Error, Result};

pub mod extract;
pub mod handler;
pub mod routing;

#[doc(hidden)]
pub mod prelude {}

#[cfg(test)]
mod test {
    use crate::backend::HttpClient;
    use crate::context::Queue;
    use crate::dataset::{Dataset as _, InMemDataset};
    use crate::extract::Dataset;
    use crate::{Daemon, Result, Router};

    #[test]
    fn example() {
        async fn handler(queue: Queue, Dataset(dataset): Dataset<u64>) -> Result<()> {
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
            .with_queue(InMemDataset::queue())
            .with_dataset(InMemDataset::<u64>::new());

        let _ = daemon.run();
    }
}
