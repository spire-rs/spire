use std::convert::Infallible;

use futures::stream::StreamExt;
use tower::{Service, ServiceBuilder, ServiceExt};

use crate::context::{Context, IntoSignal, Request, Response, Signal};
use crate::dataset::Datasets;
use crate::process::metric::{Metrics, MetricsLayer};
use crate::{Error, Result};

pub struct Runner<B, S> {
    // TODO: Error handler.
    // TODO: Delay<S> wrapper.
    pub(crate) service: Metrics<S>,
    pub(crate) datasets: Datasets,
    pub(crate) backend: B,
}

impl<B, S> Runner<B, S> {
    pub fn new(backend: B, inner: S) -> Self
    where
        B: Service<Request, Response = Response, Error = Error> + Clone,
        S: Service<Context<B>, Response = Signal, Error = Infallible> + Clone,
    {
        let service = ServiceBuilder::default()
            .layer(MetricsLayer::default())
            .service(inner);

        Self {
            service,
            datasets: Datasets::default(),
            backend,
        }
    }

    pub async fn poll_until_empty(&self) -> Result<usize>
    where
        B: Service<Request, Response = Response, Error = Error> + Clone,
        S: Service<Context<B>, Response = Signal, Error = Infallible> + Clone,
    {
        let dataset = self.datasets.get::<Request>();

        // TODO: Repeat until returns zero?
        let stream = dataset
            .into_stream()
            .then(|x| async { self.call_service(x).await })
            .map(|x| async { self.notify_signal(x).await })
            .buffer_unordered(8)
            .count();

        Ok(stream.await)
    }

    async fn call_service(&self, request: Result<Request>) -> Signal
    where
        B: Service<Request, Response = Response, Error = Error> + Clone,
        S: Service<Context<B>, Response = Signal, Error = Infallible> + Clone,
    {
        match request {
            Ok(x) => self.try_call_service(x).await,
            Err(x) => x.into_signal(),
        }
    }

    async fn try_call_service(&self, request: Request) -> Signal
    where
        B: Service<Request, Response = Response, Error = Error> + Clone,
        S: Service<Context<B>, Response = Signal, Error = Infallible> + Clone,
    {
        let backend = self.backend.clone();
        let datasets = self.datasets.clone();
        let cx = Context::new(request, backend, datasets);

        let oneshot = self.service.clone().oneshot(cx);
        oneshot.await.expect("should be infallible")
    }

    async fn notify_signal(&self, signal: Signal) {
        // TODO.
    }
}
