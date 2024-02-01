use spire::extract::driver::{Browser, Firefox};
use spire::extract::queue::TaskQueue;
use spire::extract::State;

// #[tracing::instrument]
pub async fn home_pagination(
    queue: TaskQueue,
    Browser(browser): Browser<Firefox>,
    State(links): State<u32>,
) {
}

// #[tracing::instrument]
pub async fn individual_page(
    queue: TaskQueue,
    Browser(browser): Browser<Firefox>,
    State(links): State<u32>,
) {
}
