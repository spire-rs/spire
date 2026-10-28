//! Objective configuration, running progress and relevance decisions.

mod frontier;
mod rating;
mod run;
#[cfg(all(test, feature = "testing"))]
mod tests;

use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

pub use frontier::{AgentFrontierExt, Until};
pub use rating::Candidate;
pub use run::{Finished, ObjectiveRun, Progress};

use crate::Agent;

type Scorer = dyn Fn(&Candidate) -> Option<i32> + Send + Sync;

#[derive(Clone)]
struct Settings {
    goal: String,
    relevant: String,
    done: Option<String>,
    items: Option<u64>,
    pages: Option<u64>,
    stall: Option<u64>,
    check_every: u64,
    threshold: u8,
    batch_size: usize,
    delay: Duration,
    scorer: Option<Arc<Scorer>>,
}

/// Configuration for an objective-driven crawl.
///
/// Clones are independent configurations; call [`Self::start`] to create a
/// shared running objective with an agent.
#[derive(Clone)]
pub struct Objective {
    settings: Settings,
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

impl Objective {
    /// Creates an objective configuration.
    ///
    /// Defaults to a 50/100 relevance threshold, 256 links per rating call and
    /// a 10ms batching delay. No target or stall limit is set.
    #[must_use]
    pub fn new(goal: impl Into<String>) -> Self {
        Self {
            settings: Settings {
                goal: goal.into(),
                relevant: String::new(),
                done: None,
                items: None,
                pages: None,
                stall: None,
                check_every: 5,
                threshold: 50,
                batch_size: 256,
                delay: Duration::from_millis(10),
                scorer: None,
            },
        }
    }

    /// Returns the description of what the crawl is looking for.
    #[must_use]
    pub fn goal(&self) -> &str {
        &self.settings.goal
    }

    /// Describes the kinds of links that serve the goal.
    #[must_use]
    pub fn relevant(mut self, description: impl Into<String>) -> Self {
        self.settings.relevant = description.into();
        self
    }

    /// Adds a condition the model judges from the structured progress record.
    #[must_use]
    pub fn done_when(mut self, condition: impl Into<String>) -> Self {
        self.settings.done = Some(condition.into());
        self
    }

    /// Stops after finding this many items.
    ///
    /// Zero stops before crawling.
    #[must_use]
    pub fn target_items(mut self, count: u64) -> Self {
        self.settings.items = Some(count);
        self
    }

    /// Stops after completing this many distinct relevant pages.
    #[must_use]
    pub fn target_pages(mut self, count: u64) -> Self {
        self.settings.pages = Some(count);
        self
    }

    /// Sets the limit on completed pages without progress.
    ///
    /// Progress means new relevant pages or items. Zero stops immediately;
    /// failed pages also advance this window.
    #[must_use]
    pub fn stall_after(mut self, count: u64) -> Self {
        self.settings.stall = Some(count);
        self
    }

    /// Minimum completed pages between model completion judgments, clamped to
    /// at least one.
    ///
    /// Exhaustion triggers a final changed-evidence check.
    #[must_use]
    pub fn check_every(mut self, pages: u64) -> Self {
        self.settings.check_every = pages.max(1);
        self
    }

    /// Sets the minimum relevance score to follow, clamped to 1..=100.
    #[must_use]
    pub fn relevance_threshold(mut self, score: u8) -> Self {
        self.settings.threshold = score.clamp(1, 100);
        self
    }

    /// Sets the maximum links in one call, clamped to 1..=256.
    #[must_use]
    pub fn rating_batch_size(mut self, count: usize) -> Self {
        self.settings.batch_size = count.clamp(1, 256);
        self
    }

    /// Sets how long a partial rating batch waits for other pages.
    #[must_use]
    pub fn rating_delay(mut self, delay: Duration) -> Self {
        self.settings.delay = delay;
        self
    }

    /// Settles clear link decisions locally.
    ///
    /// `None` asks the model; `Some(score)` settles the link without a model
    /// call. Scores are clamped to 0..=100; zero rejects the link.
    #[must_use]
    pub fn score_with(
        mut self,
        scorer: impl Fn(&Candidate) -> Option<i32> + Send + Sync + 'static,
    ) -> Self {
        self.settings.scorer = Some(Arc::new(scorer));
        self
    }

    /// Freezes these settings and owns the fully configured model agent.
    ///
    /// Each call creates independent progress, ratings and termination state.
    #[must_use]
    pub fn start(self, agent: Agent) -> ObjectiveRun {
        ObjectiveRun::new(self.settings, agent)
    }
}

impl std::fmt::Debug for Objective {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Objective")
            .field("goal", &self.settings.goal)
            .finish_non_exhaustive()
    }
}
