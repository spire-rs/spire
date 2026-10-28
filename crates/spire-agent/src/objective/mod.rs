//! Typed goals and middleware for objective-guided crawling.

mod middleware;
mod registry;
mod scoring;
#[cfg(all(test, feature = "testing"))]
mod tests;

use std::marker::PhantomData;
use std::sync::Arc;

pub use middleware::{ObjectiveExt, ObjectiveWorker};
pub use registry::{
    ObjectiveRegistry, ObjectiveStatus, PreparedEvidence, Progress, StopPolicy, StopReason,
};

/// A resolved link considered for objective-guided discovery.
#[derive(Debug, Clone, serde::Serialize)]
pub struct Candidate {
    /// The absolute HTTP or HTTPS URL.
    pub url: String,
    /// The visible anchor text.
    pub text: String,
}

type Scorer = dyn Fn(&Candidate) -> Option<i32> + Send + Sync;

#[derive(Clone)]
struct Settings {
    goal: String,
    relevant: String,
    done: Option<String>,
    items: Option<u64>,
    pages: Option<u64>,
    stall: Option<u64>,
    threshold: u8,
    scorer: Option<Arc<Scorer>>,
}

/// A crawl goal associated with accepted records of type `T`.
///
/// Register it with [`ObjectiveExt::with_objective`] after configuring routes.
/// Each registration owns progress; worker clones share it. Configuration
/// clones remain independent until registered.
pub struct Objective<T = ()> {
    settings: Settings,
    marker: PhantomData<fn() -> T>,
}

impl<T> Objective<T> {
    /// Creates a goal without numeric targets or a completion condition.
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
                threshold: 50,
                scorer: None,
            },
            marker: PhantomData,
        }
    }

    /// Describes links relevant to this goal.
    #[must_use]
    pub fn relevant(mut self, description: impl Into<String>) -> Self {
        self.settings.relevant = description.into();
        self
    }

    /// Adds a condition judged from newly accepted records.
    #[must_use]
    pub fn done_when(mut self, condition: impl Into<String>) -> Self {
        self.settings.done = Some(condition.into());
        self
    }

    /// Finishes after this many accepted records, including duplicates.
    #[must_use]
    pub const fn target_items(mut self, count: u64) -> Self {
        self.settings.items = Some(count);
        self
    }

    /// Finishes after this many distinct successful relevant pages.
    #[must_use]
    pub const fn target_pages(mut self, count: u64) -> Self {
        self.settings.pages = Some(count);
        self
    }

    /// Finishes after this many successful pages without objective progress.
    #[must_use]
    pub const fn stall_after(mut self, pages: u64) -> Self {
        self.settings.stall = Some(pages);
        self
    }

    /// Sets the minimum score to follow, clamped to 1..=100.
    #[must_use]
    pub fn relevance_threshold(mut self, score: u8) -> Self {
        self.settings.threshold = score.clamp(1, 100);
        self
    }

    /// Decides clear link ratings locally, leaving `None` to the model.
    ///
    /// `Some(0)` rejects a link. Scores are clamped to 0..=100.
    #[must_use]
    pub fn score_with(
        mut self,
        scorer: impl Fn(&Candidate) -> Option<i32> + Send + Sync + 'static,
    ) -> Self {
        self.settings.scorer = Some(Arc::new(scorer));
        self
    }
}

impl<T> Clone for Objective<T> {
    fn clone(&self) -> Self {
        Self {
            settings: self.settings.clone(),
            marker: PhantomData,
        }
    }
}

impl<T> std::fmt::Debug for Objective<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Objective")
            .field("goal", &self.settings.goal)
            .finish_non_exhaustive()
    }
}
