use std::convert::Infallible;

use futures::stream::StreamExt;
use tower::load::Load;
use tower::{Service, ServiceBuilder, ServiceExt};

use crate::context::{Context, IntoSignal, Request, Response, Signal, Tag, Task};
use crate::dataset::Datasets;
use crate::process::metric::{Metrics, MetricsLayer, Stats};
use crate::process::signal::{Signals, SignalsLayer};
use crate::{Error, Result};

pub struct Runner<B, S> {
    pub(crate) service: Signals<Metrics<S>>,
    pub datasets: Datasets,
    pub(crate) backend: B,
}

impl<B, S> Runner<B, S> {
    pub fn new(backend: B, inner: S) -> Self
    where
        B: Service<Request, Response = Response, Error = Error> + Clone,
        S: Service<Context<B>, Response = Signal, Error = Infallible> + Clone,
    {
        let datasets = Datasets::default();
        let service = ServiceBuilder::default()
            .layer(SignalsLayer::default())
            .layer(MetricsLayer::default())
            .service(inner);

        Self {
            service,
            datasets,
            backend,
        }
    }

    pub fn stats(&self) -> Stats {
        self.service.get_ref().load()
    }

    pub async fn run_until_empty(&self) -> Result<usize>
    where
        B: Service<Request, Response = Response, Error = Error> + Clone,
        S: Service<Context<B>, Response = Signal, Error = Infallible> + Clone,
    {
        let mut total = 0;
        loop {
            match self.run_until_signal().await? {
                x if x > 0 => total += x,
                _ => break,
            }
        }

        Ok(total)
    }

    pub async fn run_until_signal(&self) -> Result<usize>
    where
        B: Service<Request, Response = Response, Error = Error> + Clone,
        S: Service<Context<B>, Response = Signal, Error = Infallible> + Clone,
    {
        let dataset = self.datasets.get::<Request>();

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
            Ok(x) => self.try_call_service(x).await.1,
            // TODO: Call error_handler if provided.
            Err(x) => x.into_signal(),
        }
    }

    async fn try_call_service(&self, request: Request) -> (Tag, Signal)
    where
        B: Service<Request, Response = Response, Error = Error> + Clone,
        S: Service<Context<B>, Response = Signal, Error = Infallible> + Clone,
    {
        let backend = self.backend.clone();
        let datasets = self.datasets.clone();
        let caller_tag = request.tag().clone();
        let cx = Context::new(request, backend, datasets);

        let oneshot = self.service.clone().oneshot(cx);
        (caller_tag, oneshot.await.unwrap())
    }

    async fn notify_signal(&self, signal: Signal) {
        self.service.notify_signal(signal).await
    }
}
