use futures::stream::StreamExt;

use crate::backend::{Backend, Worker};
use crate::context::{Context, Tag, Task};
use crate::context::{IntoSignal, Request, Signal};
use crate::dataset::Datasets;
use crate::Result;

pub struct Runner<B, W> {
    pub(crate) service: W,
    pub(crate) datasets: Datasets,
    pub(crate) backend: B,
}

// TODO: Rem deferred/blocked.

impl<B, W> Runner<B, W> {
    pub fn new(backend: B, inner: W) -> Self
    where
        B: Backend,
        W: Worker<B>,
    {
        Self {
            service: inner,
            datasets: Datasets::new(),
            backend,
        }
    }

    pub async fn run_until_empty(&self) -> Result<usize>
    where
        B: Backend,
        W: Worker<B>,
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
        W: Worker<B>,
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
        W: Worker<B>,
    {
        match request {
            Ok(x) => self.call_service(x).await,
            Err(x) => self.notify_signal(x.into_signal(), Tag::Fallback),
        }
    }

    async fn call_service(&self, request: Request)
    where
        B: Backend,
        W: Worker<B>,
    {
        let backend = self.backend.clone();
        let datasets = self.datasets.clone();
        let owner = request.tag().clone();

        let cx = Context::new(request, backend, datasets);
        let signal = self.service.clone().invoke(cx).await;
        self.notify_signal(signal, owner);
    }

    /// Applies the signal to the subsequent requests.
    fn notify_signal(&self, signal: Signal, owner: Tag) {
        // TODO: Add Ok/Err counter.
        let _ = match &signal {
            Signal::Continue | Signal::Wait(..) => false,
            Signal::Skip | Signal::Hold(..) | Signal::Fail(..) => true,
        };

        match signal {
            // Signal::Wait(x, t) | Signal::Hold(x, t) => self.apply_defer(owner, x, t),
            // Signal::Fail(x, e) => self.apply_block(owner, x, e),
            _ => { /* Ignore */ }
        };
    }
}
