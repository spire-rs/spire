use http::Request;

use crate::backend::utils::Noop;
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
    use crate::backend::utils::Trace;

    let entity = Trace::new(Noop::default());
    let request = Request::get("https://example.com/").body(());
    let client = Client::new(entity.clone(), entity)
        .with_request_queue(InMemDataset::stack())
        .with_dataset(InMemDataset::<u64>::new())
        .with_initial_request(request.unwrap());

    let _ = client.dataset::<u64>();
    let _ = client.run().await?;
    Ok(())
}
