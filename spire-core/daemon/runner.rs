use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use futures::stream::StreamExt;
use futures::TryStreamExt;
use http_body::Body;

use crate::backend::{Backend, Worker};
use crate::context::{Context, Tag, TagQuery, Task};
use crate::context::{IntoSignal, Request, Signal};
use crate::dataset::{Dataset, Datasets};
use crate::Result;

pub struct Runner<B, W> {
    pub(crate) service: W,
    pub(crate) datasets: Datasets,
    pub(crate) backend: B,

    // Fallback means all not-yet encountered tags.
    defer: Arc<Mutex<HashMap<Tag, Instant>>>,
    block: Arc<Mutex<HashSet<Tag>>>,
    inits: Arc<Mutex<Vec<Request>>>,
}

impl<B, W> Runner<B, W> {
    /// Creates a new [`Runner`].
    pub fn new(backend: B, inner: W) -> Self
    where
        B: Backend,
        W: Worker<B>,
    {
        Self {
            service: inner,
            datasets: Datasets::new(),
            backend,

            defer: Arc::new(Mutex::new(HashMap::new())),
            block: Arc::new(Mutex::new(HashSet::new())),
            inits: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn add_initial(&self, request: Request) {
        let mut initial = self.inits.lock().unwrap();
        initial.push(request);
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

    async fn run_until_signal(&self) -> Result<usize>
    where
        B: Backend,
        W: Worker<B>,
    {
        let mut requests: Vec<_> = {
            let mut initial = self.inits.lock().unwrap();
            initial.drain(..).collect()
        };

        let dataset = self.datasets.get::<Request>();
        while let Some(request) = requests.pop() {
            dataset.add(request).await?;
        }

        let stream = dataset
            .into_stream()
            .map(|x| async { self.try_call_service(x).await })
            .buffer_unordered(8)
            .count();

        // let stream = dataset
        //     .into_stream()
        //     .map_err(|x| async { self.notify_signal(x.into_signal(), Tag::Fallback); } )
        //     .map_ok(|x| async { self.call_service(x).await; } )
        //     .buffer_unordered(8)
        //     .count();

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

    pub async fn call_service(&self, request: Request)
    where
        B: Backend,
        W: Worker<B>,
    {
        #[cfg(feature = "tracing")]
        {
            let size_hint = request.body().size_hint();
            tracing::info!(
                method = request.method().as_str(),
                path = request.uri().path(),
                bytes_lower = size_hint.lower(),
                bytes_upper = size_hint.upper(),
            );
        }

        let owner = request.tag().clone();

        // TODO: Tracing.

        let backend = self.backend.clone();
        let datasets = self.datasets.clone();

        let cx = Context::new(request, backend, datasets);
        let signal = self.service.clone().invoke(cx).await;
        self.notify_signal(signal, owner);
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
        let until = match defer.get(owner).cloned() {
            None => defer.get(&Tag::Fallback).cloned(),
            Some(x) => Some(x),
        };

        until.unwrap_or(now)
    }

    // TODO.
    fn apply_block(&self, owner: Tag, query: TagQuery) {
        let mut lock = |x| {
            let mut guard = self.block.lock().unwrap();
            guard.insert(x);
        };

        match query {
            TagQuery::Owner => lock(owner),
            TagQuery::Every => lock(Tag::default()),
            TagQuery::Single(x) => lock(x),
            TagQuery::List(x) => x.into_iter().for_each(lock),
        }
    }

    // TODO.
    fn find_block(&self, owner: &Tag) -> bool {
        let block = self.block.lock().unwrap();
        block.contains(&owner)
    }

    async fn notify_signal2(&self, signal: Signal) {}

    /// Applies the signal to the subsequent requests.
    fn notify_signal(&self, signal: Signal, owner: Tag) {
        // TODO: Add Ok/Err counter.
        let _ = match &signal {
            Signal::Continue | Signal::Wait(..) => false,
            Signal::Skip | Signal::Hold(..) | Signal::Fail(..) => true,
        };

        match signal {
            // Signal::Wait(x, t) | Signal::Hold(x, t) => self.apply_defer(owner, x, t),
            // Signal::Fail(x, e) => self.apply_block(owner, x),
            _ => { /* Ignore */ }
        };
    }
}
