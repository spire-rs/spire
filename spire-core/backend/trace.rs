use std::fmt;

use crate::backend::{Backend, Client, DebugEntity, Worker};
use crate::context::{Context, Request, Response, Signal};
use crate::Result;

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
    type Client = T::Client;

    #[inline]
    async fn client(&self) -> Result<Self::Client> {
        // TODO: Tracing.
        self.entity.client().await
    }
}

#[async_trait::async_trait]
impl<T> Client for TraceEntity<T>
where
    T: Client,
{
    #[inline]
    async fn resolve(self, req: Request) -> Result<Response> {
        // TODO: Tracing.
        self.entity.resolve(req).await
    }
}

#[async_trait::async_trait]
impl<T, B> Worker<B> for TraceEntity<T>
where
    T: Worker<B>,
    B: Send + 'static,
{
    #[inline]
    async fn invoke(self, cx: Context<B>) -> Signal {
        // TODO: Tracing.
        self.entity.invoke(cx).await
    }
}
