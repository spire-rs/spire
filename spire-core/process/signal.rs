use std::convert::Infallible;
use std::fmt;
use std::future::Future;
use std::pin::Pin;
use std::task::{ready, Context, Poll};

use pin_project_lite::pin_project;
use tower::load::Load;
use tower::{Layer, Service};

use crate::context::{Context as Cx, Signal, Tag, Task};

/// TODO.
#[derive(Clone)]
pub struct Signals<S> {
    status: StatusLock,
    inner: S,
}

// enum Status {
//     Block,
//     WaitUntil(Instant),
// }

#[derive(Default, Clone)]
struct StatusLock {
    // lock: Arc<Mutex<HashMap<TagQuery, Status>>>,
}

impl<S> Signals<S> {
    /// Creates a new [`Signals`] service.
    pub fn new(inner: S) -> Self {
        let status = StatusLock::default();
        Self { inner, status }
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

    /// Applies the signal to the subsequent requests.
    pub fn notify_signal(&self, signal: Signal) {
        match signal {
            Signal::Continue => {}
            Signal::Skip => {}
            Signal::Wait(_, _) => {}
            Signal::Repeat(_, _) => {}
            Signal::Stop(_, _) => {}
        }

        todo!()
    }

    /// TODO.
    pub fn find_queries(&self, tag: &Tag) {
        todo!()
    }
}

impl<S> fmt::Debug for Signals<S>
where
    S: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&self.inner, f)
    }
}

impl<B, S> Service<Cx<B>> for Signals<S>
where
    S: Service<Cx<B>, Response = Signal, Error = Infallible>,
{
    type Response = ();
    type Error = Infallible;
    type Future = SignalsFuture<S::Future>;

    #[inline]
    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    #[inline]
    fn call(&mut self, cx: Cx<B>) -> Self::Future {
        let _ = self.find_queries(cx.peek().tag());
        // TODO: Apply queries.
        SignalsFuture::new(self.inner.call(cx))
    }
}

impl<S> Load for Signals<S>
where
    S: Load,
{
    type Metric = S::Metric;

    fn load(&self) -> Self::Metric {
        self.inner.load()
    }
}

pin_project! {
    /// Response [`Future`] for [`Signals`].
    pub struct SignalsFuture<F> {
        #[pin] future: F,
    }
}

impl<F> SignalsFuture<F> {
    /// Creates a new [`SignalsFuture`].
    pub(crate) fn new(future: F) -> Self {
        Self { future }
    }
}

impl<F> Future for SignalsFuture<F>
where
    F: Future<Output = Result<Signal, Infallible>>,
{
    type Output = Result<(), Infallible>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.project();

        // TODO: Process Signal.
        let _ = ready!(this.future.poll(cx))?;
        Poll::Ready(Ok(()))
    }
}

#[derive(Debug, Default, Clone)]
pub struct SignalsLayer {
    _marker: (),
}

impl<S> Layer<S> for SignalsLayer {
    type Service = Signals<S>;

    fn layer(&self, inner: S) -> Self::Service {
        Signals::new(inner)
    }
}
