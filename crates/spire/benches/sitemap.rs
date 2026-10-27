//! Parsing a full sitemap through the `Sitemap` extractor, plain and gzipped.

use std::fmt::Write as _;
use std::future::{Future, ready};
use std::io::Write as _;

use bytes::Bytes;
use criterion::{Criterion, Throughput, criterion_group, criterion_main};
use flate2::Compression;
use flate2::write::GzEncoder;
use spire::backend::Session;
use spire::extract::FromContext;
use spire::extract::sitemap::Sitemap;
use spire::frontier::seed::Seed;
use spire::http::{Request, Response};
use spire::{Context, Result};

/// URLs in the sitemap: the protocol's limit.
const URLS: usize = 50_000;

/// A client serving one fixed body.
struct Serve(Bytes);

impl Session for Serve {
    fn fetch(&self, _request: &Request) -> impl Future<Output = Result<Response>> + Send {
        ready(Ok(Response::new(self.0.clone())))
    }
}

fn sitemap_xml() -> Vec<u8> {
    let mut xml = String::from(r#"<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">"#);
    for i in 0..URLS {
        write!(
            xml,
            "<url><loc>https://example.test/items/{i}</loc><priority>0.{}</priority></url>",
            i % 10
        )
        .unwrap();
    }
    xml.push_str("</urlset>");
    xml.into_bytes()
}

fn gzip(bytes: &[u8]) -> Vec<u8> {
    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(bytes).unwrap();
    encoder.finish().unwrap()
}

/// Extracts the sitemap from a context serving `body`.
async fn extract(body: Bytes) -> usize {
    let request = Seed::get("https://example.test/sitemap.xml").unwrap();
    let cx = Context::new(request.into_request(), Serve(body));
    let Sitemap(records) = Sitemap::from_context(&cx, &()).await.unwrap();
    records.len()
}

fn sitemap(c: &mut Criterion) {
    let runtime = runtime();
    let plain = Bytes::from(sitemap_xml());
    let gzipped = Bytes::from(gzip(&plain));

    let mut group = c.benchmark_group("sitemap");
    group.throughput(Throughput::Elements(URLS as u64));
    group.sample_size(20);
    group.bench_function("plain", |b| {
        b.to_async(&runtime).iter(|| extract(plain.clone()));
    });
    group.bench_function("gzip", |b| {
        b.to_async(&runtime).iter(|| extract(gzipped.clone()));
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

criterion_group!(benches, sitemap);
criterion_main!(benches);
