use std::fmt;

use crate::backend::{Backend, Client, Worker};
use crate::context::{Body, Context, IntoSignal, Request, Response, Signal};
use crate::Result;

/// No-op [`Backend`], [`Client`] and [`Worker`] used for testing and debugging.
#[derive(Clone, Default)]
pub struct DebugEntity {
    always: Option<bool>,
}

impl DebugEntity {
    /// Creates a new [`DebugEntity`] with an `always` rule.
    pub fn new(always: Option<bool>) -> Self {
        Self { always }
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
        Ok(self.clone())
    }
}

#[async_trait::async_trait]
impl Client for DebugEntity {
    #[inline]
    async fn resolve(self, _: Request) -> Result<Response> {
        Ok(Response::new(Body::default()))
    }
}

#[async_trait::async_trait]
impl<B> Worker<B> for DebugEntity
where
    B: Backend,
{
    async fn invoke(self, cx: Context<B>) -> Signal {
        match self.always {
            Some(true) => return Signal::Continue,
            Some(false) => return Signal::Skip,
            _ => {}
        }

        let resp = cx.try_resolve().await;
        resp.map_or_else(IntoSignal::into_signal, |_| Signal::default())
    }
}
