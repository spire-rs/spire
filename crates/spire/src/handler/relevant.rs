use spire_agent::Agent;
use spire_agent::objective::{Candidate, ObjectiveRegistry};

use super::Handler;
use crate::backend::Session;
use crate::crawler::Tag;
use crate::extract::FromContext;
use crate::extract::html::Links;
use crate::frontier::queue::Link;
use crate::frontier::seed::Seed;
use crate::worker::{IntoOutcome, Outcome};
use crate::{Context, Error};

/// Follows visible links accepted by any active objective.
///
/// Requires an [`Agent`] registered with [`with_agent`](spire_agent::AgentCrawlerExt::with_agent)
/// and the [`ObjectiveRegistry`] supplied by
/// [`with_objective`](spire_agent::objective::ObjectiveExt::with_objective) middleware.
///
/// Ratings are merged by maximum accepted score, then each candidate is
/// queued once. Use a priority frontier for best-first order; scope and
/// deduplication policies remain on the ordinary frontier.
#[derive(Debug, Clone)]
#[must_use]
pub struct FollowRelevantLinks {
    tag: Tag,
    same_host: bool,
    skip_nofollow: bool,
}

impl FollowRelevantLinks {
    /// Follows relevant destinations with the supplied router tag.
    pub fn new(tag: impl Into<Tag>) -> Self {
        Self {
            tag: tag.into(),
            same_host: false,
            skip_nofollow: false,
        }
    }

    /// Filters other hosts before rating links.
    pub const fn same_host(mut self) -> Self {
        self.same_host = true;
        self
    }

    /// Excludes `rel="nofollow"` links before rating.
    pub const fn skip_nofollow(mut self) -> Self {
        self.skip_nofollow = true;
        self
    }
}

impl<C, S> Handler<C, FollowRelevantLinks, S> for FollowRelevantLinks
where
    C: Session,
    S: Send + Sync + 'static,
{
    async fn call(&self, cx: &Context<C>, state: &S) -> Outcome {
        let Some(registry) = cx.request().extensions().get::<ObjectiveRegistry>() else {
            return Error::invalid("FollowRelevantLinks requires objective middleware")
                .into_outcome();
        };
        if registry.is_finished() {
            return Outcome::Continue;
        }
        let Some(agent) = cx.request().extensions().get::<Agent>() else {
            return Error::invalid("FollowRelevantLinks requires .with_agent(agent)")
                .into_outcome();
        };
        let links = match Links::from_context(cx, state).await {
            Ok(links) => links.visible(),
            Err(error) => return error.into_outcome(),
        };
        let links = if self.same_host {
            links.same_host()
        } else {
            links
        };
        let mut destinations = std::collections::HashSet::new();
        let mut candidates = Vec::new();
        for link in links
            .iter()
            .filter(|link| !self.skip_nofollow || !link.has_rel("nofollow"))
        {
            if link.uri() == cx.request().uri() {
                continue;
            }
            let seed = match Seed::get(link.uri().to_string()) {
                Ok(seed) => seed,
                Err(error) => return error.into_outcome(),
            };
            if destinations.insert(seed.canonical_uri()) {
                candidates.push(Candidate {
                    url: link.uri().to_string(),
                    text: link.text().into(),
                });
            }
        }
        let scores = match registry.rate(agent, &candidates).await {
            Ok(scores) => scores,
            Err(error) => return error.into_outcome(),
        };
        for (candidate, score) in candidates.into_iter().zip(scores) {
            if score == 0 {
                continue;
            }
            let link = Link::new(candidate.url)
                .with_priority(score)
                .with_tag(self.tag.clone());
            if let Err(error) = cx.queue().follow(link).await {
                return error.into_outcome();
            }
        }
        Outcome::Continue
    }
}
