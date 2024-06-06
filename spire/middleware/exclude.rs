use std::convert::Infallible;
use std::fmt;
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll, ready};

use pin_project_lite::pin_project;
use tower::{Layer, Service};

use crate::context::{Context as Cx, Signal};

/// Conditionally rejects [`Request`]s based on a retrieved `robots.txt` file.
///
/// [`Request`]: crate::context::Request
#[derive(Clone)]
#[must_use = "services do nothing unless you `.poll_ready` or `.call` them"]
pub struct Exclude<S> {
    inner: S,
}

impl<S> Exclude<S> {
    /// Creates a new [`Exclude`] with a provided inner service.
    pub const fn new(inner: S) -> Self {
        Self { inner }
    }

    /// Returns a reference to the inner service.
    pub const fn get_ref(&self) -> &S {
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

impl<S> fmt::Debug for Exclude<S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Exclude").finish_non_exhaustive()
    }
}

impl<C, S> Service<Cx<C>> for Exclude<S>
where
    S: Service<Cx<C>, Response = Signal, Error = Infallible> + Clone,
{
    type Response = Signal;
    type Error = Infallible;
    type Future = ExcludeFuture<S::Future, C, S>;

    #[inline]
    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    #[inline]
    fn call(&mut self, cx: Cx<C>) -> Self::Future {
        ExcludeFuture::new(cx, self.inner.clone())
    }
}

/// A `tower::`[`Layer`] that produces a [`Exclude`] service.
#[derive(Debug, Default, Clone)]
#[must_use = "layers do nothing unless you `.layer` them"]
pub struct ExcludeLayer {}

impl ExcludeLayer {
    /// Creates a new [`ExcludeLayer`].
    pub fn new() -> Self {
        Self::default()
    }
}

impl<S> Layer<S> for ExcludeLayer {
    type Service = Exclude<S>;

    fn layer(&self, inner: S) -> Self::Service {
        Exclude::new(inner)
    }
}

pin_project! {
    /// Response [`Future`] for [`Exclude`].
    #[must_use = "futures do nothing unless you `.await` or poll them"]
    pub struct ExcludeFuture<F, C, S> {
        #[pin] kind: ExcludeFutureKind<F, C, S>,
    }
}

pin_project! {
    #[project = ExcludeFutureKindProj]
    enum ExcludeFutureKind<F, C, S> {
        Resolve { #[pin] fut: F, cx: Cx<C>, inner: S, },
        Call { #[pin] fut: F, },
    }
}

impl<F, C, S> ExcludeFuture<F, C, S>
where
    S: Service<Cx<C>, Response = Signal, Error = Infallible, Future = F>,
    F: Future<Output = Result<Signal, Infallible>>,
{
    /// Creates a new [`ExcludeFuture`].
    pub fn new(cx: Cx<C>, mut inner: S) -> Self {
        // TODO. Check if req in cached, use special dataset?.

        let fut = inner.call(cx);
        let kind = ExcludeFutureKind::Call { fut };
        Self { kind }
    }
}

impl<F, B, S> fmt::Debug for ExcludeFuture<F, B, S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ExcludeFuture").finish_non_exhaustive()
    }
}

impl<F, C, S> Future for ExcludeFuture<F, C, S>
where
    S: Service<Cx<C>, Response = Signal, Error = Infallible, Future = F>,
    F: Future<Output = Result<Signal, Infallible>>,
{
    type Output = Result<Signal, Infallible>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.project();

        let signal = match this.kind.project() {
            ExcludeFutureKindProj::Resolve { .. } => todo!(),
            ExcludeFutureKindProj::Call { fut } => ready!(fut.poll(cx)),
        };

        Poll::Ready(signal)
    }
}

#[cfg(test)]
mod test {
    use tower::Layer;

    use crate::Client;
    use crate::handler::HandlerService;
    use crate::middleware::ExcludeLayer;

    async fn handler() {}

    #[test]
    #[cfg(feature = "client")]
    fn with_client() {
        let backend = crate::backend::HttpClient::default();
        let service = HandlerService::new(handler, ());
        let layered = ExcludeLayer::new().layer(service);
        let _ = Client::new(backend, layered);
    }

    #[test]
    #[cfg(feature = "driver")]
    fn with_driver() {
        let backend = crate::backend::BrowserPool::default();
        let service = HandlerService::new(handler, ());
        let layered = ExcludeLayer::new().layer(service);
        let _ = Client::new(backend, layered);
    }
}
