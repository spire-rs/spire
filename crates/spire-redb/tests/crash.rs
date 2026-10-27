//! A crawl killed mid-request resumes from its redb store.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use spire_core::backend::Noop;
use spire_core::crawler::Crawler;
use spire_core::dataset::InMemDataset;
use spire_core::frontier::FrontierExt;
use spire_core::frontier::seed::Seed;
use spire_core::worker::from_fn;
use spire_core::{Context, Error};
use spire_redb::RedbStore;
use tokio::sync::Notify;

struct TempPath(PathBuf);

impl Drop for TempPath {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

/// Visits `/0` through `/9`, each page linking to the next; hangs on
/// `hang_on` after signalling `reached`.
fn chain(hang_on: Option<u32>, reached: Arc<Notify>) -> impl spire_core::worker::Worker<Noop> {
    from_fn(move |cx: Context<Noop>| {
        let reached = reached.clone();
        async move {
            let page: u32 = cx.request().uri().path()[1..].parse().unwrap();
            if Some(page) == hang_on {
                reached.notify_one();
                std::future::pending::<()>().await;
            }
            cx.dataset::<u32>()?.write(page).await?;
            if page < 9 {
                cx.queue().follow(format!("/{}", page + 1)).await?;
            }
            Ok::<_, Error>(())
        }
    })
}

/// Opens the store, retrying while the killed crawl's handles are released.
async fn reopen(path: &PathBuf) -> RedbStore {
    for _ in 0..100 {
        if let Ok(store) = RedbStore::open(path).await {
            return store;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    panic!("the store stayed locked");
}

#[tokio::test(flavor = "multi_thread")]
async fn resumes_after_a_crash() {
    let path = TempPath(
        std::env::temp_dir().join(format!("spire-redb-crash-{}.redb", std::process::id())),
    );
    let start = || Seed::get("https://example.com/0").unwrap();

    // First run: crawl pages 0 to 2, then die while page 3 is in flight.
    let first = InMemDataset::<u32>::new();
    let reached = Arc::new(Notify::new());
    let store = RedbStore::open(&path.0).await.unwrap();
    let crawler = Crawler::new(Noop, chain(Some(3), reached.clone()))
        .with_concurrency(1)
        .with_frontier(store.frontier().dedup_with(store.seen()))
        .with_dataset(first.clone())
        .with_seed(start());
    drop(store);
    let running = tokio::spawn(async move { crawler.run().await });
    reached.notified().await;
    running.abort();
    let _ = running.await;

    // Second run: the same file resumes at page 3; the seed is already seen.
    let second = InMemDataset::<u32>::new();
    let store = reopen(&path.0).await;
    let crawler = Crawler::new(Noop, chain(None, Arc::new(Notify::new())))
        .with_frontier(store.frontier().dedup_with(store.seen()))
        .with_dataset(second.clone())
        .with_seed(start());
    let stats = crawler.run().await.unwrap();

    let first = spire_core::dataset::Data::new(first)
        .read_all()
        .await
        .unwrap();
    let second = spire_core::dataset::Data::new(second)
        .read_all()
        .await
        .unwrap();
    assert_eq!(first, [0, 1, 2]);
    assert_eq!(second, [3, 4, 5, 6, 7, 8, 9]);
    assert_eq!(stats.requests, 7);
    assert_eq!(store.frontier().in_flight().unwrap(), 0);
}

#[tokio::test(flavor = "multi_thread")]
async fn datasets_in_the_store_resume_after_an_aborted_crawl() {
    let path = TempPath(
        std::env::temp_dir().join(format!("spire-redb-crash-data-{}.redb", std::process::id())),
    );
    let start = || Seed::get("https://example.com/0").unwrap();

    // First run: pages 0 to 2 are written to the store, then the crawl dies.
    let reached = Arc::new(Notify::new());
    let store = RedbStore::open(&path.0).await.unwrap();
    let crawler = Crawler::new(Noop, chain(Some(3), reached.clone()))
        .with_concurrency(1)
        .with_frontier(store.frontier().dedup_with(store.seen()))
        .with_dataset(store.dataset::<u32>("pages"))
        .with_seed(start());
    drop(store);
    let running = tokio::spawn(async move { crawler.run().await });
    reached.notified().await;
    running.abort();
    let _ = running.await;

    // Second run: the same dataset, so the file ends up with every page.
    let store = reopen(&path.0).await;
    let pages = store.dataset::<u32>("pages");
    assert_eq!(pages.len().unwrap(), 3);
    let crawler = Crawler::new(Noop, chain(None, Arc::new(Notify::new())))
        .with_frontier(store.frontier().dedup_with(store.seen()))
        .with_dataset(pages.clone())
        .with_seed(start());
    crawler.run().await.unwrap();

    let pages = spire_core::dataset::Data::new(pages)
        .read_all()
        .await
        .unwrap();
    assert_eq!(pages, [0, 1, 2, 3, 4, 5, 6, 7, 8, 9]);
}
