use std::fs::{File, OpenOptions};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use spire_core::frontier::seed::Seed;
use spire_core::frontier::{Ack, Admission, Frontier, NextSeed, Report, Schedule};
use spire_core::{Error, ErrorKind, Result};
use sqlx::{Sqlite, SqlitePool};
use tokio::time::Instant;

use crate::error::frontier_error;

/// A persistent priority frontier with exclusive ownership and crash recovery.
///
/// Higher priorities run first; equal priorities use insertion order. Due
/// revisits compete with fresh seeds by priority. Popped seeds remain stored
/// until reported, and abandoned seeds are recovered on the next exclusive
/// open. Cancellation requeues immediately. Retry facades retain their seeds
/// in memory; recovery restarts those requests at their stored attempt.
///
/// An OS lock on `<database>.spire-frontier.lock` protects one frontier owner
/// per database file. Clones share ownership. The lock file is deliberately
/// not deleted, preventing races involving different lock-file inodes. Do not
/// remove it or access one database through hard-link aliases. Use a local
/// filesystem supporting advisory locks, not a distributed filesystem.
#[derive(Debug, Clone)]
pub struct SqliteFrontier {
    pub(super) inner: Arc<Inner>,
}

#[derive(Debug)]
pub(super) struct Inner {
    pool: SqlitePool,
    _lock: File,
    epoch: i64,
    started: Instant,
}

fn invalid(message: impl Into<String>) -> Error {
    Error::new(ErrorKind::Frontier).with_message(message)
}

fn encode(seed: &Seed) -> Result<String> {
    serde_json::to_string(seed).map_err(frontier_error)
}

fn decode(payload: &str) -> Result<Seed> {
    serde_json::from_str(payload).map_err(frontier_error)
}

// Reports may have a different retry attempt. All other request properties
// identify which stored occurrence to acknowledge, including identical URLs
// submitted under different tags, headers or identities.
fn matches(stored: &Seed, reported: &Seed) -> bool {
    stored.method() == reported.method()
        && stored.uri() == reported.uri()
        && stored.headers() == reported.headers()
        && stored.body() == reported.body()
        && stored.tag() == reported.tag()
        && stored.depth() == reported.depth()
        && stored.priority() == reported.priority()
        && stored.identity() == reported.identity()
}

impl SqliteFrontier {
    pub(super) async fn open(pool: SqlitePool) -> Result<Self> {
        let filename: String =
            sqlx::query_scalar("SELECT file FROM pragma_database_list WHERE name = 'main'")
                .fetch_one(&pool)
                .await
                .map_err(frontier_error)?;
        if filename.is_empty() {
            return Err(invalid(
                "persistent SQLite frontiers require a file-backed database",
            ));
        }
        let file = tokio::task::spawn_blocking(move || -> Result<File> {
            let path = std::fs::canonicalize(filename).map_err(frontier_error)?;
            let mut name = path.into_os_string();
            name.push(".spire-frontier.lock");
            let file = OpenOptions::new()
                .read(true)
                .write(true)
                .create(true)
                .truncate(false)
                .open(PathBuf::from(name))
                .map_err(frontier_error)?;
            file.try_lock().map_err(|error| {
                invalid(format!(
                    "cannot acquire exclusive SQLite frontier ownership: {error}"
                ))
            })?;
            Ok(file)
        })
        .await
        .map_err(frontier_error)??;
        let mut transaction = pool
            .begin_with("BEGIN IMMEDIATE")
            .await
            .map_err(frontier_error)?;
        sqlx::query("UPDATE spire_frontier_items SET in_flight = 0 WHERE in_flight = 1")
            .execute(&mut *transaction)
            .await
            .map_err(frontier_error)?;
        transaction.commit().await.map_err(frontier_error)?;
        let epoch = i64::try_from(
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis(),
        )
        .unwrap_or(i64::MAX);
        Ok(Self {
            inner: Arc::new(Inner {
                pool,
                _lock: file,
                epoch,
                started: Instant::now(),
            }),
        })
    }

