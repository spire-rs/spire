use std::cmp::max;
use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use futures::stream::{AbortHandle, Abortable};
use futures::StreamExt;

use crate::backend::{Backend, Worker};
use crate::context::{Context, Tag, TagQuery, Task};
use crate::context::{IntoSignal, Request, Signal};
use crate::dataset::{Dataset, Datasets};
use crate::Result;

pub struct Runner<B, W> {
    service: W,
    pub datasets: Datasets,
    backend: B,

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

    /// Repeatedly calls the used [`Backend`] until the [`Request`] queue is empty
    /// or the stream is aborted with a [`Signal`].
    ///
    /// Returns the total amount of processed `Request`s.
    pub async fn run(&self) -> Result<usize>
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

        let stream = stream
            // Abort on request queue/stream failures.
            .filter_map(|x| async { x.inspect_err(|_| handle.abort()).ok() })
            // Invoke the  underlying backend/worker.
            .then(|x| async move { self.run_once(x).await })
            // Abort on underlying backend/worker failures.
            .map(|x| async { x.inspect_err(|_| handle.abort()) })
            // Other.
            .buffer_unordered(concurrent_limit)
            .count();

        Ok(stream.await)
    }

    /// Calls the used [`Backend`] once with a provided [`Request`].
    ///
    /// # Errors
    ///
    /// Only if the `Request` stream should be aborted.
    pub async fn run_once(&self, req: Request) -> Result<()>
    where
        B: Backend,
        W: Worker<B::Client>,
    {
        match self.call_service(req).await {
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
        // TODO: Apply defer.

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
            Signal::Fail(x, _) => self.apply_abort(x, owner)?,
            Signal::Continue | Signal::Skip => {}
        }

        Ok(())
    }

    /// Defers all [`Tag`]s as specified per [`TagQuery`].
    fn apply_defer(&self, query: TagQuery, owner: Tag, duration: Duration) {
        let minimum = Instant::now() + duration;
        let mut defer = self.defer.lock().unwrap();

        let mut defer_one = |x: Tag| {
            let _ = match defer.entry(x) {
                Entry::Occupied(mut x) => x.insert(max(*x.get() + duration, minimum)),
                Entry::Vacant(x) => *x.insert(minimum),
            };
        };

        match query {
            TagQuery::Owner => defer_one(owner),
            TagQuery::Single(x) => defer_one(x),
            TagQuery::Every => defer_one(Tag::Fallback),
            TagQuery::List(x) => x.into_iter().for_each(defer_one),
        };
    }

    /// Aborts all [`Tag`]s as specified per [`TagQuery`].
    fn apply_abort(&self, query: TagQuery, owner: Tag) -> Result<()> {
        // TODO: Implement `Runner::apply_abort`.

        Ok(())
    }
}
