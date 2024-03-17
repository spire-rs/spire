use std::convert::Infallible;

use futures::stream::StreamExt;
use tower::{Service, ServiceBuilder, ServiceExt};
use tower::load::Load;

use crate::{Error, Result};
use crate::context::{Context, IntoSignal, Request, Response, Signal};
use crate::dataset::Datasets;
use crate::process::metric::{Metrics, MetricsLayer, Stats};
use crate::process::signal::{Signals, SignalsLayer};

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
            .map(|x| async { self.try_call_service(x).await })
            .buffer_unordered(8)
            .count();

        Ok(stream.await)
    }

    async fn try_call_service(&self, request: Result<Request>)
    where
        B: Service<Request, Response = Response, Error = Error> + Clone,
        S: Service<Context<B>, Response = Signal, Error = Infallible> + Clone,
    {
        match request {
            Ok(x) => self.call_service(x).await,
            Err(x) => self.notify_signal(x.into_signal()),
        }
    }

    async fn call_service(&self, request: Request)
    where
        B: Service<Request, Response = Response, Error = Error> + Clone,
        S: Service<Context<B>, Response = Signal, Error = Infallible> + Clone,
    {
        let backend = self.backend.clone();
        let datasets = self.datasets.clone();
        let cx = Context::new(request, backend, datasets);

        let oneshot = self.service.clone().oneshot(cx);
        oneshot.await.unwrap()
    }

    fn notify_signal(&self, signal: Signal) {
        self.service.notify_signal(signal)
    }
}
