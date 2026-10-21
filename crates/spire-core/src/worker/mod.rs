//! Workers, which handle fetched requests, and their [`Outcome`]s.

mod outcome;

use std::fmt;
use std::future::Future;

pub use outcome::{IntoOutcome, Outcome, Scope};

use crate::context::Context;

/// Handles one request of a crawl.
///
/// The worker receives a [`Context`] holding the request and a client for
/// it. It decides when, and whether, to fetch the response, may queue
/// further requests, and tells the crawler what to do next.
pub trait Worker<C>: Send + Sync + 'static {
    /// Handles the request in `cx`.
    fn handle(&self, cx: Context<C>) -> impl Future<Output = Outcome> + Send;
}

/// Creates a [`Worker`] from an async function.
///
/// # Examples
///
/// ```
/// use spire_core::backend::Noop;
/// use spire_core::worker::from_fn;
/// use spire_core::{Context, Result};
///
/// async fn handle(cx: Context<Noop>) -> Result<()> {
///     let response = cx.fetch().await?;
///     println!("status: {}", response.status());
///     Ok(())
/// }
///
/// let worker = from_fn(handle);
/// ```
pub const fn from_fn<F>(f: F) -> FromFn<F> {
    FromFn(f)
}

/// A [`Worker`] backed by an async function, created by [`from_fn`].
#[derive(Clone, Copy)]
pub struct FromFn<F>(F);

impl<C, F, Fut> Worker<C> for FromFn<F>
where
    C: Send + 'static,
    F: Fn(Context<C>) -> Fut + Send + Sync + 'static,
    Fut: Future<Output: IntoOutcome> + Send,
{
    fn handle(&self, cx: Context<C>) -> impl Future<Output = Outcome> + Send {
        let future = (self.0)(cx);
        async move { future.await.into_outcome() }
    }
}

impl<F> fmt::Debug for FromFn<F> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FromFn").finish_non_exhaustive()
    }
}
