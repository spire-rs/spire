//! Crawls with identities, through local proxies.
//!
//! Each proxy answers requests itself rather than forwarding them, with
//! what it received: its own name, the request target, and the user agent
//! and cookies sent. It sets a cookie naming itself.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use spire_core::crawler::Stats;
use spire_core::frontier::facade::RetryPolicy;
use spire_core::frontier::seed::Seed;
use spire_core::frontier::{FrontierExt, InMemFrontier};
use spire_core::http::HeaderValue;
use spire_core::http::header::USER_AGENT;
use spire_core::identity::{Identities, Identity};
use spire_core::worker::from_fn;
use spire_core::{Context, Crawler, Error};
use spire_reqwest::{HttpClient, HttpSession};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

/// Starts a proxy named `name`, returning its URL.
async fn proxy(name: &'static str) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    tokio::spawn(async move {
        while let Ok((mut stream, _)) = listener.accept().await {
            tokio::spawn(async move {
                let mut received = Vec::new();
                let mut buffer = [0; 1024];
                while !received.ends_with(b"\r\n\r\n") {
                    let Ok(read) = stream.read(&mut buffer).await else {
                        return;
                    };
                    if read == 0 {
                        return;
                    }
                    received.extend_from_slice(&buffer[..read]);
                }
                let received = String::from_utf8_lossy(&received);
                let mut lines = received.lines();
                let target = lines
                    .next()
                    .and_then(|line| line.split(' ').nth(1))
                    .unwrap_or_default()
                    .to_owned();
                let header = |name: &str| {
                    received
                        .lines()
                        .find_map(|line| {
                            let (key, value) = line.split_once(": ")?;
                            key.eq_ignore_ascii_case(name).then(|| value.to_owned())
                        })
                        .unwrap_or_default()
                };
                let body = format!(
                    "{name} {target} agent={} cookie={}",
                    header("user-agent"),
                    header("cookie"),
                );
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nSet-Cookie: via={name}; Path=/\r\nConnection: close\r\n\r\n{body}",
                    body.len(),
                );
                let _ = stream.write_all(response.as_bytes()).await;
            });
        }
    });
    format!("http://{address}")
}

/// A proxy URL nothing listens on: connections are refused.
async fn dead_proxy() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    drop(listener);
    format!("http://{address}")
}

fn identity(name: &str, proxy: &str) -> Identity {
    let agent = HeaderValue::from_str(&format!("Bot/{name}")).unwrap();
    Identity::new(name)
        .with_proxy(proxy)
        .unwrap()
        .with_header(USER_AGENT, agent)
}

fn seeds(n: usize) -> Vec<Seed> {
    (1..=n)
        .map(|page| Seed::get(format!("http://site.test/{page}")).unwrap())
        .collect()
}

/// Crawls `seeds` one at a time as `identities`, returning the bodies the
/// proxies answered with.
async fn crawl(
    backend: HttpClient,
    identities: Identities,
    seeds: Vec<Seed>,
) -> (Stats, Vec<String>) {
    let bodies = Arc::new(Mutex::new(Vec::new()));
    let seen = Arc::clone(&bodies);
    let worker = from_fn(move |cx: Context<HttpSession>| {
        let seen = Arc::clone(&seen);
        async move {
            cx.response().await?;
            let body = cx.body().await?;
            seen.lock()
                .unwrap()
                .push(String::from_utf8_lossy(&body).into_owned());
            Ok::<_, Error>(())
        }
    });
    let retries = RetryPolicy::new(3).with_base_delay(Duration::from_millis(10));
    let crawler = Crawler::new(backend, worker)
        .with_concurrency(1)
        .with_frontier(InMemFrontier::fifo().retry_with(retries))
        .with_identities(identities)
        .with_seeds(seeds);
    let stats = crawler.run().await.unwrap();
    let bodies = bodies.lock().unwrap().clone();
    (stats, bodies)
}

#[tokio::test]
async fn requests_go_through_their_identitys_proxy() {
    let (a, b) = (proxy("a").await, proxy("b").await);
    let identities = Identities::new([identity("a", &a), identity("b", &b)]);
    let (stats, bodies) = crawl(HttpClient::new(), identities, seeds(4)).await;

    assert_eq!(stats.succeeded, 4, "{stats:?}");
    for (body, (name, page)) in bodies.iter().zip([("a", 1), ("b", 2), ("a", 3), ("b", 4)]) {
        let expected = format!("{name} http://site.test/{page} agent=Bot/{name}");
        assert!(body.starts_with(&expected), "{body}");
    }
}

#[cfg(feature = "cookies")]
#[tokio::test]
async fn each_identity_keeps_its_own_cookies() {
    let (a, b) = (proxy("a").await, proxy("b").await);
    let identities = Identities::new([identity("a", &a), identity("b", &b)]);
    let (_, bodies) = crawl(HttpClient::new(), identities, seeds(4)).await;

    // The first request of each identity has no cookie; the second gets
    // back only the cookie its own proxy set.
    assert!(bodies[0].ends_with("cookie="), "{}", bodies[0]);
    assert!(bodies[1].ends_with("cookie="), "{}", bodies[1]);
    assert!(bodies[2].ends_with("cookie=via=a"), "{}", bodies[2]);
    assert!(bodies[3].ends_with("cookie=via=b"), "{}", bodies[3]);
}

#[tokio::test]
async fn a_dead_proxy_is_set_aside_and_its_requests_retried() {
    let (dead, live) = (dead_proxy().await, proxy("live").await);
    let identities =
        Identities::new([identity("dead", &dead), identity("live", &live)]).max_failures(1);
    let (stats, bodies) = crawl(HttpClient::new(), identities, seeds(3)).await;

    assert_eq!((stats.succeeded, stats.failed), (3, 0), "{stats:?}");
    assert!(stats.retried >= 1, "{stats:?}");
    assert!(
        bodies.iter().all(|body| body.starts_with("live ")),
        "{bodies:?}"
    );
}

// `socks5` needs reqwest's `socks` feature.
#[cfg(not(feature = "socks"))]
#[tokio::test]
async fn unsupported_proxies_fail_without_retrying() {
    let socks = Identity::new("socks")
        .with_proxy("socks5://127.0.0.1:1080")
        .unwrap();
    let (stats, _) = crawl(HttpClient::new(), Identities::new([socks]), seeds(1)).await;
    assert_eq!((stats.failed, stats.retried), (1, 0), "{stats:?}");
}
