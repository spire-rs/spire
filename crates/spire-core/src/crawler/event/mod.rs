//! Typed [`Event`]s a crawl reports as it runs, see [`Crawler::events`].
//!
//! [`Crawler::events`]: crate::crawler::Crawler::events

use std::fmt;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::task::{Context, Poll};
use std::time::Duration;

use ::http::{Method, StatusCode, Uri};
use futures::Stream;
use tokio::sync::broadcast;
use tokio::sync::broadcast::error::RecvError;

use crate::context::Fetch;
use crate::crawler::{Attempt, Depth, Stats, Tag};
use crate::frontier::seed::Seed;
use crate::worker::Outcome;
use crate::{Error, ErrorKind};

#[cfg(test)]
mod tests;

/// Events kept for a subscriber that falls behind, before the oldest are
/// dropped for it.
pub(crate) const CAPACITY: usize = 1024;

/// Something that happened in a crawl, as [`Crawler::events`] reports it.
///
/// Every request reports [`RequestStarted`](Self::RequestStarted), then
/// [`Fetched`](Self::Fetched) for each fetch its handlers made, then
/// [`Handled`](Self::Handled) once they returned. After that, it is
/// [`Retried`](Self::Retried) or [`Failed`](Self::Failed) if it did not
/// succeed. A request still running when the shutdown grace period ends is
/// [`Cancelled`](Self::Cancelled) instead of handled.
///
/// [`Crawler::events`]: crate::crawler::Crawler::events
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum Event {
    /// The run started.
    Started,
    /// A request started: its client is connecting.
    RequestStarted {
        /// The request.
        request: RequestInfo,
    },
    /// A request was fetched, when its handlers first asked for the
    /// response. A failed fetch is reported too, and fetched again if a
    /// handler asks again.
    Fetched {
        /// The request.
        request: RequestInfo,
        /// The status the server answered with, also when the backend
        /// failed the fetch for it, e.g. a `404`; `None` if no response
        /// came, e.g. the connection failed.
        status: Option<StatusCode>,
        /// How long the fetch took.
        latency: Duration,
    },
    /// A request's handlers returned.
    Handled {
        /// The request.
        request: RequestInfo,
        /// What the handlers returned.
        outcome: OutcomeKind,
        /// Why the request failed, for [`OutcomeKind::Fail`] and
        /// [`OutcomeKind::Fatal`].
        error: Option<Failure>,
        /// How long the request took since it started, connecting and
        /// fetching included.
        elapsed: Duration,
    },
    /// The frontier took a request back for another attempt.
    Retried {
        /// The attempt that failed.
        request: RequestInfo,
        /// How long the next attempt waits at least, if the request said.
        after: Option<Duration>,
    },
    /// A request failed for good: not retryable, out of retries, or refused
    /// a retry by the frontier.
    Failed {
        /// The request.
        request: RequestInfo,
    },
    /// A request was not crawled on purpose.
    Skipped {
        /// The request.
        request: RequestInfo,
        /// Why it was skipped.
        reason: SkipReason,
    },
    /// A request was still in flight when the shutdown grace period ended.
    Cancelled {
        /// The request.
        request: RequestInfo,
    },
    /// A host was throttled, and the crawl slowed down on it.
    HostSlowed {
        /// The host's authority, in lowercase.
        host: String,
        /// How the host was slowed down.
        slowdown: Slowdown,
    },
    /// A host asked to be left alone, with `Retry-After`, and nothing
    /// starts on it for a while.
    HostPaused {
        /// The host's authority, in lowercase.
        host: String,
        /// How long the host is paused.
        duration: Duration,
    },
    /// An origin's robots.txt was fetched, or could not be.
    RobotsFetched {
        /// The origin: its scheme, host and port.
        origin: String,
        /// Why the fetch failed, if it did.
        error: Option<Failure>,
    },
    /// The run is stopping: nothing new starts, and requests in flight
    /// finish.
    Stopping {
        /// Why the run stops.
        reason: StopReason,
    },
    /// The run finished, with its totals.
    Finished {
        /// The totals of the run.
        stats: Stats,
    },
    /// This subscriber fell behind and missed events, which were dropped
    /// for it; the crawl is never slowed down for a subscriber.
    Lagged {
        /// How many events were missed.
        missed: u64,
    },
}

