use std::convert::Infallible;
use std::fmt;
use std::future::Future;
use std::pin::Pin;
use std::task::{ready, Context, Poll};

use pin_project_lite::pin_project;
use tower::{Layer, Service};

use crate::context::{Context as Cx, Signal};

/// Populates [`RequestQueue`] with [`Request`]s from a retrieved `sitemap.xml`.
///
/// [`RequestQueue`]: crate::context::RequestQueue
/// [`Request`]: crate::context::Request
#[derive(Clone)]
pub struct Include<S> {
    inner: S,
}

impl<S> Include<S> {
    /// Creates a new [`Include`] with a provided inner service.
    pub fn new(inner: S) -> Self {
        Self { inner }
    }

    /// Returns a reference to the inner service.
    pub fn get_ref(&self) -> &S {
        &self.inner
    }

    /// Returns a mutable reference to the inner service.
    pub fn get_mut(&mut self) -> &mut S {
        &mut self.inner
    }

    /// Returns the inner service, consuming self.
    pub fn into_inner(self) -> S {
        self.inner
    }
}

impl<S> fmt::Debug for Include<S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Include").finish_non_exhaustive()
    }
}

impl<B, S> Service<Cx<B>> for Include<S>
where
    S: Service<Cx<B>, Response = Signal, Error = Infallible> + Clone,
{
    type Response = Signal;
    type Error = Infallible;
    type Future = IncludeFuture<S::Future, B, S>;

    #[inline]
    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    #[inline]
    fn call(&mut self, cx: Cx<B>) -> Self::Future {
        IncludeFuture::new(cx, self.inner.clone())
    }
}

/// A `tower::`[`Layer`] that produces a [`Include`] service.
#[derive(Debug, Default, Clone)]
pub struct IncludeLayer {}

impl IncludeLayer {
    /// Creates a new [`IncludeLayer`].
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}

impl<S> Layer<S> for IncludeLayer {
    type Service = Include<S>;

    fn layer(&self, inner: S) -> Self::Service {
        Include::new(inner)
    }
}

pin_project! {
    /// Response [`Future`] for [`Include`].
    pub struct IncludeFuture<F, B, S> {
        #[pin] kind: IncludeFutureKind<F, B, S>,
    }
}

pin_project! {
    #[project = IncludeFutureKindProj]
    enum IncludeFutureKind<F, B, S> {
        Resolve { #[pin] fut: F, cx: Cx<B>, inner: S, },
        Call { #[pin] fut: F, },
    }
}

impl<F, B, S> IncludeFuture<F, B, S>
where
    S: Service<Cx<B>, Response = Signal, Error = Infallible, Future = F>,
    F: Future<Output = Result<Signal, Infallible>>,
{
    /// Creates a new [`IncludeFuture`].
    pub fn new(cx: Cx<B>, mut inner: S) -> Self {
        // TODO. Check if req in cached.

        let fut = inner.call(cx);
        let kind = IncludeFutureKind::Call { fut };
        Self { kind }
    }
}

impl<F, B, S> fmt::Debug for IncludeFuture<F, B, S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("IncludeFuture").finish_non_exhaustive()
    }
}

impl<F, B, S> Future for IncludeFuture<F, B, S>
where
    S: Service<Cx<B>, Response = Signal, Error = Infallible, Future = F>,
    F: Future<Output = Result<Signal, Infallible>>,
{
    type Output = Result<Signal, Infallible>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.project();

        let signal = match this.kind.project() {
            IncludeFutureKindProj::Resolve { .. } => todo!(),
            IncludeFutureKindProj::Call { fut } => ready!(fut.poll(cx)),
        };

        Poll::Ready(signal)
    }
}

#[cfg(test)]
mod test {
    use tower::Layer;

    use crate::handler::HandlerService;
    use crate::middleware::IncludeLayer;
    use crate::Client;

    async fn handler() {}

    #[test]
    #[cfg(feature = "client")]
    fn with_client() {
        let backend = crate::backend::HttpClient::default();
        let service = HandlerService::new(handler, ());
        let layered = IncludeLayer::new().layer(service);
        let _ = Client::new(backend, layered);
    }

    #[test]
    #[cfg(feature = "driver")]
    fn with_driver() {
        let backend = crate::backend::BrowserPool::default();
        let service = HandlerService::new(handler, ());
        let layered = IncludeLayer::new().layer(service);
        let _ = Client::new(backend, layered);
    }
}
