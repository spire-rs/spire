use std::convert::Infallible;

use tower::{Layer, Service, ServiceExt};

use crate::context::{Context, Request, Response, Signal};
use crate::dataset::{Dataset, Datasets};
use crate::process::metric::{Metrics, MetricsLayer};
use crate::{Error, Result};

pub struct Runner<B, S> {
    // TODO: Error handler.
    pub(crate) service: Metrics<S>,
    pub(crate) datasets: Datasets,
    pub(crate) backend: B,
}

impl<B, S> Runner<B, S> {
    pub fn new(backend: B, service: S) -> Self
    where
        B: Service<Request, Response = Response, Error = Error> + Clone,
        S: Service<Context<B>, Response = Signal, Error = Infallible>,
    {
        let layer = MetricsLayer::default();
        Self {
            service: layer.layer(service),
            datasets: Datasets::default(),
            backend,
        }
    }

    async fn try_poll(&self) -> Result<()> {
        let queue = self.datasets.get::<Request>();
        let request = queue.get().await?;

        // TODO: Wait until available and retry.
        // let _ = self.try_call(request.unwrap());
        // let _ = self.signal();

        todo!()
    }

    async fn try_call(&self, request: Request) -> Signal
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

    async fn signal(&self, signal: Signal) -> Result<()> {
        todo!()
    }
}