/// The request an [`Event`] is about: what identifies it, without its
/// headers or body.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct RequestInfo {
    /// The method.
    pub method: Method,
    /// The URI.
    pub uri: Uri,
    /// The tag that routes it, if any.
    pub tag: Option<Tag>,
    /// How many links away from a seed it is.
    pub depth: Depth,
    /// Which attempt this is.
    pub attempt: Attempt,
}

impl From<&Seed> for RequestInfo {
    fn from(seed: &Seed) -> Self {
        Self {
            method: seed.method().clone(),
            uri: seed.uri().clone(),
            tag: seed.tag().cloned(),
            depth: seed.depth(),
            attempt: seed.attempt(),
        }
    }
}

/// What a request's handlers returned, see [`Outcome`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum OutcomeKind {
    /// [`Outcome::Continue`].
    Continue,
    /// [`Outcome::Retry`].
    Retry,
    /// [`Outcome::Backoff`].
    Backoff,
    /// [`Outcome::Abort`].
    Abort,
    /// [`Outcome::Stop`].
    Stop,
    /// [`Outcome::Fail`], including a
    /// timeout or a panic.
    Fail,
    /// [`Outcome::Fatal`].
    Fatal,
}

impl OutcomeKind {
    /// Returns the outcome's name, e.g. `"continue"`.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Continue => "continue",
            Self::Retry => "retry",
            Self::Backoff => "backoff",
            Self::Abort => "abort",
            Self::Stop => "stop",
            Self::Fail => "fail",
            Self::Fatal => "fatal",
        }
    }
}

impl fmt::Display for OutcomeKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Why a request or robots.txt fetch failed: a copy of its [`Error`]'s
/// kind, status and message.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct Failure {
    /// The error's kind.
    pub kind: ErrorKind,
    /// The response's status, if the error came from one.
    pub status: Option<StatusCode>,
    /// The error, as it displays.
    pub message: String,
}

impl From<&Error> for Failure {
    fn from(error: &Error) -> Self {
        Self {
            kind: error.kind(),
            status: error.status(),
            message: error.to_string(),
        }
    }
}

impl fmt::Display for Failure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

/// Why a request was skipped.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum SkipReason {
    /// robots.txt disallows it.
    Disallowed,
    /// Its tag was aborted, see [`Outcome::Abort`].
    TagAborted,
}

impl SkipReason {
    /// Returns the reason as it is logged, e.g. `"disallowed by robots.txt"`.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Disallowed => "disallowed by robots.txt",
            Self::TagAborted => "tag aborted",
        }
    }
}

impl fmt::Display for SkipReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// How a throttled host was slowed down.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Slowdown {
    /// Its adaptive concurrency was lowered, see
    /// [`HostConcurrency::adaptive`](crate::crawler::HostConcurrency::adaptive).
    Concurrency {
        /// Requests in flight to it at most, from now on.
        limit: usize,
    },
    /// The gap between starting two requests to it grew, see
    /// [`HostDelay`](crate::crawler::HostDelay).
    Gap {
        /// The new gap.
        gap: Duration,
    },
}

/// Why a run stopped.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum StopReason {
    /// The [shutdown token](crate::crawler::Crawler::shutdown_token) was
    /// cancelled.
    Shutdown,
    /// The [time limit](crate::crawler::Crawler::with_time_limit) was
    /// reached.
    TimeLimit,
    /// The [request budget](crate::crawler::Crawler::with_max_requests)
    /// was spent.
    RequestBudget,
    /// A handler returned [`Outcome::Stop`].
    Requested,
    /// A handler returned [`Outcome::Fatal`],
    /// or the frontier failed.
    Failed,
}

impl StopReason {
    /// Returns the reason as it is logged, e.g. `"time limit reached"`.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Shutdown => "shutdown requested",
            Self::TimeLimit => "time limit reached",
            Self::RequestBudget => "request budget spent",
            Self::Requested => "worker requested stop",
            Self::Failed => "crawl failed",
        }
    }
}

impl fmt::Display for StopReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Sends events to the subscribers of a crawler.
///
/// Subscribers are counted on the side, so that without one an event is
/// never built: checking is a single atomic load, where the channel's own
/// count takes a lock.
#[derive(Clone)]
pub(crate) struct Emitter {
    sender: broadcast::Sender<Event>,
    subscribers: Arc<AtomicUsize>,
}

