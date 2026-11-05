use std::fmt;
use std::future::{Future, ready};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use redb::ReadableTable;
use spire_core::Result;
use spire_core::frontier::seed::Seed;
use spire_core::frontier::{Ack, Admission, Frontier, NextSeed, Report, Schedule};
use tokio::time::Instant;

use crate::codec;
use crate::store::{
    IN_FLIGHT, META, NEXT_SEQUENCE, QUEUE, RETRIES, RedbStore, SCHEDULE, SCHEDULED,
};

/// A [`Frontier`] stored in a [`RedbStore`], crawling by priority, then in
/// insertion order.
///
/// A popped seed stays recorded as in flight until it is reported done,
/// failed or skipped, so seeds in flight at a crash are queued again when
/// the store is reopened. Seeds reported cancelled are queued again at once.
///
/// Retries are left to facades: wrap it with
/// [`FrontierExt::retry`](spire_core::frontier::FrontierExt::retry). A seed
/// waiting for a retry is kept in the store with its attempt and when it is
/// due, so it survives a crash too. Due retries and revisits join the queue,
/// and compete by priority; equal priorities run in the order they joined.
pub struct RedbFrontier {
    store: RedbStore,
}

impl RedbFrontier {
    pub(crate) const fn new(store: RedbStore) -> Self {
        Self { store }
    }

    /// Returns the number of seeds popped and not yet reported.
    ///
    /// # Errors
    ///
    /// Returns a [`Frontier`](spire_core::ErrorKind::Frontier) error if the
    /// store cannot be read.
    pub fn in_flight(&self) -> Result<usize> {
        self.store.count(IN_FLIGHT)
    }
}

impl fmt::Debug for RedbFrontier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RedbFrontier")
            .field("store", &self.store)
            .finish()
    }
}

/// Takes the next sequence number in a write transaction.
fn next_sequence(txn: &redb::WriteTransaction) -> Result<u64, redb::Error> {
    let mut meta = txn.open_table(META)?;
    let sequence = meta.get(NEXT_SEQUENCE)?.map_or(0, |guard| guard.value());
    meta.insert(NEXT_SEQUENCE, sequence + 1)?;
    Ok(sequence)
}

/// Queues an encoded seed in a write transaction.
fn enqueue(txn: &redb::WriteTransaction, priority: i32, bytes: &[u8]) -> Result<(), redb::Error> {
    let sequence = next_sequence(txn)?;
    txn.open_table(QUEUE)?
        .insert((-i64::from(priority), sequence), bytes)?;
    Ok(())
}

/// Moves the revisits and retries due by `now` to the queue, in the order they
/// came due. Seeds that cannot be decoded are dropped, as when popped.
fn release_due(txn: &redb::WriteTransaction, now: u64) -> Result<(), redb::Error> {
    let mut due = Vec::new();
    {
        let mut schedule = txn.open_table(SCHEDULE)?;
        let mut scheduled = txn.open_table(SCHEDULED)?;
        loop {
            let first = schedule.first()?.map(|(key, _)| key.value());
            let Some(key) = first.filter(|&(at, _)| at <= now) else {
                break;
            };
            schedule.remove(key)?;
            if let Some(entry) = scheduled.remove(key.1)?
                && let Some(bytes) = entry.value().get(8..)
            {
                due.push((key.0, bytes.to_vec()));
            }
        }

        let mut retries = txn.open_table(RETRIES)?;
        loop {
            let first = retries.first()?.map(|(key, _)| key.value());
            let Some(key) = first.filter(|&(at, _)| at <= now) else {
                break;
            };
            if let Some(bytes) = retries.remove(key)? {
                due.push((key.0, bytes.value().to_vec()));
            }
        }
    }

    due.sort_by_key(|(at, _)| *at);
    for (_, bytes) in due {
        if let Some(priority) = codec::priority(&bytes) {
            enqueue(txn, priority, &bytes)?;
        }
    }
    Ok(())
}

/// Converts milliseconds since the Unix epoch to an [`Instant`].
fn instant_at(due: u64) -> Instant {
    let wait = due.saturating_sub(unix_millis(SystemTime::now()));
    Instant::now() + Duration::from_millis(wait)
}

/// Converts an [`Instant`] to milliseconds since the Unix epoch.
fn millis_at(due: Instant) -> u64 {
    unix_millis(SystemTime::now() + due.saturating_duration_since(Instant::now()))
}

