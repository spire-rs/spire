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
    use crate::{Daemon, Result, Router};
    use crate::backend::HttpClient;
    use crate::context::{Queue, Tag};
    use crate::dataset::{Dataset as _, InMemDataset};
    use crate::extract::{Dataset, Html, transform::Reduce};

    #[test]
    fn example() {
        async fn handler(
            queue: Queue,
            Dataset(dataset): Dataset<u64>,
            Html(html): Html<Reduce>,
        ) -> Result<()> {
            let u = dataset.get().await?;
            dataset.add(1).await?;

            Ok(())
        }

        let router = Router::new()
            .route(Tag::Rehash(1), handler)
            .route(Tag::Rehash(2), handler)
            .fallback(handler);

        let daemon = Daemon::new(HttpClient::new(), router)
            .with_queue(InMemDataset::queue())
            .with_dataset(InMemDataset::<u64>::new());

        let _ = daemon.run();
    }
}
