use std::collections::HashMap;
use std::time::Instant;

use futures::stream::StreamExt;

use crate::backend::{Backend, Worker};
use crate::context::{Context as Cx, IntoSignal, Request, Signal, Tag, TagQuery, Task};
use crate::dataset::Datasets;
use crate::Result;

pub struct Runner<B, W> {
    pub(crate) service: W,
    pub(crate) datasets: Datasets,
    pub(crate) backend: B,

    // Fallback means all not-yet encountered tags.
    pub(crate) delays: HashMap<Tag, Instant>,
}

impl<B, W> Runner<B, W> {
    pub fn new(backend: B, inner: W) -> Self
    where
        B: Backend,
        W: Worker<B>,
    {
        Self {
            service: inner,
            datasets: Datasets::default(),
            backend,
            delays: HashMap::default(),
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
        let cx = Cx::new(request, backend, datasets);

        let clone = self.service.clone();
        let signal = clone.invoke(cx).await;
        self.notify_signal(signal, owner);
    }

    fn find_queries(&self) -> Vec<TagQuery> {
        todo!()
    }

    /// Applies the signal to the subsequent requests.
    fn notify_signal(&self, signal: Signal, owner: Tag) {
        for x in self.find_queries() {
            let _ = x.is_match(&Tag::Fallback, &Tag::Fallback);
        }

        match signal {
            // TODO: Add Ok counter.
            Signal::Continue => {}
            // TODO: Add Err counter.
            Signal::Skip => {}
            Signal::Wait(_, _) => {}
            Signal::Repeat(_, _) => {}
            Signal::Stop(_, _) => {}
        }

        // TODO.
    }
}
