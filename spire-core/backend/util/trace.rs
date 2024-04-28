use std::fmt;

use http_body::Body;

use crate::backend::{util::DebugEntity, Backend, Client, Worker};
use crate::context::{Context, Request, Response, Signal, Task};
use crate::dataset::Dataset;
use crate::Result;

/// Tracing [`Backend`], [`Client`] or [`Worker`] for improved observability.
#[derive(Clone)]
pub struct TraceEntity<T> {
    entity: T,
}

impl<T> TraceEntity<T> {
    /// Creates a new [`TraceEntity`].
    pub fn new(entity: T) -> Self {
        Self { entity }
    }
}

impl Default for TraceEntity<DebugEntity> {
    fn default() -> Self {
        Self::new(DebugEntity::default())
    }
}

impl<T> fmt::Debug for TraceEntity<T>
where
    T: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TraceWorker")
            .field("entity", &self.entity)
            .finish()
    }
}

#[async_trait::async_trait]
impl<T> Backend for TraceEntity<T>
where
    T: Backend + Sync,
{
    type Client = TraceEntity<T::Client>;

    async fn client(&self) -> Result<Self::Client> {
        let client = self.entity.client().await?;

        tracing::trace!("initialized new client");

        Ok(TraceEntity::new(client))
    }
}

#[async_trait::async_trait]
impl<T> Client for TraceEntity<T>
where
    T: Client,
{
    async fn resolve(self, req: Request) -> Result<Response> {
        tracing::trace!(
            lower = req.body().size_hint().lower(),
            upper = req.body().size_hint().upper(),
            "request body"
        );

        let resp = self.entity.resolve(req).await?;

        tracing::trace!(
            lower = resp.body().size_hint().lower(),
            upper = resp.body().size_hint().upper(),
            "response body"
        );

        Ok(resp)
    }
}

#[async_trait::async_trait]
impl<T, B> Worker<B> for TraceEntity<T>
where
    T: Worker<B>,
    B: Send + 'static,
{
    async fn invoke(self, cx: Context<B>) -> Signal {
        let requests = cx.dataset::<Request>();

        tracing::trace!(
            depth = cx.get_ref().depth(),
            requests = requests.len(),
            "invoked handler"
        );

        let signal = self.entity.invoke(cx).await;
        let _ = match &signal {
            Signal::Continue | Signal::Wait(..) => false,
            Signal::Skip | Signal::Hold(..) | Signal::Fail(..) => true,
        };

        tracing::trace!(
            // signal = signal.as_str(),
            requests = requests.len(),
            "returned handler"
        );

        signal
    }
}
