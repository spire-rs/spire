//! Various utility [`Backend`]s, [`Client`]s and [`Worker`]s.
//!
//! [`Backend`]: crate::backend::Backend
//! [`Client`]: crate::backend::Client
//! [`Worker`]: crate::backend::Worker

pub use debug::Noop;
#[cfg(feature = "metric")]
#[cfg_attr(docsrs, doc(cfg(feature = "metric")))]
pub use metric::{Metric, MetricLayer};
#[cfg(feature = "trace")]
#[cfg_attr(docsrs, doc(cfg(feature = "trace")))]
pub use trace::{Trace, TraceLayer};

mod debug;
#[cfg(feature = "metric")]
#[cfg_attr(docsrs, doc(cfg(feature = "metric")))]
mod metric;
#[cfg(feature = "trace")]
#[cfg_attr(docsrs, doc(cfg(feature = "trace")))]
mod trace;

#[cfg(test)]
mod test {
    use http::Request;

    use crate::backend::util::Noop;
    use crate::dataset::InMemDataset;
    use crate::{Client, Result};

    #[tokio::test]
    async fn noop() -> Result<()> {
        let entity = Noop::default();
        let request = Request::get("https://example.com/").body(());
        let client = Client::new(entity.clone(), entity)
            .with_request_queue(InMemDataset::stack())
            .with_dataset(InMemDataset::<u64>::new())
            .with_initial_request(request.unwrap());

        let _ = client.dataset::<u64>();
        let _ = client.run().await?;
        Ok(())
    }

    #[tokio::test]
    #[cfg(feature = "trace")]
    #[tracing_test::traced_test]
    async fn noop_trace() -> Result<()> {
        use crate::backend::util::Trace;

        let entity = Trace::default();
        let request = Request::get("https://example.com/").body(());
        let client = Client::new(entity.clone(), entity)
            .with_request_queue(InMemDataset::stack())
            .with_dataset(InMemDataset::<u64>::new())
            .with_initial_request(request.unwrap());

        let _ = client.dataset::<u64>();
        let _ = client.run().await?;
        Ok(())
    }
}
