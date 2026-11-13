use std::fs::{File, OpenOptions};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use spire_core::frontier::seed::Seed;
use spire_core::frontier::{Ack, Admission, Frontier, NextSeed, PopMode, Report, Schedule};
use spire_core::{Error, ErrorKind, Result};
use sqlx::{Sqlite, SqlitePool};
use tokio::time::Instant;

use crate::error::frontier_error;

/// A persistent priority frontier with exclusive ownership and crash recovery.
///
/// Higher priorities run first; equal priorities use insertion order. Due
/// revisits and retries compete with fresh seeds by priority. Popped seeds
/// remain stored until reported, and abandoned seeds are recovered on the next
/// exclusive open. Cancellation requeues immediately. Seeds waiting for a retry
/// are stored with their attempt and due time, and survive a restart. The
/// frontier records the latest time it saw when it pops or holds a seed. After
/// a wall clock is moved back between runs, a retry waits no longer than it had
/// left at that record. A clock moved forward brings due times forward, as that
/// cannot be told from time passing.
/// Reports must carry the popped seed or its clone so they acknowledge the
/// exact occurrence, including when identical seeds are queued together.
///
/// Complete responses are committed before handlers can read them. Interrupted
/// requests replay their original responses, so link discovery does not depend
/// on the live page remaining unchanged. Completion, failure and retries remove
/// snapshots; cancellation preserves them. Replay restores response data, not
/// a browser's live document.
///
/// Enqueueing or scheduling serializes the seed and discards caller-attached
/// runtime extensions, even without a restart. Register shared runtime services
/// with `Crawler::with_extension` instead.
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
    owner: Arc<()>,
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

