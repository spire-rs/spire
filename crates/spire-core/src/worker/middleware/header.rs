use std::future::Future;

use http::{HeaderName, HeaderValue};

use super::{Middleware, Next};
use crate::context::Context;
use crate::worker::Outcome;

/// A [`Middleware`] setting a header on every request that lacks it, e.g. a
/// contact address. A header the seed already has is kept.
///
/// Headers that say who a request comes from, such as a user agent that
/// should match a proxy, belong to an [`Identity`](crate::identity::Identity),
/// which keeps them together with its proxy and cookies.
///
/// # Examples
///
/// ```
/// use spire_core::http::HeaderValue;
/// use spire_core::http::header::{ACCEPT_LANGUAGE, FROM};
/// use spire_core::worker::middleware::SetHeader;
///
/// let language = SetHeader::new(ACCEPT_LANGUAGE, HeaderValue::from_static("en"));
/// let contact = SetHeader::new(FROM, HeaderValue::from_static("crawler@example.com"));
/// ```
#[derive(Debug, Clone)]
pub struct SetHeader {
    name: HeaderName,
    value: HeaderValue,
}

impl SetHeader {
    /// Sets `name` to `value`.
    #[must_use]
    pub const fn new(name: HeaderName, value: HeaderValue) -> Self {
        Self { name, value }
    }
}

impl<C: Send + 'static> Middleware<C> for SetHeader {
    fn handle(&self, mut cx: Context<C>, next: Next<C>) -> impl Future<Output = Outcome> + Send {
        let headers = cx.request_mut().headers_mut();
        if !headers.contains_key(&self.name) {
            headers.insert(self.name.clone(), self.value.clone());
        }
        next.run(cx)
    }
}
