use std::collections::{HashMap, VecDeque};
use std::sync::Mutex;

use futures::future::BoxFuture;

use super::Cache;
use crate::Result;

/// A bounded FIFO answer cache. Zero capacity disables storage.
#[derive(Debug)]
pub struct InMemCache {
    capacity: usize,
    entries: Mutex<Entries>,
}

#[derive(Debug, Default)]
struct Entries {
    values: HashMap<String, String>,
    order: VecDeque<String>,
}

impl InMemCache {
    /// Holds at most `capacity` answers, evicting oldest entries first.
    #[must_use]
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity,
            entries: Mutex::default(),
        }
    }
}

impl Default for InMemCache {
    fn default() -> Self {
        Self::new(1024)
    }
}

impl Cache for InMemCache {
    fn get<'a>(&'a self, key: &'a str) -> BoxFuture<'a, Result<Option<String>>> {
        Box::pin(async move {
            Ok(self
                .entries
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .values
                .get(key)
                .cloned())
        })
    }

    fn put<'a>(&'a self, key: &'a str, answer: &'a str) -> BoxFuture<'a, Result<()>> {
        Box::pin(async move {
            if self.capacity == 0 {
                return Ok(());
            }
            let mut entries = self
                .entries
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if !entries.values.contains_key(key) {
                if entries.values.len() == self.capacity
                    && let Some(oldest) = entries.order.pop_front()
                {
                    entries.values.remove(&oldest);
                }
                entries.order.push_back(key.to_owned());
            }
            entries.values.insert(key.to_owned(), answer.to_owned());
            Ok(())
        })
    }
}
