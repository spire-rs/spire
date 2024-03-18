use futures::stream::StreamExt;

use crate::backend::daemon::metric::{StatRouter, Stats};
use crate::backend::{Backend, Router};
use crate::context::{Context as Cx, IntoSignal, Request, Signal};
use crate::dataset::Datasets;
use crate::Result;

pub struct Runner<B, S> {
    pub(crate) service: StatRouter<S>,
    pub datasets: Datasets,
    pub(crate) backend: B,
}

impl<B, S> Runner<B, S> {
    // TODO: Use Backend?
    pub fn new(backend: B, inner: S) -> Self
    where
        B: Backend,
        S: Router<B>,
    {
        Self {
            service: StatRouter::new(inner, Stats::default()),
            datasets: Datasets::default(),
            backend,
        }
    }

    pub fn stats(&self) -> Stats {
        todo!()
    }

    pub async fn run_until_empty(&self) -> Result<usize>
    where
        B: Backend,
        S: Router<B>,
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
        B: Backend,
        S: Router<B>,
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
        B: Backend,
        S: Router<B>,
    {
        match request {
            Ok(x) => self.call_service(x).await,
            Err(x) => self.notify_signal(x.into_signal()),
        }
    }

    async fn call_service(&self, request: Request)
    where
        B: Backend,
        S: Router<B>,
    {
        let backend = self.backend.clone();
        let datasets = self.datasets.clone();
        let cx = Cx::new(request, backend, datasets);

        let clone = self.service.clone();
        let signal = clone.route(cx).await;
        self.notify_signal(signal);
    }

    /// Applies the signal to the subsequent requests.
    fn notify_signal(&self, signal: Signal) {
        match signal {
            Signal::Continue => {}
            Signal::Skip => {}
            Signal::Wait(_, _) => {}
            Signal::Repeat(_, _) => {}
            Signal::Stop(_, _) => {}
        }

        todo!()
    }
}
