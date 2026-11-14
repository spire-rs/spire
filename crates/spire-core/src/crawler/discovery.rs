//! A discovery phase followed by priority-ordered page handling.

use std::cmp::Reverse;
use std::collections::VecDeque;

use crate::crawler::Tag;
use crate::crawler::admission::Candidate;

pub(super) enum Discovery {
    Gathering { tag: Tag, pages: Vec<Candidate> },
    Ready(VecDeque<Candidate>),
}

impl Discovery {
    pub(super) fn new(tag: Option<Tag>) -> Self {
        tag.map_or_else(
            || Self::Ready(VecDeque::new()),
            |tag| Self::Gathering {
                tag,
                pages: Vec::new(),
            },
        )
    }

    pub(super) fn admit(&mut self, seed: Candidate) -> Option<Candidate> {
        if let Self::Gathering { tag, pages } = self
            && seed.tag() != Some(tag)
        {
            pages.push(seed);
            return None;
        }
        Some(seed)
    }

    pub(super) fn mode(&self) -> crate::frontier::PopMode {
        match self {
            Self::Gathering { .. } => crate::frontier::PopMode::Queued,
            Self::Ready(_) => crate::frontier::PopMode::All,
        }
    }

    pub(super) fn pop(&mut self) -> Option<Candidate> {
        match self {
            Self::Gathering { .. } => None,
            Self::Ready(pages) => pages.pop_front(),
        }
    }

    pub(super) fn finish(&mut self) -> bool {
        let Self::Gathering { pages, .. } = self else {
            return false;
        };
        pages.sort_by_key(|seed| Reverse(seed.priority()));
        *self = Self::Ready(std::mem::take(pages).into());
        true
    }

    pub(super) fn drain(&mut self) -> Vec<Candidate> {
        match self {
            Self::Gathering { pages, .. } => std::mem::take(pages),
            Self::Ready(pages) => pages.drain(..).collect(),
        }
    }
}
