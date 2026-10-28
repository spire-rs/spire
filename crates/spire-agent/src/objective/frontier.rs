use spire_core::frontier::seed::Seed;
use spire_core::frontier::{Ack, Admission, Frontier, NextSeed, Report};

use super::run::Phase;
use super::{ObjectiveRun, lock};

/// Adds objective-based termination to a frontier.
///
/// Keep this outside retry, revisit and scope facades so it observes every pop
/// and report. Drive the crawler with [`ObjectiveRun::run`] for shutdown wiring
/// and final completion checks; the frontier itself makes no model calls.
pub trait AgentFrontierExt: Frontier + Sized {
    /// Stops admitting work when `objective` finishes.
    ///
    /// In-flight work drains; queued and cancelled persistent seeds remain
    /// available for another run.
    fn until(self, objective: &ObjectiveRun) -> Until<Self> {
        lock(&objective.shared.state).attached = true;
        Until {
            inner: self,
            objective: objective.clone(),
        }
    }
}

impl<F: Frontier> AgentFrontierExt for F {}

/// An objective-aware frontier.
///
/// See [`AgentFrontierExt::until`].
#[derive(Debug, Clone)]
pub struct Until<F> {
    inner: F,
    objective: ObjectiveRun,
}

impl<F: Frontier> Frontier for Until<F> {
    async fn push(&self, seed: Seed) -> spire_core::Result<Admission> {
        if self.objective.finished().is_some() || self.objective.externally_stopped() {
            return Ok(Admission::Stopped);
        }
        self.inner.push(seed).await
    }

    async fn pop(&self) -> spire_core::Result<NextSeed> {
        {
            let mut state = lock(&self.objective.shared.state);
            if state.phase != Phase::Running {
                return Err(spire_core::Error::new(spire_core::ErrorKind::Invalid)
                    .with_message("drive this frontier through ObjectiveRun::run"));
            }
            state.observed = true;
        }
        if self.objective.finished().is_some() || self.objective.externally_stopped() {
            return Ok(NextSeed::Empty);
        }
        let next = self.inner.pop().await?;
        match &next {
            NextSeed::Ready(seed) => {
                *lock(&self.objective.shared.state)
                    .active
                    .entry(seed.fingerprint().get())
                    .or_default() += 1;
            }
            NextSeed::Empty if lock(&self.objective.shared.state).active.is_empty() => {
                lock(&self.objective.shared.state).exhausted = true;
            }
            _ => {}
        }
        Ok(next)
    }

    async fn report(&self, report: Report) -> spire_core::Result<Ack> {
        let page = match &report {
            Report::Done(seed) => {
                Some((seed.fingerprint().get(), seed.canonical_uri(), Some(true)))
            }
            Report::Failed(seed) => {
                Some((seed.fingerprint().get(), seed.canonical_uri(), Some(false)))
            }
            Report::Skipped(seed) | Report::Cancelled(seed) | Report::Retry { seed, .. } => {
                Some((seed.fingerprint().get(), seed.canonical_uri(), None))
            }
            _ => None,
        };
        let result = self.inner.report(report).await;
        if let Some((key, uri, completed)) = page {
            let relevant = self.objective.rated_relevant(&uri);
            let mut state = lock(&self.objective.shared.state);
            if let Some(count) = state.active.get_mut(&key) {
                *count -= 1;
                if *count == 0 {
                    state.active.remove(&key);
                }
            }
            if let Some(success) = completed
                && state.visited.insert(uri.clone())
            {
                state.progress.pages = state.progress.pages.saturating_add(1);
                if success && (relevant || state.evidence_pages.contains(&uri)) {
                    state.progress.relevant_pages = state.progress.relevant_pages.saturating_add(1);
                    state.last_progress = state.progress.pages;
                    state.revision = state.revision.saturating_add(1);
                }
                if state.revision != state.reported_revision {
                    state.last_progress = state.progress.pages;
                    state.reported_revision = state.revision;
                }
                self.objective.limits(&mut state);
            }
        }
        result
    }

    fn size_hint(&self) -> Option<usize> {
        self.inner.size_hint()
    }

    fn snapshot(&self) -> Option<Vec<Seed>> {
        self.inner.snapshot()
    }
}
