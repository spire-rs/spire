use std::cmp::max;
use std::collections::hash_map::Entry;
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use futures::StreamExt;

use crate::backend::{Backend, Worker};
use crate::context::{Context, Tag, TagQuery, Task};
use crate::context::{IntoSignal, Request, Signal};
use crate::dataset::{Dataset, Datasets};
use crate::process::notify::TagData;
use crate::Result;

/// TODO.
pub struct Runner<B, W> {
    pub service: W,
    pub datasets: Datasets,
    pub backend: B,

    pub initial: Mutex<Vec<Request>>,
    pub limit: AtomicUsize,
    pub notify: TagData,
}

impl<B, W> Runner<B, W> {
    /// Creates a new [`Runner`].
    pub fn new(backend: B, inner: W) -> Self
    where
        B: Backend,
        W: Worker<B::Client>,
    {
        Self {
            service: inner,
            datasets: Datasets::new(),
            backend,

            initial: Mutex::new(Vec::new()),
            limit: AtomicUsize::new(8),
            notify: TagData::new(),
        }
    }

    pub async fn run_until_empty(&self) -> Result<usize>
    where
        B: Backend,
        W: Worker<B::Client>,
    {
        let mut total = 0;
        loop {
            match self.run_until_signal().await? {
                x if x > 0 => total += x,
                _ => break,
            }

            #[cfg(feature = "tracing")]
            tracing::info!(processed = total);
        }

        Ok(total)
    }

    async fn run_until_signal(&self) -> Result<usize>
    where
        B: Backend,
        W: Worker<B::Client>,
    {
        // TODO: Requests are lost in case of error.
        // Use tokio::Mutex instead?

        let mut requests: Vec<_> = {
            let mut initial = self.initial.lock().unwrap();
            initial.drain(..).collect()
        };

        let dataset = self.datasets.get::<Request>();
        while let Some(request) = requests.pop() {
            dataset.write(request).await?;
        }

        let try_call_service = |x: Result<Request>| async {
            match x {
                Ok(x) => self.run_once(x).await,
                Err(x) => self.notify.notify(x.into_signal(), Tag::default()),
            };
        };

        // TODO: Abortable.
        let stream = dataset
            .into_stream()
            .map(try_call_service)
            .buffered(self.limit.load(Ordering::SeqCst))
            .count();

        Ok(stream.await)
    }

    pub async fn run_once(&self, request: Request) -> Result<()>
    where
        B: Backend,
        W: Worker<B::Client>,
    {
        match self.call_service(request).await {
            Ok((w, t)) => self.notify.notify(w, t),
            Err(x) => self.notify.notify(x.into_signal(), Tag::default()),
        }
    }

    async fn call_service(&self, request: Request) -> Result<(Signal, Tag)>
    where
        B: Backend,
        W: Worker<B::Client>,
    {
        let owner = request.tag().clone();
        let datasets = self.datasets.clone();

        let client: B::Client = self.backend.client().await?;
        let cx: Context<B::Client> = Context::new(request, client, datasets);
        let signal = self.service.clone().invoke(cx).await;

        Ok((signal, owner))
    }

    // // Applies the signal to the subsequent requests.
    // fn notify_signal(&self, signal: Signal, owner: Tag) {
    //     match signal {
    //         Signal::Wait(x, t) | Signal::Hold(x, t) => self.apply_defer(x, owner, t),
    //         Signal::Fail(x, _) => self.apply_block(x, owner),
    //         _ => { /* Ignore */ }
    //     };
    // }

    // // Returns the currently applied [`Instant`].
    // fn find_defer(&self, tag: &Tag) -> Option<Instant> {
    //     let defer = self.defer.lock().unwrap();
    //     defer
    //         .get(tag)
    //         .map_or_else(|| defer.get(&Tag::Fallback), Some)
    //         .copied()
    // }
    //
    // // Returns `true` if the given [`Tag`] is blocked.
    // fn find_block(&self, tag: &Tag) -> bool {
    //     // TODO: All, not just unknown.
    //     let block = self.block.lock().unwrap();
    //     block.contains(tag)
    // }
}
