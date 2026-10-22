//! Helpers shared by the frontier's tests.

use crate::frontier::seed::Seed;
use crate::frontier::{Frontier, Pop};

pub(crate) fn seed(path: &str) -> Seed {
    Seed::get(format!("https://example.com/{path}")).unwrap()
}

pub(crate) async fn pop_path<F: Frontier>(frontier: &F) -> Option<String> {
    match frontier.pop().await.unwrap() {
        Pop::Ready(seed) => Some(seed.uri().path().trim_start_matches('/').to_owned()),
        Pop::Wait(_) | Pop::Empty => None,
    }
}
