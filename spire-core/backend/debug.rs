use std::fmt;

use crate::backend::{Backend, Client, Worker};
use crate::context::{Context, Request, Response, Signal};
use crate::Result;

/// No-op [`Backend`], [`Client`] and [`Worker`] used for testing and debugging.
#[derive(Clone)]
pub struct DebugEntity {}

impl DebugEntity {
    /// Creates a new [`DebugEntity`].
    pub fn new() -> Self {
        Self::default()
    }
}

impl Default for DebugEntity {
    fn default() -> Self {
        todo!()
    }
}

impl fmt::Debug for DebugEntity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DebugEntity").finish_non_exhaustive()
    }
}

#[async_trait::async_trait]
impl Backend for DebugEntity {
    type Client = DebugEntity;

    #[inline]
    async fn client(&self) -> Result<Self::Client> {
        todo!()
    }
}

#[async_trait::async_trait]
impl Client for DebugEntity {
    #[inline]
    async fn resolve(self, req: Request) -> Result<Response> {
        todo!()
    }
}

#[async_trait::async_trait]
impl<B> Worker<B> for DebugEntity
where
    B: Send + 'static,
{
    #[inline]
    async fn invoke(self, cx: Context<B>) -> Signal {
        todo!()
    }
}
