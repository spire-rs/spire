//! Helpers shared by the frontier's tests.

use crate::frontier::{Frontier, Next, Seed};

pub(crate) fn seed(path: &str) -> Seed {
    Seed::get(format!("https://example.com/{path}")).unwrap()
}

pub(crate) async fn pop_path<F: Frontier>(frontier: &F) -> Option<String> {
    match frontier.pop().await.unwrap() {
        Next::Ready(seed) => Some(seed.uri().path().trim_start_matches('/').to_owned()),
        Next::Wait(_) | Next::Empty => None,
    }
}
