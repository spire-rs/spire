//! HTML parsing and `#[derive(Select)]` extraction.

use std::fmt::Write;

use criterion::{Criterion, Throughput, criterion_group, criterion_main};
use spire::extract::{Html, Select};

#[derive(Debug, Select)]
#[select("article.product")]
#[expect(
    dead_code,
    reason = "the fields are only extracted, to measure extraction"
)]
struct Product {
    #[select("h2")]
    name: String,
    #[select(".price")]
    price: Option<f64>,
    #[select("a.tag")]
    tags: Vec<String>,
    #[select("a.more", attr = "href")]
    link: String,
}

/// Products on the listing page.
const PRODUCTS: usize = 500;

/// A listing page of `PRODUCTS` products.
fn listing() -> String {
    let mut page = String::from("<html><head><title>Shop</title></head><body>");
    for i in 0..PRODUCTS {
        write!(
            page,
            r#"<article class="product"><h2>Product {i}</h2><span class="price">{i}.5</span>
            <a class="tag">new</a><a class="tag">sale</a><a class="more" href="/items/{i}">more</a></article>"#,
        )
        .unwrap();
    }
    page.push_str("</body></html>");
    page
}

fn extract(c: &mut Criterion) {
    let page = listing();
    let parsed = Html::parse(&page);

    let mut group = c.benchmark_group("extract");
    group.throughput(Throughput::Bytes(page.len() as u64));
    group.bench_function("parse", |b| {
        b.iter(|| Html::parse(std::hint::black_box(&page)));
    });

    group.throughput(Throughput::Elements(PRODUCTS as u64));
    group.bench_function("select", |b| {
        b.iter(|| parsed.select::<Vec<Product>>().unwrap());
    });
    group.bench_function("links", |b| b.iter(|| parsed.links()));
    group.finish();
}

criterion_group!(benches, extract);
criterion_main!(benches);
