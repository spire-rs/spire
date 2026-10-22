use std::fmt;

use redb::ReadableTable;
use spire_core::Result;
use spire_core::frontier::seed::Seed;
use spire_core::frontier::{Ack, Admission, Frontier, Pop, Report};

use crate::codec;
use crate::store::{IN_FLIGHT, META, NEXT_SEQUENCE, QUEUE, RedbStore};

/// A [`Frontier`] stored in a [`RedbStore`], crawling by priority, then in
/// insertion order.
///
/// A popped seed stays recorded as in flight until it is reported done,
/// failed or skipped, so seeds in flight at a crash are queued again when
/// the store is reopened. Seeds reported cancelled are queued again at once.
///
/// Retries are left to facades: wrap it with
/// [`FrontierExt::retry`](spire_core::frontier::FrontierExt::retry). A seed
/// waiting for a retry stays recorded as in flight, so it survives a crash
/// too, though its retry count starts over.
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

/// Queues an encoded seed in a write transaction.
fn enqueue(txn: &redb::WriteTransaction, priority: i32, bytes: &[u8]) -> Result<(), redb::Error> {
    let mut meta = txn.open_table(META)?;
    let sequence = meta.get(NEXT_SEQUENCE)?.map_or(0, |guard| guard.value());
    meta.insert(NEXT_SEQUENCE, sequence + 1)?;
    txn.open_table(QUEUE)?
        .insert((-i64::from(priority), sequence), bytes)?;
    Ok(())
}

impl Frontier for RedbFrontier {
    async fn push(&self, seed: Seed) -> Result<Admission> {
        let bytes = codec::encode(&seed)?;
        let priority = seed.priority();
        self.store
            .write(move |txn| enqueue(txn, priority, &bytes))
            .await?;
        Ok(Admission::Queued)
    }

    async fn pop(&self) -> Result<Pop> {
        let popped = self
            .store
            .write(|txn| {
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

        Ok(popped.map_or(Pop::Empty, Pop::Ready))
    }

    async fn report(&self, report: Report) -> Result<Ack> {
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
        self.store.count(QUEUE).ok()
    }
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
            Pop::Ready(seed) => Some(seed.uri().path().trim_start_matches('/').to_owned()),
            Pop::Wait(_) | Pop::Empty => None,
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
            let Pop::Ready(done) = frontier.pop().await.unwrap() else {
                panic!()
            };
            frontier.report(Report::Done(done)).await.unwrap();
            // `in-flight` and `queued` are popped and never reported, as if
            // the crawl crashed mid-request.
            assert_eq!(pop_path(&frontier).await.as_deref(), Some("in-flight"));
            assert_eq!(pop_path(&frontier).await.as_deref(), Some("queued"));
            let Pop::Ready(cancelled) = frontier.pop().await.unwrap() else {
                panic!()
            };
            frontier.report(Report::Cancelled(cancelled)).await.unwrap();
        }

        let frontier = RedbStore::open(&path.0).await.unwrap().frontier();
        assert_eq!(frontier.in_flight().unwrap(), 0);
        let mut left = Vec::new();
        while let Pop::Ready(seed) = frontier.pop().await.unwrap() {
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
        let Pop::Ready(popped) = frontier.pop().await.unwrap() else {
            panic!()
        };

        let retry = Report::Retry {
            seed: popped,
            after: None,
        };
        assert_eq!(frontier.report(retry).await.unwrap(), Ack::RetryRefused);
        assert_eq!(frontier.in_flight().unwrap(), 0);
    }
}
