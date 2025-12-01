//! Client trait for fetching HTTP responses.

use tower::{Service, ServiceExt};

use crate::context::{Request, Response};
use crate::{Error, Result};

/// Core trait for fetching HTTP responses.
///
/// A `Client` handles individual HTTP requests and returns responses.
/// It is automatically implemented for cloneable Tower services that
/// handle [`Request`]s and return [`Result`]<[`Response`]>.
///
/// # Examples
///
/// ```no_run
/// use spire_core::backend::Client;
/// use spire_core::context::Request;
///
/// async fn fetch<C: Client>(client: C, request: Request) {
///     let response = client.resolve(request).await.unwrap();
///     // Process the response
/// }
/// ```
#[async_trait::async_trait]
pub trait Client: Send + Sized + 'static {
    /// Fetches the [`Response`] for the given request.
    async fn resolve(self, req: Request) -> Result<Response>;
}

#[async_trait::async_trait]
impl<S> Client for S
where
    S: Service<Request, Response = Response, Error = Error> + Send + 'static,
    S::Future: Send + 'static,
{
    #[inline]
    async fn resolve(mut self, req: Request) -> Result<Response> {
        let ready = self.ready().await?;
        ready.call(req).await
    }
}