    fn now(&self) -> i64 {
        self.inner.epoch.saturating_add(
            i64::try_from(self.inner.started.elapsed().as_millis()).unwrap_or(i64::MAX),
        )
    }

    /// Counts popped requests not yet acknowledged.
    ///
    /// # Errors
    /// Returns a frontier error if the query or count conversion fails.
    pub async fn in_flight(&self) -> Result<usize> {
        let count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM spire_frontier_items WHERE in_flight = 1")
                .fetch_one(&self.inner.pool)
                .await
                .map_err(frontier_error)?;
        usize::try_from(count).map_err(frontier_error)
    }
}

async fn enqueue(transaction: &mut sqlx::Transaction<'_, Sqlite>, seed: &Seed) -> Result<()> {
    sqlx::query(
        "INSERT INTO spire_frontier_items (fingerprint, priority, payload) VALUES ($1, $2, $3)",
    )
    .bind(seed.fingerprint().get().to_be_bytes().to_vec())
    .bind(seed.priority())
    .bind(encode(seed)?)
    .execute(&mut **transaction)
    .await
    .map_err(frontier_error)?;
    Ok(())
}

impl Frontier for SqliteFrontier {
    async fn push(&self, seed: Seed) -> Result<Admission> {
        sqlx::query(
            "INSERT INTO spire_frontier_items (fingerprint, priority, payload) VALUES ($1, $2, $3)",
        )
        .bind(seed.fingerprint().get().to_be_bytes().to_vec())
        .bind(seed.priority())
        .bind(encode(&seed)?)
        .execute(&self.inner.pool)
        .await
        .map_err(frontier_error)?;
        Ok(Admission::Queued)
    }