impl Frontier for RedbFrontier {
    async fn push(&self, seed: Seed) -> Result<Admission> {
        let bytes = codec::encode(&seed)?;
        let priority = seed.priority();
        self.store
            .write(move |txn| Ok(enqueue(txn, priority, &bytes)?))
            .await?;
        Ok(Admission::Queued)
    }

    /// Moves seeds that have come due to the queue, then hands out the next.
    /// It waits for retries, but not for scheduled seeds; see
    /// [`Schedule::next_due`].
    async fn pop(&self) -> Result<NextSeed> {
        let now = unix_millis(SystemTime::now());
        let popped = self
            .store
            .write(move |txn| {
                release_due(txn, now)?;
                let mut queue = txn.open_table(QUEUE)?;
                let mut in_flight = txn.open_table(IN_FLIGHT)?;
                while let Some((_, bytes)) = queue.pop_first()? {
                    let bytes = bytes.value().to_vec();
                    if let Some(seed) = codec::decode(&bytes) {
                        in_flight.insert(seed.fingerprint().get(), bytes.as_slice())?;
                        return Ok(Some(seed));
                    }
                }
                Ok(None)
            })
            .await?;
        if let Some(seed) = popped {
            return Ok(NextSeed::Ready(seed));
        }

        let earliest = self.store.read(|txn| {
            let retries = txn.open_table(RETRIES)?;
            Ok(retries.first()?.map(|(key, _)| key.value().0))
        })?;
        Ok(earliest.map_or(NextSeed::Empty, |due| NextSeed::Wait(instant_at(due))))
    }

    async fn report(&self, report: Report) -> Result<Ack> {
        if let Report::Requeue { seed, due } = report {
            let fingerprint = seed.fingerprint().get();
            let bytes = codec::encode(&seed)?;
            let due = millis_at(due);
            self.store
                .write(move |txn| {
                    txn.open_table(IN_FLIGHT)?.remove(fingerprint)?;
                    let sequence = next_sequence(txn)?;
                    txn.open_table(RETRIES)?
                        .insert((due, sequence), bytes.as_slice())?;
                    Ok(())
                })
                .await?;
            return Ok(Ack::Applied);
        }

        let (seed, requeue, ack) = match report {
            Report::Done(seed) | Report::Failed(seed) | Report::Skipped(seed) => {
                (seed, false, Ack::Applied)
            }
            Report::Cancelled(seed) => (seed, true, Ack::Applied),
            // Retries are left to a `Retry` facade; without one the seed fails.
            Report::Retry { seed, .. } => (seed, false, Ack::RetryRefused),
            _ => return Ok(Ack::Applied),
        };

        let fingerprint = seed.fingerprint().get();
        let requeued = if requeue {
            Some((seed.priority(), codec::encode(&seed)?))
        } else {
            None
        };
        self.store
            .write(move |txn| {
                txn.open_table(IN_FLIGHT)?.remove(fingerprint)?;
                if let Some((priority, bytes)) = requeued {
                    enqueue(txn, priority, &bytes)?;
                }
                Ok(())
            })
            .await?;
        Ok(ack)
    }

    fn size_hint(&self) -> Option<usize> {
        let queued = self.store.count(QUEUE).ok()?;
        Some(queued + self.store.count(RETRIES).ok()?)
    }
}

/// Keeps scheduled seeds in the store, so revisits survive a restart.
impl Schedule for RedbFrontier {
    async fn schedule(&self, seed: Seed, due: Instant) -> Result<()> {
        let due = millis_at(due);
        let fingerprint = seed.fingerprint().get();
        let mut entry = due.to_le_bytes().to_vec();
        entry.extend(codec::encode(&seed)?);
        self.store
            .write(move |txn| {
                let mut scheduled = txn.open_table(SCHEDULED)?;
                let mut schedule = txn.open_table(SCHEDULE)?;
                // One revisit per page: the new one replaces any other.
                let earlier = scheduled
                    .insert(fingerprint, entry.as_slice())?
                    .and_then(|old| scheduled_due(old.value()));
                if let Some(earlier) = earlier {
                    schedule.remove((earlier, fingerprint))?;
                }
                schedule.insert((due, fingerprint), ())?;
                Ok(())
            })
            .await
    }

