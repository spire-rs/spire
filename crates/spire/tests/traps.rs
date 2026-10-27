//! Hidden links, expanding trap branches, and legitimate deep sites.
#![cfg(all(feature = "html", feature = "testing", feature = "robots"))]

use std::fmt::Write;
use std::sync::Arc;

use spire::crawler::Robots;
use spire::frontier::facade::TrapPolicy;
use spire::frontier::seed::Seed;
use spire::frontier::{FrontierExt, InMemFrontier};
use spire::handler::FollowLinks;
use spire::testing::{MockPage, MockSite};
use spire::{Crawler, Router};

#[tokio::test]
async fn default_and_selected_links_skip_hidden_traps_but_can_opt_in() {
    for follow in [
        FollowLinks::new("page"),
        FollowLinks::new("page").select("a"),
    ] {
        let site = MockSite::new()
            .page("https://site.test/", MockPage::html(r#"<a href="/safe">safe</a><div style="display:none"><a href="/trap/">trap</a></div>"#))
            .page("https://site.test/safe", MockPage::html("done"));
        let stats = Crawler::new(site.clone(), Router::new().fallback(follow))
            .with_frontier(InMemFrontier::fifo().dedup())
            .with_seed(Seed::get("https://site.test/").unwrap())
            .run()
            .await
            .unwrap();
        assert_eq!((stats.requests, stats.failed), (2, 0));
        assert!(
            !site
                .requests()
                .iter()
                .any(|uri| uri.path().contains("/trap/"))
        );
    }
    let site = MockSite::new()
        .page(
            "https://site.test/",
            MockPage::html(r#"<a hidden href="/trap/">trap</a>"#),
        )
        .page("https://site.test/trap/", MockPage::html("done"));
    let stats = Crawler::new(
        site,
        Router::new().fallback(FollowLinks::new("page").include_hidden()),
    )
    .with_seed(Seed::get("https://site.test/").unwrap())
    .run()
    .await
    .unwrap();
    assert_eq!((stats.requests, stats.failed), (2, 0));
}

#[tokio::test]
async fn robots_excludes_even_visible_trap_links() {
    let site = MockSite::new()
        .page(
            "https://site.test/robots.txt",
            MockPage::text("User-agent: *\nDisallow: /trap\n"),
        )
        .page(
            "https://site.test/",
            MockPage::html(r#"<a href="/trap/">trap</a>"#),
        );
    let stats = Crawler::new(
        site.clone(),
        Router::new().fallback(FollowLinks::new("page")),
    )
    .with_robots(Robots::new("Spire"))
    .with_seed(Seed::get("https://site.test/").unwrap())
    .run()
    .await
    .unwrap();
    assert_eq!(stats.skipped, 1);
    assert!(
        !site
            .requests()
            .iter()
            .any(|uri| uri.path().contains("/trap/"))
    );
}

#[tokio::test]
async fn visible_endlessly_deepening_branch_stops_and_other_paths_continue() {
    let mut site = MockSite::new().page("https://site.test/safe", MockPage::html("done"));
    for depth in 0..20 {
        let path = format!("/trap/{}", "step/".repeat(depth));
        let next = format!("{path}step/");
        site = site.page(
            &format!("https://site.test{path}"),
            MockPage::html(format!(
                r#"<a href="{next}">next</a><a href="/safe">safe</a>"#
            )),
        );
    }
    let frontier = Arc::new(InMemFrontier::fifo().dedup().avoid_traps());
    let stats = Crawler::new(
        site.clone(),
        Router::new().fallback(FollowLinks::new("page")),
    )
    .with_frontier(frontier.clone())
    .with_concurrency(1)
    .with_seed(Seed::get("https://site.test/trap/").unwrap())
    .run()
    .await
    .unwrap();
    assert_eq!(stats.failed, 0);
    assert_eq!(stats.requests, 6);
    assert_eq!(frontier.refused(), 1);
    assert!(
        site.requests()
            .iter()
            .any(|uri| uri.path().ends_with("/safe"))
    );
}

#[tokio::test]
async fn real_deep_archive_with_dates_pagination_and_siblings_is_crawled_fully() {
    let mut site = MockSite::new();
    // A deep hierarchy and multiple years/months of siblings must not be
    // mistaken for opaque random segments or repeated path components.
    let mut urls = vec!["https://site.test/".to_owned()];
    for year in 2020..2025 {
        for month in 1..=12 {
            urls.push(format!("https://site.test/archive/{year}/{month:02}/index"));
        }
    }
    let mut deep = "https://site.test/docs".to_owned();
    for level in 0..24 {
        write!(deep, "/chapter-{level}").unwrap();
        urls.push(deep.clone());
    }
    for (n, uri) in urls.iter().enumerate() {
        let body = urls
            .get(n + 1)
            .map_or_else(String::new, |next| format!(r#"<a href="{next}">next</a>"#));
        site = site.page(uri, MockPage::html(body));
    }
    let frontier = Arc::new(
        InMemFrontier::fifo()
            .dedup()
            .avoid_traps_with(TrapPolicy::default()),
    );
    let stats = Crawler::new(site, Router::new().fallback(FollowLinks::new("page")))
        .with_frontier(frontier.clone())
        .with_seed(Seed::get(&urls[0]).unwrap())
        .run()
        .await
        .unwrap();
    assert_eq!((stats.requests, stats.failed), (urls.len(), 0));
    assert_eq!(frontier.refused(), 0);
}
