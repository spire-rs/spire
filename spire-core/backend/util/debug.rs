use std::fmt;

use crate::backend::{Backend, Client, Worker};
use crate::context::{Body, Context, IntoSignal, Request, Response, Signal};
use crate::Result;

/// No-op [`Backend`], [`Client`] and [`Worker`] used for testing and debugging.
#[must_use]
#[derive(Clone, Default)]
pub struct WithDebug {
    always: Option<bool>,
}

impl WithDebug {
    /// Creates a new [`WithDebug`] with an `always` rule.
    pub const fn new(always: Option<bool>) -> Self {
        Self { always }
    }
}

impl fmt::Debug for WithDebug {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DebugEntity").finish_non_exhaustive()
    }
}

#[async_trait::async_trait]
impl Backend for WithDebug {
    type Client = Self;

    #[inline]
    async fn client(&self) -> Result<Self::Client> {
        Ok(self.clone())
    }
}

#[async_trait::async_trait]
impl Client for WithDebug {
    #[inline]
    async fn resolve(self, _: Request) -> Result<Response> {
        Ok(Response::new(Body::default()))
    }
}

#[async_trait::async_trait]
impl<C> Worker<C> for WithDebug
where
    C: Client,
{
    async fn invoke(self, cx: Context<C>) -> Signal {
        match self.always {
            Some(true) => return Signal::Continue,
            Some(false) => return Signal::Skip,
            _ => {}
        }

        let resp = cx.resolve().await;
        resp.map_or_else(IntoSignal::into_signal, |_| Signal::default())
    }
}
