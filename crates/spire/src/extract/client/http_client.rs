use derive_more::{Deref, DerefMut};

use crate::HttpClient;
use crate::context::Context;
use crate::extract::FromContextRef;

/// HttpBackend extractor for accessing the underlying HTTP client.
///
/// Extracts the HttpClient instance from the context, providing direct access
/// to the HTTP client for advanced operations, custom requests, or accessing
/// client configuration when using the reqwest backend.
///
/// This extractor is only available when using the reqwest backend feature.
///
/// # Examples
///
/// ```ignore
/// use spire::extract::client::HttpBackend;
///
/// async fn handler(HttpBackend(client): HttpBackend) {
///     // Access the underlying HttpClient for custom operations
///     // This gives you the full HttpClient instance
/// }
/// ```
#[cfg(feature = "reqwest")]
#[cfg_attr(docsrs, doc(cfg(feature = "reqwest")))]
#[derive(Debug, Clone, Deref, DerefMut)]
pub struct HttpBackend(pub HttpClient);

#[cfg(feature = "reqwest")]
#[spire_core::async_trait]
impl<S> FromContextRef<HttpClient, S> for HttpBackend
where
    S: Sync,
{
    type Rejection = std::convert::Infallible;

    async fn from_context_ref(
        cx: &Context<HttpClient>,
        _state: &S,
    ) -> Result<Self, Self::Rejection> {
        Ok(Self(cx.as_client_ref().clone()))
    }
}
