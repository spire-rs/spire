use std::convert::Infallible;

use tower::{Service, ServiceExt};

use crate::backend::Backend;
use crate::context::{Context, Request, Signal};
use crate::dataset::{Dataset, Datasets};
use crate::Result;

pub struct Runner<B, S> {
    pub(crate) datasets: Datasets,
    pub(crate) backend: B,
    pub(crate) service: S,
}

impl<B, S> Runner<B, S> {
    pub fn new(backend: B, service: S, datasets: Datasets) -> Self
    where
        B: Backend,
        S: Service<Context<B>, Response = Signal, Error = Infallible>,
    {
        // Initialize queue's inner dataset.
        let _ = datasets.get::<Request>();

        Self {
            backend,
            service,
            datasets,
        }
    }

    pub async fn try_run(&self) -> Result<()> {
        let queue = self.datasets.get::<Request>();
        let request = queue.get().await?;

        // TODO: Wait until available and retry.
        // let _ = self.try_call(request.unwrap());

        todo!()
    }

    async fn try_call(&self, request: Request) -> Result<()>
    where
        B: Backend,
        S: Service<Context<B>, Response = Signal, Error = Infallible> + Clone,
    {
        let backend = self.backend.clone();
        let datasets = self.datasets.clone();
        let cx = Context::new(backend, datasets, request);

        let oneshot = self.service.clone().oneshot(cx);
        let signal = oneshot.await.unwrap();

        Ok(())
    }

    async fn process_signal(&self, signal: Signal) -> Result<()> {
        todo!()
    }
}
