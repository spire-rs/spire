//! Frontiers, seeds and seen sets: what every request goes through.

use criterion::{BatchSize, Criterion, Throughput, criterion_group, criterion_main};
use spire_core::frontier::seed::{Fingerprint, Seed};
use spire_core::frontier::seen::{BloomSeen, InMemSeen, SeenSet};
use spire_core::frontier::{Frontier, FrontierExt, InMemFrontier, Pop};

/// Seeds per push/pop round.
const SEEDS: usize = 1_000;

fn seeds() -> Vec<Seed> {
    (0..SEEDS)
        .map(|i| {
            let seed = Seed::get(format!("https://example.test/items/{i}?page={}", i % 7)).unwrap();
            seed.with_priority(i32::try_from(i % 5).unwrap())
        })
        .collect()
}

/// Pushes every seed, then pops them all.
async fn round<F: Frontier>(frontier: F, seeds: Vec<Seed>) {
    for seed in seeds {
        frontier.push(seed).await.unwrap();
    }
    while let Pop::Ready(_) = frontier.pop().await.unwrap() {}
}

fn frontiers(c: &mut Criterion) {
    let runtime = runtime();
    let mut group = c.benchmark_group("frontier");
    group.throughput(Throughput::Elements(SEEDS as u64));

    group.bench_function("fifo", |b| {
        b.to_async(&runtime).iter_batched(
            || (InMemFrontier::fifo(), seeds()),
            |(frontier, seeds)| round(frontier, seeds),
            BatchSize::SmallInput,
        );
    });
    group.bench_function("priority", |b| {
        b.to_async(&runtime).iter_batched(
            || (InMemFrontier::priority(), seeds()),
            |(frontier, seeds)| round(frontier, seeds),
            BatchSize::SmallInput,
        );
    });
    group.bench_function("facades", |b| {
        b.to_async(&runtime).iter_batched(
            || {
                let frontier = InMemFrontier::fifo()
                    .dedup()
                    .allow_hosts(["example.test"])
                    .max_depth(5)
                    .retry(3)
                    .tag_backoff();
                (frontier, seeds())
            },
            |(frontier, seeds)| round(frontier, seeds),
            BatchSize::SmallInput,
        );
    });
    group.finish();
}

fn seed(c: &mut Criterion) {
    let seed = Seed::get("HTTPS://Example.COM:443/a/./b/../c%7e?utm_source=x&b=2&a=1#top").unwrap();
    let mut group = c.benchmark_group("seed");
    group.bench_function("canonical_uri", |b| {
        b.iter(|| std::hint::black_box(&seed).canonical_uri());
    });
    group.bench_function("fingerprint", |b| {
        b.iter(|| std::hint::black_box(&seed).fingerprint());
    });
    group.finish();
}

/// Fingerprints of `count` distinct URLs.
fn fingerprints(count: usize) -> Vec<Fingerprint> {
    (0..count)
        .map(|i| {
            Seed::get(format!("https://example.test/{i}"))
                .unwrap()
                .fingerprint()
        })
        .collect()
}

/// Inserts every fingerprint into `seen`.
async fn insert_all<S: SeenSet>(seen: S, fingerprints: Vec<Fingerprint>) {
    for fingerprint in fingerprints {
        seen.insert(fingerprint).await.unwrap();
    }
}

fn seen(c: &mut Criterion) {
    let runtime = runtime();
    let mut group = c.benchmark_group("seen");

    for count in [10_000, 1_000_000] {
        let inserted = fingerprints(count);
        group.throughput(Throughput::Elements(count as u64));
        group.sample_size(10);

        group.bench_function(format!("memory/{count}"), |b| {
            b.to_async(&runtime).iter_batched(
                || (InMemSeen::default(), inserted.clone()),
                |(seen, fingerprints)| insert_all(seen, fingerprints),
                BatchSize::LargeInput,
            );
        });
        group.bench_function(format!("bloom/{count}"), |b| {
            b.to_async(&runtime).iter_batched(
                || (BloomSeen::new(count, 0.01), inserted.clone()),
                |(seen, fingerprints)| insert_all(seen, fingerprints),
                BatchSize::LargeInput,
            );
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

criterion_group!(benches, frontiers, seed, seen);
criterion_main!(benches);
