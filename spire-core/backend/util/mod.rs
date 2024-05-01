//! Various utility [`Backend`]s, [`Client`]s and [`Worker`]s.
//!
//! [`Backend`]: crate::backend::Backend
//! [`Client`]: crate::backend::Client
//! [`Worker`]: crate::backend::Worker

pub use debug::WithDebug;
#[cfg(feature = "tracing")]
#[cfg_attr(docsrs, doc(cfg(feature = "tracing")))]
pub use trace::WithTrace;

mod debug;
#[cfg(feature = "tracing")]
#[cfg_attr(docsrs, doc(cfg(feature = "tracing")))]
mod trace;

#[cfg(test)]
mod test {
    use http::Request;

    use crate::backend::util::WithTrace;
    use crate::dataset::InMemDataset;
    use crate::{Client, Result};

    #[tokio::test]
    #[cfg(feature = "tracing")]
    #[tracing_test::traced_test]
    async fn noop() -> Result<()> {
        let entity = WithTrace::default();

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