    async fn pop(&self) -> Result<NextSeed> {
        let mut transaction = self
            .inner
            .pool
            .begin_with("BEGIN IMMEDIATE")
            .await
            .map_err(frontier_error)?;
        let now = self.now();
        sqlx::query("INSERT INTO spire_frontier_items (fingerprint, priority, payload)
            SELECT fingerprint, priority, payload FROM spire_scheduled WHERE due <= $1 ORDER BY due, fingerprint")
            .bind(now).execute(&mut *transaction).await.map_err(frontier_error)?;
        sqlx::query("DELETE FROM spire_scheduled WHERE due <= $1")
            .bind(now)
            .execute(&mut *transaction)
            .await
            .map_err(frontier_error)?;
        let next: Option<(i64, String)> = sqlx::query_as(
            "SELECT id, payload FROM spire_frontier_items WHERE in_flight = 0 ORDER BY priority DESC, id LIMIT 1",
        ).fetch_optional(&mut *transaction).await.map_err(frontier_error)?;
        let Some((id, payload)) = next else {
            transaction.commit().await.map_err(frontier_error)?;
            return Ok(NextSeed::Empty);
        };
        // Corrupt seeds remain stored and return an error, rather than being
        // silently discarded or allowing an incomplete recovery.
        let seed = decode(&payload)?;
        sqlx::query("UPDATE spire_frontier_items SET in_flight = 1 WHERE id = $1")
            .bind(id)
            .execute(&mut *transaction)
            .await
            .map_err(frontier_error)?;
        transaction.commit().await.map_err(frontier_error)?;
        Ok(NextSeed::Ready(seed))
    }

    async fn report(&self, report: Report) -> Result<Ack> {
        let (seed, requeue, ack) = match report {
            Report::Done(seed) | Report::Failed(seed) | Report::Skipped(seed) => {
                (seed, false, Ack::Applied)
            }
            Report::Cancelled(seed) => (seed, true, Ack::Applied),
            Report::Retry { seed, .. } => (seed, false, Ack::RetryRefused),
            _ => return Ok(Ack::Applied),
        };
        let mut transaction = self
            .inner
            .pool
            .begin_with("BEGIN IMMEDIATE")
            .await
            .map_err(frontier_error)?;
        let candidates: Vec<(i64, String)> = sqlx::query_as(
            "SELECT id, payload FROM spire_frontier_items WHERE fingerprint = $1 AND in_flight = 1 ORDER BY id",
        ).bind(seed.fingerprint().get().to_be_bytes().to_vec())
            .fetch_all(&mut *transaction).await.map_err(frontier_error)?;
        let mut found = None;
        for (id, payload) in candidates {
            if matches(&decode(&payload)?, &seed) {
                found = Some(id);
                break;
            }
        }
        if let Some(id) = found {
            sqlx::query("DELETE FROM spire_frontier_items WHERE id = $1")
                .bind(id)
                .execute(&mut *transaction)
                .await
                .map_err(frontier_error)?;
            if requeue {
                enqueue(&mut transaction, &seed).await?;
            }
        }
        transaction.commit().await.map_err(frontier_error)?;
        Ok(ack)
    }
}

impl Schedule for SqliteFrontier {
    async fn schedule(&self, seed: Seed, due: Instant) -> Result<()> {
        let due = self.inner.epoch.saturating_add(
            i64::try_from(
                due.saturating_duration_since(self.inner.started)
                    .as_millis(),
            )
            .unwrap_or(i64::MAX),
        );
        sqlx::query("INSERT INTO spire_scheduled (fingerprint, due, priority, payload) VALUES ($1, $2, $3, $4)
            ON CONFLICT(fingerprint) DO UPDATE SET due = excluded.due, priority = excluded.priority, payload = excluded.payload")
            .bind(seed.fingerprint().get().to_be_bytes().to_vec()).bind(due).bind(seed.priority()).bind(encode(&seed)?)
            .execute(&self.inner.pool).await.map_err(frontier_error)?;
        Ok(())
    }

    async fn next_due(&self) -> Result<Option<Instant>> {
        let due: Option<i64> = sqlx::query_scalar("SELECT MIN(due) FROM spire_scheduled")
            .fetch_one(&self.inner.pool)
            .await
            .map_err(frontier_error)?;
        due.map(|due| {
            let wait = u64::try_from(due.saturating_sub(self.now())).unwrap_or_default();
            Instant::now()
                .checked_add(Duration::from_millis(wait))
                .ok_or_else(|| invalid("scheduled SQLite revisit exceeds the clock range"))
        })
        .transpose()
    }
}

#[cfg(test)]
mod tests {
    use std::process::Command;

    use spire_core::frontier::FrontierExt;
    use spire_core::frontier::facade::RetryPolicy;
    use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
    use tempfile::TempDir;

    use super::*;
    use crate::sqlite::SqliteStore;

    fn seed(path: &str, priority: i32) -> Seed {
        Seed::get(format!("https://example.com/{path}"))
            .unwrap()
            .with_priority(priority)
    }

    async fn store(path: &std::path::Path) -> SqliteStore {
        let pool = SqlitePoolOptions::new()
            .max_connections(2)
            .connect_with(
                SqliteConnectOptions::new()
                    .filename(path)
                    .create_if_missing(true),
            )
            .await
            .unwrap();
        let store = SqliteStore::new(pool);
        store.initialize().await.unwrap();
        store
    }

