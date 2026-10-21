//! Router dispatch: by tag, by path pattern, and to the fallback.

use criterion::{Criterion, criterion_group, criterion_main};
use spire::backend::Noop;
use spire::frontier::seed::Seed;
use spire::worker::Worker;
use spire::{Context, Router};

/// A router with 20 tag routes and 20 path routes, all doing nothing.
fn router() -> Router<Noop> {
    let mut router = Router::new();
    for i in 0..20 {
        router = router
            .route(format!("tag{i}"), || async {})
            .route_path(&format!("/section{i}/**/item-*.html"), || async {});
    }
    router.fallback(|| async {})
}

fn context(uri: &str, tag: Option<&str>) -> Context<Noop> {
    let seed = Seed::get(uri).unwrap();
    let seed = match tag {
        Some(tag) => seed.with_tag(tag),
        None => seed,
    };
    Context::new(seed.into_request(), Noop)
}

fn routing(c: &mut Criterion) {
    let runtime = runtime();
    let router = router();
    let long = "https://example.test/section19/a/b/c/d/e/f/g/h/item-42.html";

    let mut group = c.benchmark_group("routing");
    group.bench_function("tag", |b| {
        b.to_async(&runtime)
            .iter(|| router.handle(context("https://example.test/", Some("tag19"))));
    });
    group.bench_function("path", |b| {
        b.to_async(&runtime)
            .iter(|| router.handle(context(long, None)));
    });
    group.bench_function("fallback", |b| {
        b.to_async(&runtime)
            .iter(|| router.handle(context("https://example.test/other", None)));
    });
    group.finish();
}

/// A current-thread runtime: steadier timings than a thread pool.
fn runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
}

criterion_group!(benches, routing);
criterion_main!(benches);
