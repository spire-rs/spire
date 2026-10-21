use std::fmt;
use std::future::Future;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use http::{HeaderName, HeaderValue};

use super::{Middleware, Next};
use crate::context::Context;
use crate::worker::Outcome;

/// A [`Middleware`] setting a header on every request that lacks it, e.g. a
/// user agent. A header the seed already has is kept.
///
/// [`rotate`](Self::rotate) cycles through several values, one per request.
/// Clones share their place in the cycle.
///
/// # Examples
///
/// ```
/// use spire_core::http::HeaderValue;
/// use spire_core::http::header::{ACCEPT_LANGUAGE, USER_AGENT};
/// use spire_core::worker::middleware::SetHeader;
///
/// let language = SetHeader::new(ACCEPT_LANGUAGE, HeaderValue::from_static("en"));
/// let agents = SetHeader::rotate(USER_AGENT, [
///     HeaderValue::from_static("MyBot/1.0 (a)"),
///     HeaderValue::from_static("MyBot/1.0 (b)"),
/// ]);
/// ```
#[derive(Clone)]
pub struct SetHeader {
    name: HeaderName,
    values: Arc<[HeaderValue]>,
    next: Arc<AtomicUsize>,
}

impl SetHeader {
    /// Sets `name` to `value`.
    #[must_use]
    pub fn new(name: HeaderName, value: HeaderValue) -> Self {
        Self::rotate(name, [value])
    }

    /// Sets `name` to each of `values` in turn, one per request.
    ///
    /// # Panics
    ///
    /// Panics if `values` is empty.
    #[must_use]
    pub fn rotate(name: HeaderName, values: impl IntoIterator<Item = HeaderValue>) -> Self {
        let values: Arc<[HeaderValue]> = values.into_iter().collect();
        assert!(
            !values.is_empty(),
            "`SetHeader::rotate` needs a value for `{name}`"
        );
        Self {
            name,
            values,
            next: Arc::default(),
        }
    }

    fn value(&self) -> HeaderValue {
        let index = self.next.fetch_add(1, Ordering::Relaxed) % self.values.len();
        self.values[index].clone()
    }
}

impl<C: Send + 'static> Middleware<C> for SetHeader {
    fn handle(&self, mut cx: Context<C>, next: Next<C>) -> impl Future<Output = Outcome> + Send {
        let headers = cx.request_mut().headers_mut();
        if !headers.contains_key(&self.name) {
            headers.insert(self.name.clone(), self.value());
        }
        next.run(cx)
    }
}

impl fmt::Debug for SetHeader {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SetHeader")
            .field("name", &self.name)
            .field("values", &self.values.len())
            .finish_non_exhaustive()
    }
}
