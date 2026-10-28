//! Helpers shared by the frontier's tests.

use crate::frontier::seed::Seed;
use crate::frontier::{Frontier, NextSeed};

pub(crate) fn seed(path: &str) -> Seed {
    Seed::get(format!("https://example.com/{path}")).unwrap()
}

pub(crate) async fn pop_path<F: Frontier>(frontier: &F) -> Option<String> {
    match frontier.pop().await.unwrap() {
        NextSeed::Ready(seed) => Some(seed.uri().path().trim_start_matches('/').to_owned()),
        NextSeed::Wait(_) | NextSeed::Empty => None,
    }
}