    async fn pop(frontier: &SqliteFrontier) -> Seed {
        match frontier.pop().await.unwrap() {
            NextSeed::Ready(seed) => seed,
            other => panic!("expected a seed, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn priority_fifo_exclusive_ownership_and_recovery() {
        let directory = TempDir::new().unwrap();
        let path = directory.path().join("test.sqlite");
        let first = store(&path).await;
        let independent = store(&path).await;
        let frontier = first.frontier().await.unwrap();
        let shared = first.clone().frontier().await.unwrap();
        assert!(independent.frontier().await.is_err());
        for (path, priority) in [
            ("first", 0),
            ("low", i32::MIN),
            ("high", i32::MAX),
            ("second", 0),
        ] {
            frontier.push(seed(path, priority)).await.unwrap();
        }
        let high = pop(&shared).await;
        assert_eq!(high.uri().path(), "/high");
        assert_eq!(frontier.in_flight().await.unwrap(), 1);
        frontier.report(Report::Done(high)).await.unwrap();
        let abandoned = pop(&frontier).await;
        assert_eq!(abandoned.uri().path(), "/first");
        drop(frontier);
        assert!(
            independent.frontier().await.is_err(),
            "the shared handle still owns the lock"
        );
        drop(shared);
        let recovered = independent.frontier().await.unwrap();
        assert_eq!(recovered.in_flight().await.unwrap(), 0);
        assert_eq!(pop(&recovered).await.uri().path(), "/first");
        assert_eq!(pop(&recovered).await.uri().path(), "/second");
        assert_eq!(pop(&recovered).await.uri().path(), "/low");
        drop(recovered);
        first.pool().close().await;
        independent.pool().close().await;
    }

    #[tokio::test]
    async fn reports_preserve_duplicate_occurrences_and_cancelled_requests() {
        let directory = TempDir::new().unwrap();
        let store = store(&directory.path().join("test.sqlite")).await;
        let frontier = store.frontier().await.unwrap();
        frontier.push(seed("same", 0).with_tag("a")).await.unwrap();
        frontier.push(seed("same", 0).with_tag("b")).await.unwrap();
        frontier.push(seed("same", 0).with_tag("b")).await.unwrap();
        let a = pop(&frontier).await;
        let b = pop(&frontier).await;
        let duplicate = pop(&frontier).await;
        frontier.report(Report::Done(b)).await.unwrap();
        assert_eq!(frontier.in_flight().await.unwrap(), 2);
        frontier.report(Report::Skipped(a)).await.unwrap();
        frontier.report(Report::Cancelled(duplicate)).await.unwrap();
        assert_eq!(frontier.in_flight().await.unwrap(), 0);
        let again = pop(&frontier).await;
        assert_eq!(again.tag().unwrap().as_str(), "b");
        assert_eq!(
            frontier
                .report(Report::Retry {
                    seed: again,
                    after: None
                })
                .await
                .unwrap(),
            Ack::RetryRefused
        );
        assert!(matches!(frontier.pop().await.unwrap(), NextSeed::Empty));
        drop(frontier);
        store.pool().close().await;
    }

    #[tokio::test]
    async fn scheduled_revisits_replace_and_compete_by_priority() {
        let directory = TempDir::new().unwrap();
        let store = store(&directory.path().join("test.sqlite")).await;
        let frontier = store.frontier().await.unwrap();
        let later = Instant::now() + Duration::from_secs(100);
        frontier.schedule(seed("revisit", 3), later).await.unwrap();
        frontier
            .schedule(seed("revisit", 3), later + Duration::from_secs(10))
            .await
            .unwrap();
        assert!(matches!(frontier.pop().await.unwrap(), NextSeed::Empty));
        assert!(frontier.next_due().await.unwrap().unwrap() > later);
        frontier
            .schedule(seed("revisit", 3), Instant::now())
            .await
            .unwrap();
        frontier.push(seed("fresh", 4)).await.unwrap();
        let fresh = pop(&frontier).await;
        assert_eq!(fresh.uri().path(), "/fresh");
        frontier.report(Report::Done(fresh)).await.unwrap();
        assert_eq!(pop(&frontier).await.uri().path(), "/revisit");
        assert_eq!(frontier.next_due().await.unwrap(), None);
        drop(frontier);
        store.pool().close().await;
    }

    #[tokio::test]
    async fn retry_facade_works_with_dedup_and_acknowledges_changed_attempts() {
        let directory = TempDir::new().unwrap();
        let store = store(&directory.path().join("test.sqlite")).await;
        let inner = store.frontier().await.unwrap();
        let frontier = inner
            .clone()
            .dedup_with(store.seen())
            .retry_with(RetryPolicy::new(1).with_base_delay(Duration::ZERO));
        frontier.push(seed("retry", 1)).await.unwrap();
        let NextSeed::Ready(first) = frontier.pop().await.unwrap() else {
            panic!()
        };
        assert_eq!(
            frontier
                .report(Report::Retry {
                    seed: first,
                    after: Some(Duration::ZERO)
                })
                .await
                .unwrap(),
            Ack::Applied
        );
        let NextSeed::Ready(retried) = frontier.pop().await.unwrap() else {
            panic!()
        };
        assert_eq!(retried.attempt().get(), 1);
        frontier.report(Report::Done(retried)).await.unwrap();
        assert_eq!(inner.in_flight().await.unwrap(), 0);
        drop(frontier);
        drop(inner);
        store.pool().close().await;
    }

    #[tokio::test]
    async fn memory_database_is_rejected_and_corrupt_seeds_are_not_discarded() {
        let memory = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        let store_memory = SqliteStore::new(memory.clone());
        store_memory.initialize().await.unwrap();
        assert!(store_memory.frontier().await.is_err());
        memory.close().await;
        let directory = TempDir::new().unwrap();
        let store = store(&directory.path().join("test.sqlite")).await;
        let frontier = store.frontier().await.unwrap();
        frontier.push(seed("bad", 0)).await.unwrap();
        sqlx::query("UPDATE spire_frontier_items SET payload = 'not JSON'")
            .execute(store.pool())
            .await
            .unwrap();
        assert!(frontier.pop().await.is_err());
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM spire_frontier_items")
            .fetch_one(store.pool())
            .await
            .unwrap();
        assert_eq!(count, 1);
        drop(frontier);
        store.pool().close().await;
    }

    #[tokio::test]
    async fn process_exit_releases_ownership_and_recovers_in_flight() {
        let directory = TempDir::new().unwrap();
        let path = directory.path().join("test.sqlite");
        let executable = std::env::current_exe().unwrap();
        let child_path = path.clone();
        let status = tokio::task::spawn_blocking(move || {
            Command::new(executable)
                .args([
                    "--exact",
                    "sqlite::frontier::tests::crash_child",
                    "--nocapture",
                ])
                .env("SPIRE_SQLITE_CRASH_TEST_PATH", child_path)
                .status()
                .unwrap()
        })
        .await
        .unwrap();
        assert!(status.success());
        let store = store(&path).await;
        let frontier = store.frontier().await.unwrap();
        assert_eq!(pop(&frontier).await.uri().path(), "/abandoned");
        drop(frontier);
        store.pool().close().await;
    }

    #[tokio::test]
    async fn another_process_cannot_recover_a_live_frontier() {
        let directory = TempDir::new().unwrap();
        let path = directory.path().join("test.sqlite");
        let store = store(&path).await;
        let frontier = store.frontier().await.unwrap();
        frontier.push(seed("live", 0)).await.unwrap();
        let live = pop(&frontier).await;
        let executable = std::env::current_exe().unwrap();
        let status = tokio::task::spawn_blocking(move || {
            Command::new(executable)
                .args([
                    "--exact",
                    "sqlite::frontier::tests::crash_child",
                    "--nocapture",
                ])
                .env("SPIRE_SQLITE_CRASH_TEST_PATH", path)
                .env("SPIRE_SQLITE_EXPECT_LOCKED", "1")
                .status()
                .unwrap()
        })
        .await
        .unwrap();
        assert!(status.success());
        assert_eq!(frontier.in_flight().await.unwrap(), 1);
        assert!(matches!(frontier.pop().await.unwrap(), NextSeed::Empty));
        frontier.report(Report::Done(live)).await.unwrap();
        drop(frontier);
        store.pool().close().await;
    }

    #[tokio::test]
    async fn schedules_survive_closing_and_reopening() {
        let directory = TempDir::new().unwrap();
        let path = directory.path().join("test.sqlite");
        let first = store(&path).await;
        let frontier = first.frontier().await.unwrap();
        frontier
            .schedule(seed("later", 7), Instant::now() + Duration::from_hours(1))
            .await
            .unwrap();
        drop(frontier);
        first.pool().close().await;
        let second = store(&path).await;
        let frontier = second.frontier().await.unwrap();
        assert!(frontier.next_due().await.unwrap().unwrap() > Instant::now());
        assert!(matches!(frontier.pop().await.unwrap(), NextSeed::Empty));
        sqlx::query("UPDATE spire_scheduled SET due = 0")
            .execute(second.pool())
            .await
            .unwrap();
        assert_eq!(pop(&frontier).await.uri().path(), "/later");
        drop(frontier);
        second.pool().close().await;
    }

    #[tokio::test]
    async fn complete_crawl_reopens_results_seen_set_and_revalidation() {
        use spire_core::Context;
        use spire_core::crawler::Crawler;
        use spire_core::dataset::Data;
        use spire_core::http::http::{HeaderValue, StatusCode};
        use spire_core::testing::{MockPage, MockSession, MockSite};
        use spire_core::worker::middleware::Revalidate;
        use spire_core::worker::{WorkerExt, from_fn};

        async fn collect(cx: Context<MockSession>) -> Result<()> {
            let body = String::from_utf8(cx.body().await?.to_vec()).unwrap();
            cx.dataset::<String>()?.write(body).await
        }
        let directory = TempDir::new().unwrap();
        let path = directory.path().join("crawl.sqlite");
        let first = store(&path).await;
        let request = seed("page", 9).with_tag("pages");
        let site = MockSite::new().page(
            "https://example.com/page",
            MockPage::html("first result").with_header("etag", HeaderValue::from_static("\"v1\"")),
        );
        let frontier = first.frontier().await.unwrap();
        let crawler = Crawler::new(
            site,
            from_fn(collect).layer(Revalidate::new(first.validators())),
        )
        .with_frontier(frontier.clone().dedup_with(first.seen()).retry(1))
        .with_dataset(first.dataset::<String>("pages"))
        .with_seed(request.clone());
        let stats = crawler.run().await.unwrap();
        assert_eq!((stats.succeeded, stats.failed), (1, 0));
        assert_eq!(frontier.in_flight().await.unwrap(), 0);
        drop(crawler);
        drop(frontier);
        first.pool().close().await;

        let second = store(&path).await;
        assert!(
            !spire_core::frontier::seen::SeenSet::insert(&second.seen(), request.fingerprint())
                .await
                .unwrap()
        );
        assert_eq!(
            Data::new(second.dataset::<String>("pages"))
                .read_all()
                .await
                .unwrap(),
            ["first result"]
        );
        let site = MockSite::new().page(
            "https://example.com/page",
            MockPage::status(StatusCode::NOT_MODIFIED),
        );
        let frontier = second.frontier().await.unwrap();
        let crawler = Crawler::new(
            site.clone(),
            from_fn(collect).layer(Revalidate::new(second.validators())),
        )
        .with_frontier(frontier.clone())
        .with_dataset(second.dataset::<String>("pages"))
        .with_seed(request);
        let stats = crawler.run().await.unwrap();
        assert_eq!(stats.unchanged, 1);
        assert_eq!(frontier.in_flight().await.unwrap(), 0);
        assert_eq!(site.sent()[0].headers["if-none-match"], "\"v1\"");
        assert!(second.dataset::<String>("pages").is_empty().await.unwrap());
        drop(crawler);
        drop(frontier);
        second.pool().close().await;
    }

    #[tokio::test]
    async fn crash_child() {
        let Some(path) = std::env::var_os("SPIRE_SQLITE_CRASH_TEST_PATH") else {
            return;
        };
        let store = store(std::path::Path::new(&path)).await;
        if std::env::var_os("SPIRE_SQLITE_EXPECT_LOCKED").is_some() {
            std::process::exit(i32::from(store.frontier().await.is_ok()));
        }
        let frontier = store.frontier().await.unwrap();
        frontier.push(seed("abandoned", 0)).await.unwrap();
        let _ = pop(&frontier).await;
        std::process::exit(0); // Deliberately skip Rust destructors.
    }
}