// The row ID follows one popped occurrence through clones and Retry. The
// owner marker prevents seeds from an older frontier session acknowledging
// records after recovery, without retaining the frontier's file lock.
#[derive(Debug, Clone)]
struct Occurrence {
    id: i64,
    owner: Arc<()>,
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
        let last_seen: Option<i64> =
            sqlx::query_scalar("SELECT last_seen FROM spire_frontier_clock")
                .fetch_optional(&mut *transaction)
                .await
                .map_err(frontier_error)?;
        transaction.commit().await.map_err(frontier_error)?;
        let wall = i64::try_from(
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis(),
        )
        .unwrap_or(i64::MAX);
        let epoch = wall.max(last_seen.unwrap_or_default());
        Ok(Self {
            inner: Arc::new(Inner {
                pool,
                _lock: file,
                epoch,
                started: Instant::now(),
                owner: Arc::new(()),
            }),
        })
    }

    /// Converts `due` to the stored clock.
    fn stored_due(&self, due: Instant) -> i64 {
        self.inner.epoch.saturating_add(
            i64::try_from(
                due.saturating_duration_since(self.inner.started)
                    .as_millis(),
            )
            .unwrap_or(i64::MAX),
        )
    }

    /// Converts a time on the stored clock to an [`Instant`].
    fn instant_at(&self, due: i64) -> Result<Instant> {
        let wait = u64::try_from(due.saturating_sub(self.now())).unwrap_or_default();
        Instant::now()
            .checked_add(Duration::from_millis(wait))
            .ok_or_else(|| invalid("a stored SQLite due time exceeds the clock range"))
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

/// Records `now` as the latest time the frontier saw.
async fn mark_seen(transaction: &mut sqlx::Transaction<'_, Sqlite>, now: i64) -> Result<()> {
    sqlx::query(
        "INSERT INTO spire_frontier_clock (id, last_seen) VALUES (1, $1)
        ON CONFLICT(id) DO UPDATE SET last_seen = MAX(last_seen, excluded.last_seen)",
    )
    .bind(now)
    .execute(&mut **transaction)
    .await
    .map_err(frontier_error)?;
    Ok(())
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

/// What happens to a reported seed once its in-flight row is removed.
enum Then {
    Forget,
    Queue,
    Hold(Instant),
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

    async fn pop_with(&self, mode: PopMode) -> Result<NextSeed> {
        let mut transaction = self
            .inner
            .pool
            .begin_with("BEGIN IMMEDIATE")
            .await
            .map_err(frontier_error)?;
        let now = self.now();
        mark_seen(&mut transaction, now).await?;
        sqlx::query(
            "INSERT INTO spire_frontier_items (fingerprint, priority, payload)
            SELECT fingerprint, priority, payload FROM (
                SELECT fingerprint, priority, payload, due, 0 AS source, rowid AS position
                    FROM spire_frontier_scheduled WHERE $2
                UNION ALL
                SELECT fingerprint, priority, payload, due, 1 AS source, id AS position
                    FROM spire_frontier_retries
            ) WHERE due <= $1 ORDER BY due, source, position",
        )
        .bind(now)
        .bind(mode == PopMode::All)
        .execute(&mut *transaction)
        .await
        .map_err(frontier_error)?;
        sqlx::query("DELETE FROM spire_frontier_scheduled WHERE due <= $1 AND $2")
            .bind(now)
            .bind(mode == PopMode::All)
            .execute(&mut *transaction)
            .await
            .map_err(frontier_error)?;
        sqlx::query("DELETE FROM spire_frontier_retries WHERE due <= $1")
            .bind(now)
            .execute(&mut *transaction)
            .await
            .map_err(frontier_error)?;
        let next: Option<(i64, String)> = sqlx::query_as(
            "SELECT id, payload FROM spire_frontier_items WHERE in_flight = 0 ORDER BY priority DESC, id LIMIT 1",
        ).fetch_optional(&mut *transaction).await.map_err(frontier_error)?;
        let Some((id, payload)) = next else {
            let earliest: Option<i64> =
                sqlx::query_scalar("SELECT MIN(due) FROM spire_frontier_retries")
                    .fetch_one(&mut *transaction)
                    .await
                    .map_err(frontier_error)?;
            transaction.commit().await.map_err(frontier_error)?;
            return Ok(match earliest {
                Some(due) => NextSeed::Wait(self.instant_at(due)?),
                None => NextSeed::Empty,
            });
        };
        // Corrupt seeds remain stored and return an error, rather than being
        // silently discarded or allowing an incomplete recovery.
        let mut seed = decode(&payload)?;
        seed.extensions_mut().insert(Occurrence {
            id,
            owner: self.inner.owner.clone(),
        });
        let replay =
            super::replay::load(&mut transaction, &self.inner.pool, id, &self.inner).await?;
        seed.extensions_mut().insert(replay);
        sqlx::query("UPDATE spire_frontier_items SET in_flight = 1 WHERE id = $1")
            .bind(id)
            .execute(&mut *transaction)
            .await
            .map_err(frontier_error)?;
        transaction.commit().await.map_err(frontier_error)?;
        Ok(NextSeed::Ready(seed))
    }

    async fn report(&self, report: Report) -> Result<Ack> {
        let (seed, then, ack) = match report {
            Report::Done(seed) | Report::Failed(seed) | Report::Skipped(seed) => {
                (seed, Then::Forget, Ack::Applied)
            }
            Report::Cancelled(seed) => (seed, Then::Queue, Ack::Applied),
            Report::Requeue { seed, due } => (seed, Then::Hold(due), Ack::Applied),
            Report::Retry { seed, .. } => (seed, Then::Forget, Ack::RetryRefused),
            _ => return Ok(Ack::Applied),
        };
        let occurrence = seed.extensions().get::<Occurrence>().ok_or_else(|| {
            invalid("SQLite reports require the seed returned by pop, or its clone")
        })?;
        if !Arc::ptr_eq(&occurrence.owner, &self.inner.owner) {
            return Err(invalid(
                "SQLite report belongs to a different frontier session",
            ));
        }
        let mut transaction = self
            .inner
            .pool
            .begin_with("BEGIN IMMEDIATE")
            .await
            .map_err(frontier_error)?;
        let deleted =
            sqlx::query("DELETE FROM spire_frontier_items WHERE id = $1 AND in_flight = 1")
                .bind(occurrence.id)
                .execute(&mut *transaction)
                .await
                .map_err(frontier_error)?;
        if deleted.rows_affected() == 1 {
            match then {
                Then::Forget => {}
                Then::Queue => {
                    enqueue(&mut transaction, &seed).await?;
                    sqlx::query("UPDATE spire_frontier_responses SET frontier_id = last_insert_rowid() WHERE frontier_id = $1")
                        .bind(occurrence.id).execute(&mut *transaction).await.map_err(frontier_error)?;
                }
                Then::Hold(due) => {
                    mark_seen(&mut transaction, self.now()).await?;
                    sqlx::query(
                        "INSERT INTO spire_frontier_retries (fingerprint, due, priority, payload)
                        VALUES ($1, $2, $3, $4)",
                    )
                    .bind(seed.fingerprint().get().to_be_bytes().to_vec())
                    .bind(self.stored_due(due))
                    .bind(seed.priority())
                    .bind(encode(&seed)?)
                    .execute(&mut *transaction)
                    .await
                    .map_err(frontier_error)?;
                }
            }
        }
        if deleted.rows_affected() == 1 {
            sqlx::query("DELETE FROM spire_frontier_responses WHERE frontier_id = $1")
                .bind(occurrence.id)
                .execute(&mut *transaction)
                .await
                .map_err(frontier_error)?;
        }
        transaction.commit().await.map_err(frontier_error)?;
        Ok(ack)
    }
}

impl Schedule for SqliteFrontier {
    async fn schedule(&self, seed: Seed, due: Instant) -> Result<()> {
        let due = self.stored_due(due);
        let mut transaction = self
            .inner
            .pool
            .begin_with("BEGIN IMMEDIATE")
            .await
            .map_err(frontier_error)?;
        mark_seen(&mut transaction, self.now()).await?;
        sqlx::query("INSERT INTO spire_frontier_scheduled (fingerprint, due, priority, payload) VALUES ($1, $2, $3, $4)
            ON CONFLICT(fingerprint) DO UPDATE SET due = excluded.due, priority = excluded.priority, payload = excluded.payload")
            .bind(seed.fingerprint().get().to_be_bytes().to_vec()).bind(due).bind(seed.priority()).bind(encode(&seed)?)
            .execute(&mut *transaction).await.map_err(frontier_error)?;
        transaction.commit().await.map_err(frontier_error)?;
        Ok(())
    }

    async fn next_due(&self) -> Result<Option<Instant>> {
        let due: Option<i64> = sqlx::query_scalar("SELECT MIN(due) FROM spire_frontier_scheduled")
            .fetch_one(&self.inner.pool)
            .await
            .map_err(frontier_error)?;
        due.map(|due| self.instant_at(due)).transpose()
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
    async fn queued_mode_releases_retries_but_preserves_due_revisits() {
        use spire_core::frontier::Schedule;
        use tokio::time::Instant;
        let directory = TempDir::new().unwrap();
        let store = store(&directory.path().join("queued.sqlite")).await;
        let frontier = store.frontier().await.unwrap();
        frontier.push(seed("retry", 0)).await.unwrap();
        let NextSeed::Ready(retry) = frontier.pop().await.unwrap() else {
            panic!("expected queued seed");
        };
        frontier
            .report(Report::Requeue {
                seed: retry,
                due: Instant::now(),
            })
            .await
            .unwrap();
        frontier
            .schedule(seed("revisit", 0), Instant::now())
            .await
            .unwrap();
        let NextSeed::Ready(retry) = frontier.pop_with(PopMode::Queued).await.unwrap() else {
            panic!("expected retry");
        };
        assert_eq!(retry.uri().path(), "/retry");
        frontier.report(Report::Done(retry)).await.unwrap();
        assert!(matches!(
            frontier.pop_with(PopMode::Queued).await.unwrap(),
            NextSeed::Empty
        ));
        let NextSeed::Ready(revisit) = frontier.pop().await.unwrap() else {
            panic!("expected revisit");
        };
        assert_eq!(revisit.uri().path(), "/revisit");
    }

    #[tokio::test]
    async fn replay_recovers_original_links_after_the_live_page_changes() {
        use spire_core::Context;
        use spire_core::backend::{Backend, Noop};
        use spire_core::frontier::queue::Queue;
        use spire_core::http::ResponseExt;
        use spire_core::testing::{MockPage, MockSite};

        let directory = TempDir::new().unwrap();
        let path = directory.path().join("replay.sqlite");
        let store = store(&path).await;
        let frontier = store.frontier().await.unwrap();
        frontier.push(seed("index", 0)).await.unwrap();
        let popped = pop(&frontier).await;
        let site = MockSite::new().page("https://example.com/index", MockPage::text("/a /b"));
        let cx = Context::new(popped.into_request(), site.connect().await.unwrap());
        assert_eq!(cx.body().await.unwrap().as_ref(), b"/a /b");
        // The process dies before the handler queues any of the links.
        drop(cx);
        drop(frontier);
        store.close().await;
        let store = SqliteStore::open(&path).await.unwrap();
        let frontier = store.frontier().await.unwrap();
        let popped = pop(&frontier).await;
        // Noop returns an empty response: recovery must use the stored one.
        let queue = Queue::new(frontier.clone());
        let cx = Context::new(popped.clone().into_request(), Noop).with_queue(&queue);
        let response = cx.response().await.unwrap();
        assert_eq!(response.body().as_ref(), b"/a /b");
        assert_eq!(response.final_uri().unwrap(), "https://example.com/index");
        for link in std::str::from_utf8(response.body())
            .unwrap()
            .split_whitespace()
        {
            cx.queue().follow(link).await.unwrap();
        }
        frontier.report(Report::Done(popped)).await.unwrap();
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM spire_frontier_responses")
            .fetch_one(store.pool())
            .await
            .unwrap();
        assert_eq!(count, 0);
        assert_eq!(pop(&frontier).await.uri().path(), "/a");
        assert_eq!(pop(&frontier).await.uri().path(), "/b");
        drop(cx);
        drop(queue);
        drop(frontier);
        store.close().await;
    }

    #[tokio::test]
    async fn cancelled_occurrences_keep_their_snapshot_without_accepting_stale_reports() {
        use spire_core::Context;
        use spire_core::backend::{Backend, Noop};
        use spire_core::testing::{MockPage, MockSite};

        let directory = TempDir::new().unwrap();
        let store = store(&directory.path().join("replay.sqlite")).await;
        let frontier = store.frontier().await.unwrap();
        frontier.push(seed("index", 0)).await.unwrap();
        let popped = pop(&frontier).await;
        let site = MockSite::new().page("https://example.com/index", MockPage::text("saved"));
        let cx = Context::new(popped.clone().into_request(), site.connect().await.unwrap());
        cx.response().await.unwrap();
        frontier
            .report(Report::Cancelled(popped.clone()))
            .await
            .unwrap();
        let next = pop(&frontier).await;
        frontier.report(Report::Done(popped)).await.unwrap();
        let recovered = Context::new(next.clone().into_request(), Noop);
        assert_eq!(recovered.body().await.unwrap().as_ref(), b"saved");
        frontier.report(Report::Done(next)).await.unwrap();
        drop(recovered);
        drop(cx);
        drop(frontier);
        store.close().await;
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
    async fn repeated_and_stale_reports_cannot_acknowledge_other_occurrences() {
        let directory = TempDir::new().unwrap();
        let store = store(&directory.path().join("test.sqlite")).await;
        let frontier = store.frontier().await.unwrap();
        frontier.push(seed("same", 0)).await.unwrap();
        frontier.push(seed("same", 0)).await.unwrap();
        let first = pop(&frontier).await;
        let second = pop(&frontier).await;
        assert!(
            frontier
                .report(Report::Done(seed("same", 0)))
                .await
                .is_err()
        );
        frontier
            .report(Report::Cancelled(first.clone()))
            .await
            .unwrap();
        let requeued = pop(&frontier).await;
        frontier
            .report(Report::Cancelled(first.clone()))
            .await
            .unwrap();
        frontier.report(Report::Done(first)).await.unwrap();
        assert_eq!(frontier.in_flight().await.unwrap(), 2);
        assert!(matches!(frontier.pop().await.unwrap(), NextSeed::Empty));
        drop(frontier);
        let reopened = store.frontier().await.unwrap();
        let recovered = pop(&reopened).await;
        assert!(reopened.report(Report::Done(second)).await.is_err());
        assert!(reopened.report(Report::Done(requeued)).await.is_err());
        assert_eq!(reopened.in_flight().await.unwrap(), 1);
        reopened.report(Report::Done(recovered)).await.unwrap();
        let remaining = pop(&reopened).await;
        reopened.report(Report::Done(remaining)).await.unwrap();
        assert!(matches!(reopened.pop().await.unwrap(), NextSeed::Empty));
        drop(reopened);
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
    async fn completing_a_duplicate_does_not_acknowledge_a_pending_retry_occurrence() {
        let directory = TempDir::new().unwrap();
        let path = directory.path().join("retry.sqlite");
        let first = store(&path).await;
        let inner = first.frontier().await.unwrap();
        let frontier = inner.clone().retry(3);
        frontier.push(seed("same", 0)).await.unwrap();
        let NextSeed::Ready(original) = frontier.pop().await.unwrap() else {
            panic!()
        };
        frontier
            .report(Report::Retry {
                seed: original,
                after: Some(Duration::ZERO),
            })
            .await
            .unwrap();
        let NextSeed::Ready(retried) = frontier.pop().await.unwrap() else {
            panic!()
        };
        assert_eq!(retried.attempt().get(), 1);
        // Cancellation stores this occurrence at attempt one before a fresh
        // identical request is admitted at attempt zero.
        frontier.report(Report::Cancelled(retried)).await.unwrap();
        frontier.push(seed("same", 0)).await.unwrap();
        let NextSeed::Ready(pending) = frontier.pop().await.unwrap() else {
            panic!()
        };
        let NextSeed::Ready(completed) = frontier.pop().await.unwrap() else {
            panic!()
        };
        assert_eq!((pending.attempt().get(), completed.attempt().get()), (1, 0));
        frontier
            .report(Report::Retry {
                seed: pending,
                after: Some(Duration::from_hours(1)),
            })
            .await
            .unwrap();
        frontier.report(Report::Done(completed)).await.unwrap();
        drop(frontier);
        drop(inner);
        first.pool().close().await;
        let reopened = store(&path).await;
        let recovered = reopened.frontier().await.unwrap();
        // The retry is still waiting, and nothing else was left behind.
        assert_eq!(recovered.in_flight().await.unwrap(), 0);
        let NextSeed::Wait(at) = recovered.pop().await.unwrap() else {
            panic!("expected a wait for the pending retry")
        };
        assert!(at - Instant::now() > Duration::from_mins(59));
        drop(recovered);
        reopened.pool().close().await;
    }

    #[tokio::test]
    async fn due_retries_compete_by_priority_and_survive_reopening() {
        let directory = TempDir::new().unwrap();
        let path = directory.path().join("order.sqlite");
        let first = store(&path).await;
        let inner = first.frontier().await.unwrap();
        let frontier = inner.clone().retry(3);
        frontier.push(seed("retried", 5)).await.unwrap();
        let NextSeed::Ready(retried) = frontier.pop().await.unwrap() else {
            panic!()
        };
        frontier
            .report(Report::Retry {
                seed: retried,
                after: Some(Duration::ZERO),
            })
            .await
            .unwrap();
        frontier.push(seed("low", 1)).await.unwrap();
        frontier.push(seed("high", 9)).await.unwrap();
        drop(frontier);
        drop(inner);
        first.pool().close().await;

        let reopened = store(&path).await;
        let frontier = reopened.frontier().await.unwrap();
        let order = [
            pop(&frontier).await,
            pop(&frontier).await,
            pop(&frontier).await,
        ];
        let order = order.map(|seed| (seed.uri().path().to_owned(), seed.attempt().get()));
        assert_eq!(
            order,
            [
                ("/high".to_owned(), 0),
                ("/retried".to_owned(), 1),
                ("/low".to_owned(), 0),
            ]
        );
        drop(frontier);
        reopened.pool().close().await;
    }

    #[tokio::test]
    async fn seeds_due_together_join_the_queue_in_a_fixed_order() {
        let directory = TempDir::new().unwrap();
        // Reverses whatever order SQLite leaves unspecified, so the test
        // depends on the query spelling out the order.
        let pool = SqlitePoolOptions::new()
            .connect_with(
                SqliteConnectOptions::new()
                    .filename(directory.path().join("ties.sqlite"))
                    .create_if_missing(true)
                    .pragma("reverse_unordered_selects", "ON"),
            )
            .await
            .unwrap();
        let store = SqliteStore::new(pool);
        store.initialize().await.unwrap();
        let frontier = store.frontier().await.unwrap();
        let names = ["e", "b", "d", "a", "c"];
        let mut popped = Vec::new();
        for name in names {
            frontier.push(seed(name, 0)).await.unwrap();
            popped.push(pop(&frontier).await);
        }
        let due = Instant::now();
        for seed in popped {
            let requeue = Report::Requeue { seed, due };
            assert_eq!(frontier.report(requeue).await.unwrap(), Ack::Applied);
        }
        frontier.schedule(seed("revisit", 0), due).await.unwrap();

        let mut order = Vec::new();
        for _ in 0..=names.len() {
            order.push(pop(&frontier).await.uri().path().to_owned());
        }
        assert_eq!(order, ["/revisit", "/e", "/b", "/d", "/a", "/c"]);
        drop(frontier);
        store.pool().close().await;
    }

    #[tokio::test]
    async fn a_clock_moved_back_between_runs_does_not_delay_a_retry() {
        let directory = TempDir::new().unwrap();
        let store = store(&directory.path().join("clock.sqlite")).await;
        let wall = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
        // As left by a run whose clock was ten hours ahead of this one's, with
        // a retry due a minute after it stopped.
        let last_seen = i64::try_from((wall + Duration::from_hours(10)).as_millis()).unwrap();
        sqlx::query("INSERT INTO spire_frontier_clock (id, last_seen) VALUES (1, $1)")
            .bind(last_seen)
            .execute(store.pool())
            .await
            .unwrap();
        let retried = seed("retried", 0);
        sqlx::query(
            "INSERT INTO spire_frontier_retries (fingerprint, due, priority, payload)
            VALUES ($1, $2, 0, $3)",
        )
        .bind(retried.fingerprint().get().to_be_bytes().to_vec())
        .bind(last_seen + 60_000)
        .bind(encode(&retried).unwrap())
        .execute(store.pool())
        .await
        .unwrap();

        let frontier = store.frontier().await.unwrap();
        let NextSeed::Wait(at) = frontier.pop().await.unwrap() else {
            panic!("expected a wait for the retry");
        };
        let wait = at - Instant::now();
        assert!(wait <= Duration::from_mins(1), "{wait:?}");
        drop(frontier);
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
        use spire_core::Context;
        use spire_core::backend::Noop;
        use spire_core::dataset::Dataset;

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
        let recovered = pop(&frontier).await;
        assert_eq!(recovered.uri().path(), "/abandoned");
        let cx = Context::new(recovered.into_request(), Noop);
        assert_eq!(cx.body().await.unwrap().as_ref(), b"/not-yet-queued");
        // The child exited with an open read transaction, without Drop.
        assert_eq!(
            store.dataset::<u32>("delivery").read().await.unwrap(),
            Some(42)
        );
        drop(cx);
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
        sqlx::query("UPDATE spire_frontier_scheduled SET due = 0")
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
            from_fn(collect).layer(Revalidate::new(first.validator())),
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
            from_fn(collect).layer(Revalidate::new(second.validator())),
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
        use spire_core::Context;
        use spire_core::backend::Backend;
        use spire_core::dataset::Dataset;
        use spire_core::testing::{MockPage, MockSite};

        let Some(path) = std::env::var_os("SPIRE_SQLITE_CRASH_TEST_PATH") else {
            return;
        };
        let store = store(std::path::Path::new(&path)).await;
        if std::env::var_os("SPIRE_SQLITE_EXPECT_LOCKED").is_some() {
            std::process::exit(i32::from(store.frontier().await.is_ok()));
        }
        let frontier = store.frontier().await.unwrap();
        frontier.push(seed("abandoned", 0)).await.unwrap();
        let popped = pop(&frontier).await;
        let site = MockSite::new().page(
            "https://example.com/abandoned",
            MockPage::text("/not-yet-queued"),
        );
        let cx = Context::new(popped.into_request(), site.connect().await.unwrap());
        cx.response().await.unwrap();
        let data = store.dataset::<u32>("delivery");
        data.write(42).await.unwrap();
        let batch = data.read_with_ack(1).await.unwrap();
        assert_eq!(batch.items(), [42]);
        std::process::exit(0); // Deliberately skip Rust destructors.
    }
}
