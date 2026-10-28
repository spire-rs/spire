use spire_agent::objective::{Candidate, ObjectiveRun};

use super::Handler;
use crate::backend::Session;
use crate::crawler::Tag;
use crate::extract::html::Links;
use crate::extract::{FromContext, FromRef};
use crate::frontier::queue::Link;
use crate::worker::{IntoOutcome, Outcome};
use crate::{Context, TRACING_TARGET};

/// Follows visible links relevant to an objective.
///
/// Concurrent pages share batched ratings by canonical URL. Scores become
/// link priorities; use a priority frontier for best-first order.
///
/// The handler awaits ratings and queue writes before finishing. Scope,
/// deduplication and trap limits belong on the frontier; [`Self::same_host`]
/// filters links before rating them.
#[derive(Debug, Clone)]
#[must_use]
pub struct FollowRelevant {
    tag: Tag,
    same_host: bool,
    skip_nofollow: bool,
}

impl FollowRelevant {
    /// Follows relevant links with `tag`.
    ///
    /// The [`ObjectiveRun`] is selected from router state through [`FromRef`].
    pub fn new(tag: impl Into<Tag>) -> Self {
        Self {
            tag: tag.into(),
            same_host: false,
            skip_nofollow: false,
        }
    }

    /// Rates and follows only links to the page's own host.
    pub const fn same_host(mut self) -> Self {
        self.same_host = true;
        self
    }

    /// Excludes `rel="nofollow"` links before rating them.
    pub const fn skip_nofollow(mut self) -> Self {
        self.skip_nofollow = true;
        self
    }
}

impl<C, S> Handler<C, FollowRelevant, S> for FollowRelevant
where
    C: Session,
    S: Send + Sync + 'static,
    ObjectiveRun: FromRef<S>,
{
    async fn call(&self, cx: &Context<C>, state: &S) -> Outcome {
        let objective = ObjectiveRun::from_ref(state);
        if objective.finished().is_some() {
            return Outcome::Stop;
        }
        let links = match Links::from_context(cx, state).await {
            Ok(links) => links.visible(),
            Err(error) => return error.into_outcome(),
        };
        let links = if self.same_host {
            links.same_host()
        } else {
            links
        };
        let candidates = links
            .iter()
            .filter(|link| !self.skip_nofollow || !link.has_rel("nofollow"))
            .map(|link| Candidate {
                url: link.uri().to_string(),
                text: link.text().into(),
            })
            .collect::<Vec<_>>();
        let ratings = match objective.rate(&candidates).await {
            Ok(ratings) => ratings,
            Err(error) => return error.into_outcome(),
        };
        let mut queued = 0;
        for (candidate, score) in candidates.into_iter().zip(ratings) {
            if !objective.accepts(score) {
                continue;
            }
            let link = Link::new(candidate.url)
                .with_priority(score)
                .with_tag(self.tag.clone());
            match cx.queue().follow(link).await {
                Ok(admission) => queued += usize::from(admission.is_queued()),
                Err(error) => return error.into_outcome(),
            }
        }
        tracing::debug!(target: TRACING_TARGET, queued, "followed relevant links");
        if let Err(error) = objective.check().await {
            return error.into_outcome();
        }
        if objective.finished().is_some() {
            Outcome::Stop
        } else {
            Outcome::Continue
        }
    }
}
