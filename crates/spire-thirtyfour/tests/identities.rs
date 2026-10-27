//! Crawls with identities in a real browser, through local proxies.
//!
//! Skipped unless `SPIRE_WEBDRIVER` names a WebDriver server with BiDi, e.g.
//! `SPIRE_WEBDRIVER=http://127.0.0.1:4444` with `chromedriver --port=4444`.
//!
//! Each proxy answers requests itself rather than forwarding them, with
//! what it received: its own name, the request target, and the user agent,
//! identity header and cookies sent. It sets a cookie naming itself, and
//! one with `credentials` asks for them first.

use std::sync::{Arc, Mutex};

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use spire_core::frontier::seed::Seed;
use spire_core::http::HeaderValue;
use spire_core::http::header::{HeaderName, USER_AGENT};
use spire_core::identity::{Identities, Identity};
use spire_core::worker::from_fn;
use spire_core::{Context, Crawler, Error};
use spire_thirtyfour::{ThirtyfourBackend, ThirtyfourSession};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

/// Starts a proxy named `name`, returning its address.
async fn proxy(name: &'static str, credentials: Option<&'static str>) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let expected = credentials.map(|credentials| format!("basic {}", STANDARD.encode(credentials)));
    tokio::spawn(async move {
        while let Ok((mut stream, _)) = listener.accept().await {
            let expected = expected.clone();
            tokio::spawn(async move {
                let mut received = Vec::new();
                let mut buffer = [0; 4096];
                while !received.windows(4).any(|window| window == b"\r\n\r\n") {
                    match stream.read(&mut buffer).await {
                        Ok(0) | Err(_) => return,
                        Ok(read) => received.extend_from_slice(&buffer[..read]),
                    }
                }
                let received = String::from_utf8_lossy(&received).into_owned();
                let header = |name: &str| {
                    received
                        .lines()
                        .find_map(|line| {
                            let (key, value) = line.split_once(": ")?;
                            key.eq_ignore_ascii_case(name).then(|| value.to_owned())
                        })
                        .unwrap_or_default()
                };
                if let Some(expected) = &expected
                    && !header("proxy-authorization").eq_ignore_ascii_case(expected)
                {
                    let refused = "HTTP/1.1 407 Proxy Authentication Required\r\nProxy-Authenticate: Basic realm=\"test\"\r\nContent-Length: 0\r\nConnection: close\r\n\r\n";
                    let _ = stream.write_all(refused.as_bytes()).await;
                    return;
                }
                let target = received
                    .lines()
                    .next()
                    .and_then(|line| line.split(' ').nth(1))
                    .unwrap_or_default()
                    .to_owned();
                let body = format!(
                    "<p>{name} {target} agent={} identity={} cookie={}</p>",
                    header("user-agent"),
                    header("x-identity"),
                    header("cookie"),
                );
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\nSet-Cookie: via={name}; Path=/\r\nConnection: close\r\n\r\n{body}",
                    body.len(),
                );
                let _ = stream.write_all(response.as_bytes()).await;
            });
        }
    });
    address.to_string()
}

fn identity(name: &str, proxy: &str) -> Identity {
    let header = |value: String| HeaderValue::from_str(&value).unwrap();
    Identity::new(name)
        .with_proxy(proxy)
        .unwrap()
        .with_header(USER_AGENT, header(format!("Bot/{name}")))
        .with_header(
            HeaderName::from_static("x-identity"),
            header(name.to_owned()),
        )
}

/// Crawls `pages` of `http://site.test/` one at a time as `identities`,
/// returning the text of each page.
async fn crawl(identities: Identities, pages: usize) -> Option<Vec<String>> {
    let url = std::env::var("SPIRE_WEBDRIVER").ok()?;
    let backend = ThirtyfourBackend::builder()
        .remote(url)
        .max_sessions(1)
        .build()
        .unwrap();

    let texts = Arc::new(Mutex::new(Vec::new()));
    let seen = Arc::clone(&texts);
    let worker = from_fn(move |cx: Context<ThirtyfourSession>| {
        let seen = Arc::clone(&seen);
        async move {
            cx.response().await?;
            let body = String::from_utf8_lossy(&cx.body().await?).into_owned();
            let text = body
                .split("<p>")
                .nth(1)
                .and_then(|rest| rest.split("</p>").next())
                .unwrap_or(&body)
                .to_owned();
            seen.lock().unwrap().push(text);
            Ok::<_, Error>(())
        }
    });
    let seeds = (1..=pages).map(|page| Seed::get(format!("http://site.test/{page}")).unwrap());
    let crawler = Crawler::new(backend.clone(), worker)
        .with_concurrency(1)
        .with_identities(identities)
        .with_seeds(seeds);
    let stats = crawler.run().await.unwrap();
    drop(crawler);
    backend.close().await;
    assert_eq!(stats.failed, 0, "{stats:?}");
    let texts = texts.lock().unwrap().clone();
    Some(texts)
}

#[tokio::test(flavor = "multi_thread")]
async fn pages_load_through_their_identitys_proxy() {
    let (a, b) = (proxy("a", None).await, proxy("b", None).await);
    let identities = Identities::new([
        identity("a", &format!("http://{a}")),
        identity("b", &format!("http://{b}")),
    ]);
    let Some(texts) = crawl(identities, 2).await else {
        return;
    };
    assert!(
        texts[0].starts_with("a http://site.test/1 agent=Bot/a identity=a"),
        "{}",
        texts[0]
    );
    assert!(
        texts[1].starts_with("b http://site.test/2 agent=Bot/b identity=b"),
        "{}",
        texts[1]
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn proxies_get_their_credentials() {
    let gate = proxy("gate", Some("user:secret")).await;
    let identities = Identities::new([identity("a", &format!("http://user:secret@{gate}"))]);
    let Some(texts) = crawl(identities, 1).await else {
        return;
    };
    assert!(
        texts[0].starts_with("gate http://site.test/1"),
        "{}",
        texts[0]
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn each_identity_keeps_its_own_cookies() {
    let (a, b) = (proxy("a", None).await, proxy("b", None).await);
    let identities = Identities::new([
        identity("a", &format!("http://{a}")),
        identity("b", &format!("http://{b}")),
    ]);
    let Some(texts) = crawl(identities, 4).await else {
        return;
    };
    // The first page of each identity has no cookie; the second gets back
    // only the cookie its own proxy set.
    assert!(texts[0].ends_with("cookie="), "{}", texts[0]);
    assert!(texts[1].ends_with("cookie="), "{}", texts[1]);
    assert!(texts[2].ends_with("cookie=via=a"), "{}", texts[2]);
    assert!(texts[3].ends_with("cookie=via=b"), "{}", texts[3]);
}
