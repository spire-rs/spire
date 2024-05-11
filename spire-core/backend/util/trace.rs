use std::fmt;

use http_body::Body;

use crate::backend::{util::WithDebug, Backend, Client, Worker};
use crate::context::{Context, Request, Response, Signal, Task};
use crate::dataset::Dataset;
use crate::Result;

/// Tracing [`Backend`], [`Client`] or [`Worker`] for improved observability.
#[must_use]
#[derive(Clone)]
pub struct WithTrace<T> {
    entity: T,
}

impl<T> WithTrace<T> {
    /// Creates a new [`WithTrace`].
    pub const fn new(entity: T) -> Self {
        Self { entity }
    }
}

impl Default for WithTrace<WithDebug> {
    fn default() -> Self {
        Self::new(WithDebug::default())
    }
}

impl<T> fmt::Debug for WithTrace<T>
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
impl<T> Backend for WithTrace<T>
where
    T: Backend + Sync,
{
    type Client = WithTrace<T::Client>;

    async fn client(&self) -> Result<Self::Client> {
        let client = self.entity.client().await?;

        tracing::trace!("initialized new client");

        Ok(WithTrace::new(client))
    }
}

#[async_trait::async_trait]
impl<T> Client for WithTrace<T>
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
            status = resp.status().as_u16(),
            lower = resp.body().size_hint().lower(),
            upper = resp.body().size_hint().upper(),
            "response body"
        );

        Ok(resp)
    }
}

#[async_trait::async_trait]
impl<T, B> Worker<B> for WithTrace<T>
where
    T: Worker<B>,
    B: Send + 'static,
{
    async fn invoke(self, cx: Context<B>) -> Signal {
        let requests = cx.dataset::<Request>().into_inner();

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