    fn next_due(&self) -> impl Future<Output = Result<Option<Instant>>> + Send {
        let first = self.store.read(|txn| match txn.open_table(SCHEDULE) {
            Ok(schedule) => Ok(schedule.first()?.map(|(key, _)| key.value().0)),
            Err(redb::TableError::TableDoesNotExist(_)) => Ok(None),
            Err(error) => Err(error.into()),
        });
        ready(first.map(|due| due.map(instant_at)))
    }
}

/// When a scheduled entry is due: its first 8 bytes.
fn scheduled_due(entry: &[u8]) -> Option<u64> {
    entry.first_chunk::<8>().map(|due| u64::from_le_bytes(*due))
}

/// Milliseconds since the Unix epoch, zero before it.
fn unix_millis(time: SystemTime) -> u64 {
    let since = time.duration_since(UNIX_EPOCH).unwrap_or_default();
    u64::try_from(since.as_millis()).unwrap_or(u64::MAX)
}

#[cfg(test)]
pub(crate) mod tests {
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use super::*;

    /// A database path unique to the test, removed when dropped.
    pub(crate) struct TempPath(pub(crate) PathBuf);

    impl TempPath {
        pub(crate) fn new() -> Self {
            static COUNTER: AtomicUsize = AtomicUsize::new(0);
            let n = COUNTER.fetch_add(1, Ordering::SeqCst);
            let name = format!("spire-redb-{}-{n}.redb", std::process::id());
            Self(std::env::temp_dir().join(name))
        }
    }

