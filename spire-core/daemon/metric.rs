use std::fmt;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::backend::Backend;
use crate::context::{Context, Signal};
use crate::daemon::Worker;

/// TODO: Serde.
/// sequential estimation
#[derive(Debug, Default, PartialOrd, PartialEq, Clone)]
pub struct Stats {
    pub requests: u32,
    pub responses: u32,
    // pub failures: u32,
    pub average: Duration,
}

/// TODO.
#[derive(Clone)]
pub(crate) struct StatsLock {
    stats: Arc<Mutex<Stats>>,
}

impl StatsLock {
    pub fn new(stats: Stats) -> Self {
        let stats = Arc::new(Mutex::new(stats));
        Self { stats }
    }

    pub fn stats(&self) -> Stats {
        todo!()
    }

    pub fn notify_request(&self) {
        let mut guard = self.stats.lock().unwrap();
        guard.requests += 1;
    }

    pub fn notify_response(&self, since: Duration) {
        let mut guard = self.stats.lock().unwrap();
        guard.responses += 1;

        let prev_total = guard.average.as_millis() * guard.requests as u128;
        let curr_total = prev_total + since.as_millis();
        let average = curr_total / guard.requests as u128 + 1u128;

        guard.average = Duration::from_millis(average as u64);
    }
}

#[derive(Clone)]
pub struct StatWorker<S> {
    inner: S,
    stats: StatsLock,
}

impl<S> StatWorker<S> {
    /// Creates a new [`StatWorker`].
    pub fn new(inner: S, stats: Stats) -> Self {
        let stats = StatsLock::new(stats);
        Self { inner, stats }
    }

    /// Returns the current stats.
    pub fn stats(&self) -> Stats {
        todo!()
    }
}

impl<S> fmt::Debug for StatWorker<S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("StatWorker").finish_non_exhaustive()
    }
}

#[async_trait::async_trait]
impl<B, S> Worker<B> for StatWorker<S>
where
    B: Backend,
    S: Worker<B>,
{
    #[inline]
    async fn route(self, cx: Context<B>) -> Signal {
        let t0 = Instant::now();
        self.stats.notify_request();

        let signal = self.inner.route(cx).await;
        let since = Instant::now() - t0;
        self.stats.notify_response(since);

        signal
    }
}
