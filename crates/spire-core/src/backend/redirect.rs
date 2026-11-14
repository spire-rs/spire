//! Admission of individual HTTP redirect hops by the crawl scheduler.

use std::time::Duration;

use tokio::sync::{mpsc, oneshot};
use tokio::task::Id;
use tokio::time::Instant;

use crate::frontier::seed::Seed;
use crate::http::{Request, RequestExt};
use crate::{Error, ErrorKind, Result};

/// A request extension used by backends that expose redirect hops.
///
/// Before every network transfer, call [`check`](Self::check) with the
/// outgoing request and hold its permit until the transfer finishes. The
/// scheduler checks frontier scope, robots.txt, and host limits, releasing
/// the previous host slot while the target waits. Transfers within one
/// request are sequential.
/// Standalone requests have no such extension.
#[derive(Debug, Clone)]
pub struct Redirects {
    pub(crate) sender: mpsc::UnboundedSender<TransferEvent>,
    pub(crate) task: Id,
}

#[derive(Debug)]
pub(crate) enum TransferEvent {
    Admit(RedirectRequest),
    Finished {
        task: Id,
        started: Instant,
        latency: Duration,
    },
}

/// An admitted network transfer. Dropping it records time spent transferring,
/// excluding time waiting for scheduler admission.
#[must_use = "hold the permit until the network transfer finishes"]
#[derive(Debug)]
pub struct Transfer {
    sender: mpsc::UnboundedSender<TransferEvent>,
    task: Id,
    started: Instant,
}

impl Drop for Transfer {
    fn drop(&mut self) {
        let _ = self.sender.send(TransferEvent::Finished {
            task: self.task,
            started: self.started,
            latency: self.started.elapsed(),
        });
    }
}

#[derive(Debug)]
pub(crate) struct RedirectRequest {
    pub(crate) task: Id,
    pub(crate) seed: Seed,
    pub(crate) answer: oneshot::Sender<Result<()>>,
}

impl Redirects {
    /// Waits until a request may be sent and returns its transfer permit.
    ///
    /// # Errors
    /// Returns an error if scope or robots.txt disallows the target, the crawl
    /// stops, or the originating request task is no longer running.
    pub async fn check(&self, request: &Request) -> Result<Transfer> {
        let mut seed = Seed::new(request.method().clone(), request.uri().clone())?
            .with_depth(request.depth())
            .with_body(request.body().clone());
        if let Some(tag) = request.tag() {
            seed = seed.with_tag(tag.clone());
        }
        if let Some(identity) = request.identity() {
            seed = seed.with_identity(identity.name());
        }
        for (name, value) in request.headers() {
            seed = seed.with_header(name.clone(), value.clone());
        }
        let (answer, received) = oneshot::channel();
        self.sender
            .send(TransferEvent::Admit(RedirectRequest {
                task: self.task,
                seed,
                answer,
            }))
            .map_err(|_| cancelled())?;
        received.await.map_err(|_| cancelled())??;
        Ok(Transfer {
            sender: self.sender.clone(),
            task: self.task,
            started: Instant::now(),
        })
    }
}

pub(crate) fn cancelled() -> Error {
    Error::new(ErrorKind::Cancelled).with_message("redirect admission was cancelled")
}
