//! A crawl whose process is killed resumes from its redb store, losing
//! nothing, whether commits are synced always or in batches, and so does a
//! dataset stored next to its frontier.
//!
//! The test runs itself again as a child process that aborts mid-crawl.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;

use spire_core::backend::Noop;
use spire_core::crawler::Crawler;
use spire_core::dataset::Data;
use spire_core::frontier::FrontierExt;
use spire_core::frontier::seed::Seed;
use spire_core::worker::from_fn;
use spire_core::{Context, Error};
use spire_redb::RedbStore;

const CHILD: &str = "SPIRE_REDB_CHILD";

/// Pages visited by the child, one per line, written before the crash.
fn visited_path(db: &Path) -> PathBuf {
    db.with_extension("visited")
}

async fn open(db: &Path, always: bool) -> RedbStore {
    let builder = RedbStore::builder();
    let builder = if always {
        builder.sync_always()
    } else {
        builder
    };
    builder.open(db).await.unwrap()
}

/// Crawls `/0` to `/9`, each page linking to the next, recording each page
/// in `log` and in the store's `pages` dataset; aborts the process on
/// `abort_on`.
async fn crawl(db: &Path, always: bool, abort_on: Option<u32>, log: PathBuf) {
    let store = open(db, always).await;
    let worker = from_fn(move |cx: Context<Noop>| {
        let log = log.clone();
        async move {
            let page: u32 = cx.request().uri().path()[1..].parse().unwrap();
            if Some(page) == abort_on {
                std::process::abort();
            }
            cx.dataset::<u32>()?.write(page).await?;
            let mut file = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&log)
                .unwrap();
            writeln!(file, "{page}").unwrap();
            if page < 9 {
                cx.queue().follow(format!("/{}", page + 1)).await?;
            }
            Ok::<_, Error>(())
        }
    });

    let crawler = Crawler::new(Noop, worker)
        .with_concurrency(1)
        .with_frontier(store.frontier().dedup_with(store.seen()))
        .with_dataset(store.dataset::<u32>("pages"))
        .with_seed(Seed::get("https://example.com/0").unwrap());
    crawler.run().await.unwrap();
}

fn pages(log: &Path) -> Vec<u32> {
    let text = std::fs::read_to_string(log).unwrap_or_default();
    text.lines().map(|line| line.parse().unwrap()).collect()
}

/// Runs in the child process only: crawls until page 3, then aborts.
#[tokio::test]
async fn child() {
    let Ok(db) = std::env::var(CHILD) else {
        return;
    };
    let db = PathBuf::from(db);
    let always = std::env::var("SPIRE_REDB_ALWAYS").is_ok();
    crawl(&db, always, Some(3), visited_path(&db)).await;
    unreachable!("the crawl aborts on page 3");
}

/// Kills a child crawl on page 3, resumes it, and returns the pages of both
/// runs, and the pages in the store's dataset after the second.
async fn crash_and_resume(always: bool) -> (Vec<u32>, Vec<u32>, Vec<u32>) {
    let name = format!("spire-redb-process-{}-{always}.redb", std::process::id());
    let db = std::env::temp_dir().join(name);
    let _ = std::fs::remove_file(&db);
    let _ = std::fs::remove_file(visited_path(&db));

    let mut child = Command::new(std::env::current_exe().unwrap());
    child
        .args(["--exact", "child", "--nocapture"])
        .env(CHILD, &db);
    if always {
        child.env("SPIRE_REDB_ALWAYS", "1");
    }
    let status = child.status().unwrap();
    assert!(!status.success(), "the child should have aborted");
    let before = pages(&visited_path(&db));

    let resumed = visited_path(&db).with_extension("resumed");
    let _ = std::fs::remove_file(&resumed);
    crawl(&db, always, None, resumed.clone()).await;
    let after = pages(&resumed);
    let stored = Data::new(open(&db, always).await.dataset::<u32>("pages"))
        .read_all()
        .await
        .unwrap();

    for path in [db.clone(), visited_path(&db), resumed] {
        let _ = std::fs::remove_file(path);
    }
    (before, after, stored)
}

#[tokio::test(flavor = "multi_thread")]
async fn sync_always_repeats_only_the_request_in_flight() {
    if std::env::var(CHILD).is_ok() {
        return;
    }
    let (before, after, stored) = crash_and_resume(true).await;
    assert_eq!(before, [0, 1, 2]);
    assert_eq!(after, [3, 4, 5, 6, 7, 8, 9]);
    // The child's items were synced, and the second run added the rest.
    assert_eq!(stored, (0..10).collect::<Vec<_>>());
}

#[tokio::test(flavor = "multi_thread")]
async fn batched_sync_repeats_work_but_loses_nothing() {
    if std::env::var(CHILD).is_ok() {
        return;
    }
    let (before, after, stored) = crash_and_resume(false).await;
    assert_eq!(before, [0, 1, 2]);
    // Nothing was synced before the crash, so the crawl starts over from the
    // seed: pages are repeated, none is lost.
    assert_eq!(after, (0..10).collect::<Vec<_>>());
    // The child's items were rolled back with the frontier, so the repeated
    // pages are stored once.
    assert_eq!(stored, (0..10).collect::<Vec<_>>());
}