impl Emitter {
    /// Creates an emitter keeping `capacity` events, at least one, for each
    /// subscriber.
    pub(crate) fn new(capacity: usize) -> Self {
        let (sender, _) = broadcast::channel(capacity.max(1));
        Self {
            sender,
            subscribers: Arc::default(),
        }
    }

    /// Whether anyone is subscribed.
    pub(crate) fn is_active(&self) -> bool {
        self.subscribers.load(Ordering::Relaxed) > 0
    }

    /// Sends the event `event` builds, only built if anyone is subscribed.
    pub(crate) fn emit(&self, event: impl FnOnce() -> Event) {
        if self.is_active() {
            // A subscriber dropped since the check is not an error.
            let _ = self.sender.send(event());
        }
    }

    /// Sends the event `event` builds from `request`, if the events of a
    /// request are wanted: `request` is only built while anyone subscribes.
    pub(crate) fn emit_for(
        &self,
        request: Option<&RequestInfo>,
        event: impl FnOnce(RequestInfo) -> Event,
    ) {
        if let Some(request) = request {
            self.emit(|| event(request.clone()));
        }
    }

    /// Subscribes to every event from now on.
    pub(crate) fn subscribe(&self) -> Events {
        self.subscribers.fetch_add(1, Ordering::Relaxed);
        let subscription = Subscription {
            receiver: self.sender.subscribe(),
            subscribers: self.subscribers.clone(),
        };
        let stream = futures::stream::unfold(subscription, |mut subscription| async move {
            let event = match subscription.receiver.recv().await {
                Ok(event) => event,
                Err(RecvError::Lagged(missed)) => Event::Lagged { missed },
                Err(RecvError::Closed) => return None,
            };
            Some((event, subscription))
        });
        Events {
            stream: Box::pin(stream),
        }
    }

    /// Prepares reporting the fetches of the request for `seed`, if anyone
    /// is subscribed.
    pub(crate) fn notify(&self, seed: &Seed) -> Option<Notify> {
        self.is_active().then(|| Notify {
            emitter: self.clone(),
            request: seed.into(),
        })
    }
}

impl fmt::Debug for Emitter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Emitter")
            .field("subscribers", &self.subscribers.load(Ordering::Relaxed))
            .finish_non_exhaustive()
    }
}

/// The [`Event`]s of a crawler, from [`Crawler::events`].
///
/// It ends once the crawler is dropped.
///
/// [`Crawler::events`]: crate::crawler::Crawler::events
#[must_use = "streams do nothing unless polled"]
pub struct Events {
    stream: Pin<Box<dyn Stream<Item = Event> + Send>>,
}

impl Stream for Events {
    type Item = Event;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Event>> {
        self.stream.as_mut().poll_next(cx)
    }
}

impl fmt::Debug for Events {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Events").finish_non_exhaustive()
    }
}

/// A subscriber's receiver, counted while it lives.
struct Subscription {
    receiver: broadcast::Receiver<Event>,
    subscribers: Arc<AtomicUsize>,
}

impl Drop for Subscription {
    fn drop(&mut self) {
        self.subscribers.fetch_sub(1, Ordering::Relaxed);
    }
}

/// Reports the fetches of a running request, from the task it runs on.
pub(crate) struct Notify {
    emitter: Emitter,
    request: RequestInfo,
}

impl Notify {
    /// Reports that the request was fetched.
    pub(crate) fn fetched(&self, fetch: Fetch) {
        self.emitter.emit(|| Event::Fetched {
            request: self.request.clone(),
            status: fetch.status,
            latency: fetch.latency.unwrap_or_default(),
        });
    }
}

impl From<&Outcome> for OutcomeKind {
    fn from(outcome: &Outcome) -> Self {
        match outcome {
            Outcome::Continue => Self::Continue,
            Outcome::Retry { .. } => Self::Retry,
            Outcome::Backoff { .. } => Self::Backoff,
            Outcome::Abort(_) => Self::Abort,
            Outcome::Stop => Self::Stop,
            Outcome::Fail(_) => Self::Fail,
            Outcome::Fatal(_) => Self::Fatal,
        }
    }
}
