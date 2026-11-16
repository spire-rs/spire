//! Durable response snapshots for replaying interrupted handlers.

use std::fmt;
use std::future::Future;
use std::sync::Arc;

use futures::FutureExt;
use futures::future::BoxFuture;

use crate::Result;
use crate::http::Response;

/// Records a complete response before it is exposed to handlers.
///
/// Persistent frontiers can attach a [`ResponseReplay`] to each popped seed.
/// The recorder should commit the snapshot before returning, and retain it
/// until the frontier acknowledges the corresponding occurrence.
pub trait ResponseRecorder: Send + Sync + 'static {
    /// Commits `response` for the current frontier occurrence.
    ///
    /// # Errors
    /// Returns an error if the snapshot cannot be committed.
    fn record(&self, response: Response) -> impl Future<Output = Result<()>> + Send;
}

/// A request extension carrying its recovered response and durable recorder.
///
/// Attach this to a seed's extensions when popping it from persistent storage.
/// The context reuses the snapshot, or records its first successful fetch
/// before allowing handlers to read it. Replay restores response data, not a
/// browser's live DOM or other external state.
#[derive(Clone)]
pub struct ResponseReplay {
    recovered: Option<Response>,
    recorder: Arc<dyn Recorder>,
}

impl ResponseReplay {
    /// Pairs a recovered response, if any, with this occurrence's recorder.
    #[must_use]
    pub fn new(recovered: Option<Response>, recorder: impl ResponseRecorder) -> Self {
        Self {
            recovered,
            recorder: Arc::new(recorder),
        }
    }

    pub(crate) fn recovered(&self) -> Option<Response> {
        self.recovered.clone()
    }

    pub(crate) async fn record(&self, response: Response) -> Result<()> {
        self.recorder.record(response).await
    }
}

trait Recorder: Send + Sync {
    fn record(&self, response: Response) -> BoxFuture<'_, Result<()>>;
}

impl<R: ResponseRecorder> Recorder for R {
    fn record(&self, response: Response) -> BoxFuture<'_, Result<()>> {
        ResponseRecorder::record(self, response).boxed()
    }
}

impl fmt::Debug for ResponseReplay {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ResponseReplay")
            .field("recovered", &self.recovered.is_some())
            .finish_non_exhaustive()
    }
}
