//! The crash-safe frontier and seen set, with batched and per-commit syncs.

use std::future::Future;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use spire_core::frontier::seed::Seed;
use spire_core::frontier::seen::SeenSet;
use spire_core::frontier::{Frontier, Next};
use spire_redb::RedbStore;

/// Seeds per round: per-commit syncs take milliseconds each.
const SEEDS: usize = 50;

/// A database file removed when dropped.
struct TempPath(PathBuf);

impl TempPath {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let name = format!(
            "spire-redb-bench-{}-{}.redb",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        );
        Self(std::env::temp_dir().join(name))
    }
}

impl Drop for TempPath {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

fn seeds() -> Vec<Seed> {
    (0..SEEDS)
        .map(|i| Seed::get(format!("https://example.test/{i}")).unwrap())
        .collect()
}

/// Opens a store on a fresh file, syncing every commit or once a second.
async fn store(sync_always: bool) -> (TempPath, RedbStore) {
    let path = TempPath::new();
    let builder = RedbStore::builder();
    let builder = if sync_always {
        builder.sync_always()
    } else {
        builder
    };
    let store = builder.open(&path.0).await.unwrap();
    (path, store)
}

/// Times `round` on a fresh store, `iters` times, leaving out opening it.
async fn timed<F, Fut>(iters: u64, sync_always: bool, round: F) -> Duration
where
    F: Fn(RedbStore) -> Fut,
    Fut: Future<Output = ()>,
{
    let mut total = Duration::ZERO;
    for _ in 0..iters {
        let (_path, store) = store(sync_always).await;
        let started = Instant::now();
        round(store).await;
        total += started.elapsed();
    }
    total
}

fn redb(c: &mut Criterion) {
    let runtime = runtime();
    let mut group = c.benchmark_group("redb");
    group.throughput(Throughput::Elements(SEEDS as u64));
    group.sample_size(10);

    for (name, sync_always) in [("batched", false), ("sync_always", true)] {
        group.bench_function(BenchmarkId::new("frontier", name), |b| {
            b.to_async(&runtime).iter_custom(|iters| {
                timed(iters, sync_always, |store| async move {
                    let frontier = store.frontier();
                    for seed in seeds() {
                        frontier.push(seed).await.unwrap();
                    }
                    while let Next::Ready(_) = frontier.pop().await.unwrap() {}
                })
            });
        });
        group.bench_function(BenchmarkId::new("seen", name), |b| {
            b.to_async(&runtime).iter_custom(|iters| {
                timed(iters, sync_always, |store| async move {
                    let seen = store.seen();
                    for seed in seeds() {
                        seen.insert(seed.fingerprint()).await.unwrap();
                    }
                })
            });
        });
    }
    group.finish();
}

/// A current-thread runtime: steadier timings than a thread pool.
fn runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
}

criterion_group!(benches, redb);
criterion_main!(benches);
