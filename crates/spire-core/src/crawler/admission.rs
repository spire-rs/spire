//! Work awaiting robots.txt or host admission.

use std::collections::{HashMap, VecDeque};
use std::ops::Deref;

use tokio::task::Id;
use tokio::time::Instant;

use crate::backend::redirect::RedirectRequest;
use crate::crawler::Tag;
use crate::frontier::seed::Seed;

/// An unchanged frontier occurrence with its stable scheduling order.
/// The order survives host and tag waits; it is never stored in seed
/// extensions.
#[derive(Debug)]
pub(crate) struct Candidate {
    pub(crate) seed: Seed,
    pub(super) order: usize,
}

impl Candidate {
    pub(super) fn new(seed: Seed, order: usize) -> Self {
        Self { seed, order }
    }
}

impl Deref for Candidate {
    type Target = Seed;

    fn deref(&self) -> &Seed {
        &self.seed
    }
}

/// A frontier occurrence or a continuation of an already-running request.
/// The continuation owns its reply channel and is never serialized as a seed.
#[derive(Debug)]
pub(crate) enum Pending {
    Seed(Candidate),
    Redirect(RedirectRequest),
}

impl Pending {
    pub(crate) fn task(&self) -> Option<Id> {
        match self {
            Self::Seed(_) => None,
            Self::Redirect(redirect) => Some(redirect.task),
        }
    }

    pub(crate) fn is_cancelled(&self) -> bool {
        matches!(self, Self::Redirect(redirect) if redirect.answer.is_closed())
    }

    pub(crate) fn is_redirect(&self) -> bool {
        matches!(self, Self::Redirect(_))
    }
}

impl From<Candidate> for Pending {
    fn from(seed: Candidate) -> Self {
        Self::Seed(seed)
    }
}

#[cfg(test)]
impl From<Seed> for Pending {
    fn from(seed: Seed) -> Self {
        Self::Seed(Candidate::new(seed, 0))
    }
}

impl Deref for Pending {
    type Target = Seed;

    fn deref(&self) -> &Seed {
        match self {
            Self::Seed(seed) => seed,
            Self::Redirect(redirect) => &redirect.seed,
        }
    }
}

/// Robots-approved occurrences waiting for a global slot and an unpaused tag.
/// Occurrences are moved intact, preserving persistent acknowledgment tokens.
#[derive(Default)]
pub(super) struct Ready {
    queued: VecDeque<Candidate>,
    paused: HashMap<Option<Tag>, Instant>,
}

impl Ready {
    pub(super) fn push(&mut self, seed: Candidate) {
        let index = self
            .queued
            .partition_point(|queued| queued.order <= seed.order);
        self.queued.insert(index, seed);
    }

    fn is_paused(&self, seed: &Seed, now: Instant) -> bool {
        self.paused
            .get(&seed.tag().cloned())
            .is_some_and(|until| *until > now)
    }

    /// Host waiters must recheck pauses set after they left this queue.
    pub(super) fn admit(&mut self, seed: Candidate, now: Instant) -> Option<Candidate> {
        if self.is_paused(&seed, now) {
            self.push(seed);
            None
        } else {
            Some(seed)
        }
    }

    pub(super) fn pop(&mut self, now: Instant) -> Option<Candidate> {
        self.paused.retain(|_, until| *until > now);
        let index = self
            .queued
            .iter()
            .position(|seed| !self.is_paused(seed, now))?;
        self.queued.remove(index)
    }

    pub(super) fn pause(&mut self, tag: Option<Tag>, until: Instant) {
        self.paused
            .entry(tag)
            .and_modify(|previous| *previous = (*previous).max(until))
            .or_insert(until);
    }

    pub(super) fn unpause(&mut self, tag: Option<&Tag>) {
        self.paused.remove(&tag.cloned());
    }

    /// Expired deadlines wake an idle scheduler, but must not spin it while
    /// every global slot is occupied.
    pub(super) fn earliest(&self, now: Instant, has_slot: bool) -> Option<Instant> {
        self.queued
            .iter()
            .filter_map(|seed| self.paused.get(&seed.tag().cloned()))
            .filter(|until| has_slot || **until > now)
            .min()
            .copied()
    }

    pub(super) fn len(&self) -> usize {
        self.queued.len()
    }

    pub(super) fn drain(&mut self) -> impl Iterator<Item = Candidate> + '_ {
        self.queued.drain(..)
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    fn seed(path: &str, tag: Option<&str>) -> Candidate {
        let seed = Seed::get(format!("https://example.com/{path}")).unwrap();
        Candidate::new(
            match tag {
                Some(tag) => seed.with_tag(tag),
                None => seed,
            },
            0,
        )
    }

    #[test]
    fn pauses_merge_and_eligible_occurrences_keep_their_order_and_metadata() {
        use std::sync::Arc;
        let now = Instant::now();
        let until = now + Duration::from_secs(10);
        let mut ready = Ready::default();
        ready.pause(None, until);
        ready.pause(None, now + Duration::from_secs(1));
        ready.pause(Some(Tag::from("a")), until);
        let marker = Arc::new(7_u8);
        let mut first = seed("first", None);
        first.seed.extensions_mut().insert(marker.clone());
        ready.push(first);
        ready.push(seed("second", Some("a")));
        ready.push(seed("free", Some("b")));
        assert_eq!(ready.pop(now).unwrap().uri().path(), "/free");
        assert_eq!(ready.earliest(now, true), Some(until));
        assert!(ready.pop(until - Duration::from_nanos(1)).is_none());
        assert_eq!(ready.earliest(until, false), None);
        assert_eq!(ready.earliest(until, true), Some(until));
        let first = ready.pop(until).unwrap();
        assert_eq!(first.uri().path(), "/first");
        assert!(Arc::ptr_eq(
            first.extensions().get::<Arc<u8>>().unwrap(),
            &marker
        ));
        assert_eq!(ready.pop(until).unwrap().uri().path(), "/second");
    }

    #[test]
    fn a_host_waiter_keeps_its_original_place_when_a_new_pause_parks_it_again() {
        let now = Instant::now();
        let mut ready = Ready::default();
        let first = seed("first", Some("a"));
        ready.push(first);
        let first = ready.pop(now).unwrap();
        let mut second = seed("second", Some("a"));
        second.order = 1;
        ready.pause(Some(Tag::from("a")), now + Duration::from_secs(5));
        ready.push(second);
        assert!(ready.admit(first, now).is_none());
        assert_eq!(
            ready
                .pop(now + Duration::from_secs(5))
                .unwrap()
                .uri()
                .path(),
            "/first"
        );
        assert_eq!(
            ready
                .pop(now + Duration::from_secs(5))
                .unwrap()
                .uri()
                .path(),
            "/second"
        );
    }

    #[test]
    fn unpausing_releases_queued_work_and_unused_pauses_do_not_keep_a_run_alive() {
        let now = Instant::now();
        let mut ready = Ready::default();
        let tag = Some(Tag::from("a"));
        ready.pause(tag.clone(), now + Duration::from_hours(1));
        assert_eq!(ready.earliest(now, true), None);
        ready.push(seed("page", Some("a")));
        assert!(ready.pop(now).is_none());
        ready.unpause(tag.as_ref());
        assert_eq!(ready.pop(now).unwrap().uri().path(), "/page");
    }
}
