//! Middleware, which runs around a [`Worker`] for every request.
//!
//! A [`Middleware`] receives the [`Context`] before any handler, and the
//! [`Next`] worker to pass it on to. Before passing it on, it can change the
//! request, e.g. add headers, or answer without fetching. Around, it can
//! check the response's head: fetched once, it is shared with the handlers.
//! After, it can change the [`Outcome`].
//!
//! [`WorkerExt::layer`] wraps a worker, such as a router, in middleware. The
//! middleware added last runs first.
//!
//! # Examples
//!
//! ```
//! use std::time::Duration;
//!
//! use spire_core::backend::Noop;
//! use spire_core::http::http::HeaderValue;
//! use spire_core::http::http::header::USER_AGENT;
//! use spire_core::worker::middleware::{self, Next, SetHeader};
//! use spire_core::worker::{Outcome, WorkerExt, from_fn};
//! use spire_core::crawler::Crawler;
//! use spire_core::{Context, Result};
//!
//! /// Retries pages that answer with a captcha instead of content.
//! async fn captcha(cx: Context<Noop>, next: Next<Noop>) -> Result<Outcome> {
//!     if cx.response().await?.headers().contains_key("x-captcha") {
//!         return Ok(Outcome::Retry { after: Some(Duration::from_secs(60)) });
//!     }
//!     Ok(next.run(cx).await)
//! }
//!
//! let worker = from_fn(|_: Context<Noop>| async {})
//!     .layer(middleware::from_fn(captcha))
//!     .layer(SetHeader::new(USER_AGENT, HeaderValue::from_static("MyBot/1.0")));
//! let crawler = Crawler::new(Noop, worker);
//! ```

mod header;
mod revalidate;

use std::fmt;
use std::future::Future;
use std::sync::Arc;

use futures::future::BoxFuture;
pub use header::SetHeader;
pub use revalidate::{InMemValidators, Revalidate, Validator, Validators};

use crate::context::Context;
use crate::worker::{IntoOutcome, Outcome, Worker};

/// Runs around a [`Worker`] for every request, see the [module](self).
pub trait Middleware<C>: Send + Sync + 'static {
    /// Handles the request in `cx`, passing it on to `next` or not.
    fn handle(&self, cx: Context<C>, next: Next<C>) -> impl Future<Output = Outcome> + Send;
}

/// The rest of the chain a [`Middleware`] passes a request on to: further
/// middleware, then the worker.
pub struct Next<C> {
    worker: Arc<dyn ErasedWorker<C>>,
}

impl<C> Next<C> {
    /// Runs the rest of the chain on `cx`.
    pub async fn run(self, cx: Context<C>) -> Outcome
    where
        C: Send + 'static,
    {
        self.worker.handle(cx).await
    }
}

impl<C> fmt::Debug for Next<C> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Next").finish_non_exhaustive()
    }
}

/// A [`Worker`] behind a pointer, so [`Next`] does not name its type.
trait ErasedWorker<C>: Send + Sync + 'static {
    fn handle(&self, cx: Context<C>) -> BoxFuture<'_, Outcome>;
}

impl<C: Send + 'static, W: Worker<C>> ErasedWorker<C> for W {
    fn handle(&self, cx: Context<C>) -> BoxFuture<'_, Outcome> {
        Box::pin(Worker::handle(self, cx))
    }
}

/// Wraps workers in [`Middleware`].
pub trait WorkerExt: Sized {
    /// Wraps this worker in `middleware`, which runs before it, and before
    /// any middleware added earlier.
    fn layer<C, M>(self, middleware: M) -> Layered<M, C>
    where
        C: Send + 'static,
        Self: Worker<C>,
        M: Middleware<C>,
    {
        Layered {
            middleware,
            worker: Arc::new(self),
        }
    }
}

impl<W> WorkerExt for W {}

/// A [`Worker`] wrapped in [`Middleware`], created by [`WorkerExt::layer`].
pub struct Layered<M, C> {
    middleware: M,
    worker: Arc<dyn ErasedWorker<C>>,
}

impl<M, C> Worker<C> for Layered<M, C>
where
    M: Middleware<C>,
    C: Send + 'static,
{
    fn handle(&self, cx: Context<C>) -> impl Future<Output = Outcome> + Send {
        let next = Next {
            worker: Arc::clone(&self.worker),
        };
        self.middleware.handle(cx, next)
    }
}

impl<M: Clone, C> Clone for Layered<M, C> {
    fn clone(&self) -> Self {
        Self {
            middleware: self.middleware.clone(),
            worker: Arc::clone(&self.worker),
        }
    }
}

impl<M: fmt::Debug, C> fmt::Debug for Layered<M, C> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Layered")
            .field("middleware", &self.middleware)
            .finish_non_exhaustive()
    }
}

/// Creates a [`Middleware`] from an async function of the [`Context`] and
/// the [`Next`] worker, returning anything [`IntoOutcome`], e.g.
/// `Result<Outcome>`.
pub const fn from_fn<F>(f: F) -> FromFn<F> {
    FromFn(f)
}

/// A [`Middleware`] backed by an async function, created by [`from_fn`].
#[derive(Clone, Copy)]
pub struct FromFn<F>(F);

impl<C, F, Fut> Middleware<C> for FromFn<F>
where
    C: Send + 'static,
    F: Fn(Context<C>, Next<C>) -> Fut + Send + Sync + 'static,
    Fut: Future<Output: IntoOutcome> + Send,
{
    fn handle(&self, cx: Context<C>, next: Next<C>) -> impl Future<Output = Outcome> + Send {
        let future = (self.0)(cx, next);
        async move { future.await.into_outcome() }
    }
}

impl<F> fmt::Debug for FromFn<F> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FromFn").finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests;
