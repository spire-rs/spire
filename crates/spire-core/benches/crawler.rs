//! The crawler's own overhead: requests through `Crawler::run` with a backend
//! and worker that do nothing, so only scheduling, per-host bookkeeping and
//! frontier traffic are measured.

use criterion::{BatchSize, BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use spire_core::backend::Noop;
use spire_core::frontier::seed::Seed;
use spire_core::worker::from_fn;
use spire_core::{Context, Crawler};

/// Requests per crawl.
const REQUESTS: usize = 1_000;

/// Seeds spread over 16 hosts, as a crawl of several sites would be.
fn seeds() -> Vec<Seed> {
    (0..REQUESTS)
        .map(|i| Seed::get(format!("https://host{}.test/{i}", i % 16)).unwrap())
        .collect()
}

fn crawler(c: &mut Criterion) {
    let runtime = runtime();
    let mut group = c.benchmark_group("crawler");
    group.throughput(Throughput::Elements(REQUESTS as u64));

    for concurrency in [1, 8, 64] {
        group.bench_with_input(
            BenchmarkId::new("noop", concurrency),
            &concurrency,
            |b, &concurrency| {
                b.to_async(&runtime).iter_batched(
                    || {
                        let worker = from_fn(|_: Context<Noop>| async {});
                        Crawler::new(Noop, worker)
                            .with_concurrency(concurrency)
                            .with_seeds(seeds())
                    },
                    |crawler| async move { crawler.run().await.unwrap() },
                    BatchSize::SmallInput,
                );
            },
        );
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

criterion_group!(benches, crawler);
criterion_main!(benches);
