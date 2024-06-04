use std::cmp::max;
use std::collections::{HashMap, HashSet};
use std::collections::hash_map::Entry;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use crate::context::{Context, Signal, Tag, TagQuery};
use crate::Result;

/// Storage for all [`Tag`] associated data.
#[derive(Default)]
pub struct TagData {
    // Fallback means all not-yet encountered tags.
    defer: Mutex<HashMap<Tag, Instant>>,
    block: Mutex<HashSet<Tag>>,
}

impl TagData {
    /// Creates a new [`TagData`].
    pub fn new() -> Self {
        Self::default()
    }

    /// Modifies [`Context`] by attaching associated defer and block metadata.
    pub async fn prepare<C>(&self, cx: &mut Context<C>) -> Result<()> {
        // TODO: Find defers.
        // TODO: Find blocks.
        // TODO: Apply both defer and block.

        Ok(())
    }

    /// Applies the signal to the subsequent requests.
    pub fn notify(&self, signal: Signal, owner: Tag) -> Result<()> {
        match signal {
            Signal::Wait(x, t) | Signal::Hold(x, t) => self.apply_defer(x, owner, t),
            Signal::Fail(x, _) => self.apply_block(x, owner),
            _ => Ok(()),
        }
    }

    /// Defers all [`Tag`]s as specified per [`TagQuery`].
    fn apply_defer(&self, query: TagQuery, owner: Tag, duration: Duration) -> Result<()> {
        let minimum = Instant::now() + duration;
        let mut defer = self.defer.lock().unwrap();

        let mut defer_one = |x: Tag| {
            let _ = match defer.entry(x) {
                Entry::Occupied(mut x) => x.insert(max(*x.get() + duration, minimum)),
                Entry::Vacant(x) => *x.insert(minimum),
            };
        };

        match query {
            TagQuery::Owner => defer_one(owner),
            TagQuery::Single(x) => defer_one(x),
            TagQuery::Every => defer_one(Tag::Fallback),
            TagQuery::List(x) => x.into_iter().for_each(defer_one),
        };

        Ok(())
    }

    /// Blocks all [`Tag`]s as specified per [`TagQuery`].
    fn apply_block(&self, query: TagQuery, owner: Tag) -> Result<()> {
        let mut guard = self.block.lock().unwrap();
        let mut block_one = |x: Tag| {
            guard.insert(x);
        };

        match query {
            TagQuery::Owner => block_one(owner),
            TagQuery::Every => block_one(Tag::Fallback),
            TagQuery::Single(x) => block_one(x),
            TagQuery::List(x) => x.into_iter().for_each(block_one),
        }

        Ok(())
    }
}
