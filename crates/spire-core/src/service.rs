//! Adapters between [`tower`] services and spire's traits.
//!
//! - [`ServiceBackend`] turns a `Service<()>` that yields clients into a
//!   [`Backend`].
//! - [`ServiceClient`] turns a `Service<Request>` into a [`Client`].
//! - [`ServiceWorker`] turns a `Service<Context<C>>` into a [`Worker`].
//!
//! Wrapping services lets tower middleware such as timeouts, rate limits
//! and concurrency limits compose with any part of a crawl.

use std::convert::Infallible;
use std::future::Future;

use tower::{Service, ServiceExt};

use crate::backend::{Backend, Client};
use crate::context::Context;
use crate::request::{Request, Response};
use crate::worker::{IntoOutcome, Outcome, Worker};
use crate::{BoxError, Error, ErrorKind, Result};

/// A [`Backend`] backed by a `Service<()>` that yields clients.
///
/// The service is cloned for every connection.
#[derive(Debug, Clone)]
pub struct ServiceBackend<S>(pub S);

impl<S, C> Backend for ServiceBackend<S>
where
    S: Service<(), Response = C> + Clone + Send + Sync + 'static,
    S::Error: Into<BoxError>,
    S::Future: Send,
    C: Client,
{
    type Client = C;

    fn connect(&self) -> impl Future<Output = Result<C>> + Send {
        let service = self.0.clone();
        async move {
            service
                .oneshot(())
                .await
                .map_err(|error| Error::from_boxed(ErrorKind::Backend, error))
        }
    }
}

/// A [`Client`] backed by a `Service<Request>`.
#[derive(Debug, Clone)]
pub struct ServiceClient<S>(pub S);

impl<S> Client for ServiceClient<S>
where
    S: Service<Request, Response = Response> + Send + 'static,
    S::Error: Into<BoxError>,
    S::Future: Send,
{
    async fn fetch(self, request: Request) -> Result<Response> {
        self.0
            .oneshot(request)
            .await
            .map_err(|error| Error::from_boxed(ErrorKind::Http, error))
    }
}

/// A [`Worker`] backed by an infallible `Service<Context<C>>`.
///
/// The service is cloned for every request.
#[derive(Debug, Clone)]
pub struct ServiceWorker<S>(pub S);

impl<C, S> Worker<C> for ServiceWorker<S>
where
    C: Send + 'static,
    S: Service<Context<C>, Error = Infallible> + Clone + Send + Sync + 'static,
    S::Response: IntoOutcome,
    S::Future: Send,
{
    fn handle(&self, cx: Context<C>) -> impl Future<Output = Outcome> + Send {
        let service = self.0.clone();
        async move {
            match service.oneshot(cx).await {
                Ok(response) => response.into_outcome(),
                Err(never) => match never {},
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use tower::service_fn;

    use super::*;
    use crate::backend::Noop;
    use crate::frontier::Seed;

    fn request() -> Request {
        Seed::get("https://example.com/").unwrap().into_request()
    }

    #[tokio::test]
    async fn adapters_round_trip() {
        let client = ServiceClient(service_fn(|_: Request| async {
            Ok::<_, Infallible>(Response::default())
        }));
        let backend = ServiceBackend(service_fn(move |()| {
            let client = client.clone();
            async move { Ok::<_, Infallible>(client) }
        }));
        let response = backend
            .connect()
            .await
            .unwrap()
            .fetch(request())
            .await
            .unwrap();
        assert!(response.status().is_success());

        let worker = ServiceWorker(service_fn(|cx: Context<Noop>| async move {
            Ok::<_, Infallible>(cx.fetch().await.map(drop))
        }));
        let outcome = worker.handle(Context::new(request(), Noop)).await;
        assert!(matches!(outcome, Outcome::Continue));
    }

    #[tokio::test]
    async fn client_errors_are_http_errors() {
        let client = ServiceClient(service_fn(|_: Request| async {
            Err::<Response, _>(std::io::Error::other("reset"))
        }));
        let error = client.fetch(request()).await.unwrap_err();
        assert_eq!(error.kind(), ErrorKind::Http);
    }
}
