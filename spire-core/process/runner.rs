use std::cmp::max;
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use futures::stream::StreamExt;

use crate::backend::{Backend, Worker};
use crate::context::{Context, Tag, TagQuery, Task};
use crate::context::{IntoSignal, Request, Signal};
use crate::dataset::{Dataset, Datasets};
use crate::Result;

pub struct Runner<B, W> {
    pub(crate) service: W,
    pub(crate) datasets: Datasets,
    pub(crate) backend: B,

    inits: Mutex<Vec<Request>>,
    limit: AtomicUsize,

    // Fallback means all not-yet encountered tags.
    defer: Mutex<HashMap<Tag, Instant>>,
    block: Mutex<HashSet<Tag>>,
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

            inits: Mutex::new(Vec::new()),
            limit: AtomicUsize::new(8),

            defer: Mutex::new(HashMap::new()),
            block: Mutex::new(HashSet::new()),
        }
    }

    pub fn with_initial_request(&self, request: Request) {
        let mut initial = self.inits.lock().unwrap();
        initial.push(request);
    }

    pub fn with_concurrency_limit(&self, limit: usize) {
        self.limit.store(max(limit, 1), Ordering::SeqCst);
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
        let mut requests: Vec<_> = {
            let mut initial = self.inits.lock().unwrap();
            initial.drain(..).collect()
        };

        let dataset = self.datasets.get::<Request>();
        while let Some(request) = requests.pop() {
            dataset.add(request).await?;
        }

        // TODO.
        let try_call_service = |x: Result<Request>| async {
            let (signal, owner) = match x {
                Ok(x) => self
                    .call_service(x)
                    .await
                    .unwrap_or_else(|x| (x.into_signal(), Tag::default())),
                Err(x) => (x.into_signal(), Tag::default()),
            };

            self.notify_signal(signal, owner).await;
        };

        let stream = dataset
            .into_stream()
            .map(try_call_service)
            .buffered(self.limit.load(Ordering::SeqCst))
            .count();

        Ok(stream.await)
    }

    pub async fn call_service(&self, request: Request) -> Result<(Signal, Tag)>
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

    /// Applies the signal to the subsequent requests.
    async fn notify_signal(&self, signal: Signal, owner: Tag) {
        // TODO: Add Ok/Err counter.
        match signal {
            Signal::Wait(x, t) | Signal::Hold(x, t) => self.apply_defer(owner, x, t),
            Signal::Fail(x, e) => self.apply_block(owner, x),
            _ => { /* Ignore */ }
        };
    }

    // TODO.
    fn apply_defer(&self, owner: Tag, query: TagQuery, duration: Duration) {
        let until = Instant::now() + duration;

        let defer = self.defer.lock().unwrap();
        match query {
            TagQuery::Owner => {}
            TagQuery::Single(_) => {}

            TagQuery::Every => {}
            TagQuery::List(_) => {}
        }

        todo!()
    }

    // TODO.
    fn find_defer(&self, owner: &Tag) -> Instant {
        let now = Instant::now();

        let defer = self.defer.lock().unwrap();
        // let until = match defer.get(owner).cloned() {
        //     None => defer.get(&Tag::Fallback).cloned(),
        //     Some(x) => Some(x),
        // };

        let until = defer
            .get(owner)
            .copied()
            .map_or_else(|| defer.get(&Tag::Fallback).cloned(), Some);

        until.unwrap_or(now)
    }

    // TODO.
    fn apply_block(&self, owner: Tag, query: TagQuery) {
        let block = |x| {
            let mut guard = self.block.lock().unwrap();
            guard.insert(x);
        };

        match query {
            TagQuery::Owner => block(owner),
            TagQuery::Every => block(Tag::default()),
            TagQuery::Single(x) => block(x),
            TagQuery::List(x) => x.into_iter().for_each(block),
        }
    }

    // TODO.
    fn find_block(&self, owner: &Tag) -> bool {
        let block = self.block.lock().unwrap();
        block.contains(owner)
    }
}