    impl Drop for TempPath {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.0);
        }
    }

    pub(crate) fn seed(path: &str) -> Seed {
        Seed::get(format!("https://example.com/{path}")).unwrap()
    }

    async fn pop_path(frontier: &RedbFrontier) -> Option<String> {
        match frontier.pop().await.unwrap() {
            NextSeed::Ready(seed) => Some(seed.uri().path().trim_start_matches('/').to_owned()),
            NextSeed::Wait(_) | NextSeed::Empty => None,
        }
    }

    #[tokio::test]
    async fn crawls_by_priority_then_insertion() {
        let path = TempPath::new();
        let frontier = RedbStore::open(&path.0).await.unwrap().frontier();
        for (name, priority) in [("first", 0), ("low", -1), ("high", 3), ("second", 0)] {
            frontier
                .push(seed(name).with_priority(priority))
                .await
                .unwrap();
        }
        assert_eq!(frontier.size_hint(), Some(4));

        let mut order = Vec::new();
        while let Some(name) = pop_path(&frontier).await {
            order.push(name);
        }
        assert_eq!(order, ["high", "first", "second", "low"]);
        assert_eq!(frontier.in_flight().unwrap(), 4);
    }

    #[tokio::test]
    async fn survives_reopening() {
        let path = TempPath::new();
        {
            let frontier = RedbStore::open(&path.0).await.unwrap().frontier();
            for name in ["done", "in-flight", "queued", "cancelled"] {
                frontier.push(seed(name).with_tag(name)).await.unwrap();
            }
            let NextSeed::Ready(done) = frontier.pop().await.unwrap() else {
                panic!()
            };
            frontier.report(Report::Done(done)).await.unwrap();
            // `in-flight` and `queued` are popped and never reported, as if
            // the crawl crashed mid-request.
            assert_eq!(pop_path(&frontier).await.as_deref(), Some("in-flight"));
            assert_eq!(pop_path(&frontier).await.as_deref(), Some("queued"));
            let NextSeed::Ready(cancelled) = frontier.pop().await.unwrap() else {
                panic!()
            };
            frontier.report(Report::Cancelled(cancelled)).await.unwrap();
        }

        let frontier = RedbStore::open(&path.0).await.unwrap().frontier();
        assert_eq!(frontier.in_flight().unwrap(), 0);
        let mut left = Vec::new();
        while let NextSeed::Ready(seed) = frontier.pop().await.unwrap() {
            left.push(seed.tag().map(ToString::to_string).unwrap());
        }
        left.sort();
        assert_eq!(left, ["cancelled", "in-flight", "queued"]);
    }

    #[tokio::test]
    async fn refuses_retries_without_a_facade() {
        let path = TempPath::new();
        let frontier = RedbStore::open(&path.0).await.unwrap().frontier();
        frontier.push(seed("a")).await.unwrap();
        let NextSeed::Ready(popped) = frontier.pop().await.unwrap() else {
            panic!()
        };

        let retry = Report::Retry {
            seed: popped,
            after: None,
        };
        assert_eq!(frontier.report(retry).await.unwrap(), Ack::RetryRefused);
        assert_eq!(frontier.in_flight().unwrap(), 0);
    }

    #[tokio::test]
    async fn scheduled_seeds_come_back_when_due_and_survive_reopening() {
        use std::time::Duration;

        use spire_core::frontier::Schedule;
        use tokio::time::Instant;

        let path = TempPath::new();
        {
            let frontier = RedbStore::open(&path.0).await.unwrap().frontier();
            let later = Instant::now() + Duration::from_hours(1);
            frontier.schedule(seed("later"), later).await.unwrap();
            frontier
                .schedule(seed("due"), Instant::now())
                .await
                .unwrap();
            frontier.push(seed("queued")).await.unwrap();
            // A page has one revisit: scheduling it again replaces it.
            let later = Instant::now() + Duration::from_hours(2);
            frontier.schedule(seed("later"), later).await.unwrap();
        }

        let frontier = RedbStore::open(&path.0).await.unwrap().frontier();
        // The due revisit joins the queue behind what is queued; the other
        // is not waited for, but reported as due in two hours.
        let mut popped = Vec::new();
        while let NextSeed::Ready(seed) = frontier.pop().await.unwrap() {
            popped.push(seed.uri().path().to_owned());
        }
        assert_eq!(popped, ["/queued", "/due"]);
        let next = frontier.next_due().await.unwrap().unwrap() - Instant::now();
        let range = Duration::from_mins(119)..=Duration::from_hours(2);
        assert!(range.contains(&next), "{next:?}");
    }

    #[tokio::test]
    async fn due_retries_compete_by_priority_and_keep_past_admission() {
        use std::time::Duration;

        use spire_core::frontier::FrontierExt;

        let path = TempPath::new();
        let store = RedbStore::open(&path.0).await.unwrap();
        let frontier = store.frontier().dedup_with(store.seen()).retry(3);
        for (name, priority) in [("low", 1), ("retried", 5), ("high", 9)] {
            let seed = seed(name).with_priority(priority);
            assert!(frontier.push(seed).await.unwrap().is_queued());
        }
        let NextSeed::Ready(high) = frontier.pop().await.unwrap() else {
            panic!()
        };
        let NextSeed::Ready(retried) = frontier.pop().await.unwrap() else {
            panic!()
        };
        frontier.report(Report::Done(high)).await.unwrap();
        let retry = Report::Retry {
            seed: retried,
            after: Some(Duration::ZERO),
        };
        assert_eq!(frontier.report(retry).await.unwrap(), Ack::Applied);

        let NextSeed::Ready(next) = frontier.pop().await.unwrap() else {
            panic!()
        };
        assert_eq!(next.uri().path(), "/retried");
        assert_eq!(next.attempt().get(), 1);
    }

    #[tokio::test]
    async fn a_waiting_retry_survives_reopening_with_its_attempt_and_due_time() {
        use std::time::Duration;

        use spire_core::frontier::FrontierExt;
        use tokio::time::Instant;

        let path = TempPath::new();
        {
            let frontier = RedbStore::open(&path.0).await.unwrap().frontier().retry(3);
            frontier.push(seed("later")).await.unwrap();
            frontier.push(seed("due")).await.unwrap();
            for after in [Duration::from_hours(1), Duration::ZERO] {
                let NextSeed::Ready(seed) = frontier.pop().await.unwrap() else {
                    panic!()
                };
                let after = Some(after);
                frontier
                    .report(Report::Retry { seed, after })
                    .await
                    .unwrap();
            }
            assert_eq!(frontier.size_hint(), Some(2));
        }

        let frontier = RedbStore::open(&path.0).await.unwrap().frontier();
        assert_eq!(frontier.in_flight().unwrap(), 0);
        let NextSeed::Ready(due) = frontier.pop().await.unwrap() else {
            panic!()
        };
        assert_eq!(due.uri().path(), "/due");
        assert_eq!(due.attempt().get(), 1);
        let NextSeed::Wait(at) = frontier.pop().await.unwrap() else {
            panic!("expected a wait for the other retry")
        };
        let wait = at - Instant::now();
        assert!(wait > Duration::from_mins(59), "{wait:?}");
    }
}
