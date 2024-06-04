use std::cmp::max;
use std::collections::{HashMap, HashSet};
use std::collections::hash_map::Entry;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use futures::stream::{Abortable, AbortHandle};
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
    // Fallback means all not-yet encountered tags.
    defer: Mutex<HashMap<Tag, Instant>>,
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
            defer: Mutex::new(HashMap::new()),
        }
    }

    /// Repeatedly call the used [`Backend`] until the [`Request`] queue is empty.
    ///
    /// Returns the total amount of processed `Request`s.
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

    /// Repeatedly call the used [`Backend`] until the [`Request`] queue is empty or
    /// the stream is aborted with a [`Signal`].
    ///
    /// Returns the total amount of processed `Request`s.
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

        let concurrent_limit = self.limit.load(Ordering::SeqCst);
        let (handle, registration) = AbortHandle::new_pair();
        let stream = Abortable::new(dataset.into_stream(), registration);

        let try_run_once = |x| async {
            let result = match x {
                Ok(req) => self.run_once(req).await,
                Err(x) => self.notify(x.into_signal(), Tag::Fallback),
            };

            if result.is_err() {
                handle.abort();
            }
        };

        let stream = stream.map(try_run_once).buffered(concurrent_limit).count();

        Ok(stream.await)
    }

    /// Calls the used [`Backend`] once with a provided [`Request`].
    ///
    /// # Errors
    ///
    /// Only if the `Request` stream should be aborted.
    pub async fn run_once(&self, request: Request) -> Result<()>
    where
        B: Backend,
        W: Worker<B::Client>,
    {
        match self.call_service(request).await {
            Ok((signal, owner)) => self.notify(signal, owner),
            Err(x) => self.notify(x.into_signal(), Tag::Fallback),
        }
    }

    /// Creates the [`Context`] and calls the used [`Backend`] with it.
    ///
    /// Returns [`Signal`] and owner [`Tag`].
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

    /// Applies the [`Signal`] to the subsequent [`Request`]s.
    ///
    /// # Errors
    ///
    /// Only if the `Request` stream should be aborted.
    pub fn notify(&self, signal: Signal, owner: Tag) -> Result<()> {
        match signal {
            Signal::Wait(x, t) | Signal::Hold(x, t) => self.apply_defer(x, owner, t),
            Signal::Fail(x, _) => self.apply_block(x, owner),
            Signal::Continue | Signal::Skip => Ok(()),
        }
    }

    /// Defers all [`Tag`]s as specified per [`TagQuery`].
    fn apply_defer(&self, query: TagQuery, owner: Tag, duration: Duration) -> Result<()> {
        todo!("apply_defer not implemented")
    }

    /// Blocks all [`Tag`]s as specified per [`TagQuery`].
    fn apply_block(&self, query: TagQuery, owner: Tag) -> Result<()> {
        todo!("apply_block not implemented")
    }
}
