//! Redirects must pass crawl admission before the target receives a request.

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use spire_core::Context;
use spire_core::backend::{Backend, Session};
use spire_core::crawler::Crawler;
use spire_core::frontier::seed::Seed;
use spire_core::frontier::{FrontierExt, InMemFrontier};
use spire_core::http::http::header::{
    AUTHORIZATION, CONTENT_ENCODING, CONTENT_TYPE, PROXY_AUTHORIZATION, WWW_AUTHENTICATE,
};
use spire_core::http::http::{HeaderMap, HeaderValue, Method};
use spire_core::worker::from_fn;
use spire_reqwest::{ClientConfig, ReqwestBackend, ReqwestSession, reqwest};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

type Received = Arc<Mutex<Vec<(String, Instant)>>>;

async fn server(reply: impl Fn(&str) -> String + Send + Sync + 'static) -> (String, Received) {
    delayed_server(Duration::ZERO, Duration::ZERO, reply).await
}

async fn delayed_server(
    head_delay: Duration,
    body_delay: Duration,
    reply: impl Fn(&str) -> String + Send + Sync + 'static,
) -> (String, Received) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let received = Arc::new(Mutex::new(Vec::new()));
    let seen = received.clone();
    let reply = Arc::new(reply);
    tokio::spawn(async move {
        while let Ok((mut stream, _)) = listener.accept().await {
            let seen = seen.clone();
            let reply = reply.clone();
            tokio::spawn(async move {
                let mut bytes = Vec::new();
                let mut buffer = [0; 4096];
                loop {
                    let n = stream.read(&mut buffer).await.unwrap();
                    if n == 0 {
                        return;
                    }
                    bytes.extend_from_slice(&buffer[..n]);
                    if let Some(end) = bytes.windows(4).position(|part| part == b"\r\n\r\n") {
                        let head = String::from_utf8_lossy(&bytes[..end]);
                        let length = head
                            .lines()
                            .filter_map(|line| line.split_once(':'))
                            .find(|(name, _)| name.eq_ignore_ascii_case("content-length"))
                            .map_or(0, |(_, length)| length.trim().parse::<usize>().unwrap());
                        let chunked = head
                            .lines()
                            .any(|line| line.eq_ignore_ascii_case("transfer-encoding: chunked"));
                        if chunked && bytes[end + 4..].ends_with(b"0\r\n\r\n")
                            || !chunked && bytes.len() >= end + 4 + length
                        {
                            break;
                        }
                    }
                }
                let request = String::from_utf8(bytes).unwrap();
                seen.lock().unwrap().push((request.clone(), Instant::now()));
                let response = reply(&request);
                let split = response.find("\r\n\r\n").unwrap() + 4;
                tokio::time::sleep(head_delay).await;
                if stream
                    .write_all(&response.as_bytes()[..split])
                    .await
                    .is_err()
                {
                    return;
                }
                tokio::time::sleep(body_delay).await;
                let _ = stream.write_all(&response.as_bytes()[split..]).await;
            });
        }
    });
    (url, received)
}

fn answer(status: &str, headers: &str) -> String {
    format!("HTTP/1.1 {status}\r\n{headers}Content-Length: 0\r\nConnection: close\r\n\r\n")
}

#[tokio::test]
async fn a_single_timeout_covers_redirect_hops_and_the_final_body() {
    use spire_core::ErrorKind;

    for delayed_body in [false, true] {
        let pause = Duration::from_millis(200);
        let (target, received) = delayed_server(
            if delayed_body { Duration::ZERO } else { pause },
            if delayed_body { pause } else { Duration::ZERO },
            |_| "HTTP/1.1 200 OK\r\nContent-Length: 4\r\nConnection: close\r\n\r\ndone".to_owned(),
        )
        .await;
        let (source, _) = delayed_server(pause, Duration::ZERO, move |_| {
            answer("302 Found", &format!("Location: {target}/target\r\n"))
        })
        .await;
        let backend = ReqwestBackend::from_builder(|| {
            ClientConfig::new().timeout(Duration::from_millis(350))
        })
        .unwrap();
        if delayed_body {
            // No crawler timeout or retries: the backend must bound body
            // consumption.
            let worker = from_fn(|cx: Context<ReqwestSession>| async move {
                let error = cx.body().await.unwrap_err();
                assert_eq!(error.kind(), ErrorKind::Timeout);
                Err::<(), _>(error)
            });
            let stats = Crawler::new(backend, worker)
                .with_frontier(InMemFrontier::fifo())
                .with_seed(Seed::get(source).unwrap())
                .run()
                .await
                .unwrap();
            assert_eq!((stats.failed, stats.retried), (1, 0));
        } else {
            let session = backend.connect().await.unwrap();
            let error = session
                .fetch(&Seed::get(source).unwrap().into_request())
                .await
                .unwrap_err();
            assert_eq!(error.kind(), ErrorKind::Timeout);
        }
        assert_eq!(received.lock().unwrap().len(), 1);
    }
}

#[tokio::test]
async fn cross_origin_redirects_keep_transport_proxy_authentication() {
    for explicit_header in [false, true] {
        let (proxy, received) = server(|request| {
            if request.starts_with("GET http://source.test/start ") {
                answer("302 Found", "Location: http://target.test/target\r\n")
            } else if request.contains("proxy-authorization: Basic dXNlcjpwYXNz\r\n") {
                answer("200 OK", "")
            } else {
                answer("407 Proxy Authentication Required", "")
            }
        })
        .await;
        let backend = ReqwestBackend::from_builder(move || {
            let mut headers = HeaderMap::new();
            headers.insert(
                PROXY_AUTHORIZATION,
                HeaderValue::from_static("Basic default-secret"),
            );
            ClientConfig::new()
                .default_headers(headers)
                .transport(|builder| {
                    builder.proxy(
                        reqwest::Proxy::http(&proxy)
                            .unwrap()
                            .basic_auth("user", "pass"),
                    )
                })
        })
        .unwrap();
        let mut seed = Seed::get("http://source.test/start").unwrap();
        if explicit_header {
            seed = seed.with_header(
                PROXY_AUTHORIZATION,
                HeaderValue::from_static("Basic seed-secret"),
            );
        }
        let session = backend.connect().await.unwrap();
        session.fetch(&seed.into_request()).await.unwrap();
        let received = received.lock().unwrap();
        assert_eq!(received.len(), 2);
        assert!(received[0].0.contains(if explicit_header {
            "seed-secret"
        } else {
            "default-secret"
        }));
        assert!(
            received[1]
                .0
                .contains("proxy-authorization: Basic dXNlcjpwYXNz\r\n")
        );
        assert!(!received[1].0.contains("secret"));
    }
}

#[tokio::test]
async fn host_scope_is_checked_before_sending_a_redirect_target() {
    let (target, received) = server(|_| answer("200 OK", "")).await;
    let target = target.replace("127.0.0.1", "localhost");
    let (source, _) =
        server(move |_| answer("302 Found", &format!("Location: {target}/secret\r\n"))).await;
    let worker = from_fn(|cx: Context<ReqwestSession>| async move { cx.body().await.map(drop) });
    let stats = Crawler::new(ReqwestBackend::new(), worker)
        .with_frontier(
            InMemFrontier::fifo()
                .allow_hosts(["127.0.0.1"])
                .dedup()
                .retry(3),
        )
        .with_seed(Seed::get(source).unwrap())
        .with_request_timeout(Duration::from_secs(2))
        .run()
        .await
        .unwrap();
    assert_eq!((stats.failed, stats.retried), (1, 0));
    assert!(received.lock().unwrap().is_empty());
}

#[tokio::test]
async fn same_host_redirect_waits_without_deadlocking_a_full_crawler() {
    let (source, received) = server(|request| {
        if request.starts_with("GET /start ") {
            answer("302 Found", "Location: /target#fragment\r\n")
        } else {
            answer("200 OK", "")
        }
    })
    .await;
    let worker = from_fn(|cx: Context<ReqwestSession>| async move { cx.body().await.map(drop) });
    let stats = Crawler::new(ReqwestBackend::new(), worker)
        .with_concurrency(1)
        .with_host_concurrency(1)
        .with_host_delay(Duration::from_millis(30))
        .with_request_timeout(Duration::from_secs(2))
        .with_seed(Seed::get(format!("{source}/start")).unwrap())
        .run()
        .await
        .unwrap();
    assert_eq!(stats.succeeded, 1);
    let received = received.lock().unwrap();
    assert_eq!(received.len(), 2);
    assert!(received[1].0.starts_with("GET /target HTTP/1.1"));
    assert!(received[1].1.duration_since(received[0].1) >= Duration::from_millis(25));
}

#[tokio::test]
async fn cross_origin_redirects_do_not_restore_sensitive_client_defaults() {
    let (target, received) = server(|_| answer("200 OK", "")).await;
    let (source, _) =
        server(move |_| answer("302 Found", &format!("Location: {target}/target\r\n"))).await;
    let backend = ReqwestBackend::from_builder(|| {
        let mut headers = HeaderMap::new();
        headers.insert(AUTHORIZATION, HeaderValue::from_static("Bearer secret"));
        headers.insert(
            PROXY_AUTHORIZATION,
            HeaderValue::from_static("Basic proxy-secret"),
        );
        headers.insert("cookie2", HeaderValue::from_static("cookie2-secret"));
        headers.insert(
            WWW_AUTHENTICATE,
            HeaderValue::from_static("challenge-secret"),
        );
        ClientConfig::new().default_headers(headers)
    })
    .unwrap();
    let worker = from_fn(|cx: Context<ReqwestSession>| async move { cx.body().await.map(drop) });
    let stats = Crawler::new(backend, worker)
        .with_seed(Seed::get(source).unwrap())
        .run()
        .await
        .unwrap();
    assert_eq!(stats.succeeded, 1);
    assert!(!received.lock().unwrap()[0].0.contains("secret"));
}

#[tokio::test]
async fn sessions_follow_redirects_and_the_raw_client_fetches_one_hop() {
    for status in ["303 See Other", "307 Temporary Redirect"] {
        let (source, received) = server(move |request| {
            if request.starts_with("POST /start ") {
                answer(status, "Location: /target\r\n")
            } else {
                answer("200 OK", "")
            }
        })
        .await;
        let backend = ReqwestBackend::new();
        let session = backend.connect().await.unwrap();
        let request = Seed::new(Method::POST, format!("{source}/start").parse().unwrap())
            .unwrap()
            .with_body("payload")
            .with_header(CONTENT_TYPE, HeaderValue::from_static("text/plain"))
            .with_header(CONTENT_ENCODING, HeaderValue::from_static("gzip"))
            .into_request();
        session.fetch(&request).await.unwrap();
        let received = received.lock().unwrap();
        assert_eq!(received.len(), 2);
        if status.starts_with("303") {
            assert!(received[1].0.starts_with("GET /target "));
            assert!(!received[1].0.contains("content-type: text/plain"));
            assert!(!received[1].0.contains("content-encoding:"));
        } else {
            assert!(received[1].0.starts_with("POST /target "));
            assert!(received[1].0.contains("content-type: text/plain"));
            assert!(received[1].0.contains("content-encoding: gzip"));
            assert!(received[1].0.ends_with("payload"));
        }
    }
    let (source, _) = server(|request| {
        if request.starts_with("GET /start ") {
            answer("302 Found", "Location: /target\r\n")
        } else {
            answer("200 OK", "")
        }
    })
    .await;
    let backend = ReqwestBackend::from_builder(|| {
        ClientConfig::new().transport(|builder| builder.redirect(reqwest::redirect::Policy::none()))
    })
    .unwrap();
    let request = Seed::get(format!("{source}/start")).unwrap().into_request();
    assert_eq!(
        backend
            .connect()
            .await
            .unwrap()
            .fetch(&request)
            .await
            .unwrap()
            .status(),
        200
    );
    assert_eq!(
        backend
            .as_reqwest()
            .get(format!("{source}/start"))
            .send()
            .await
            .unwrap()
            .status(),
        302
    );
}

#[cfg(feature = "cookies")]
#[tokio::test]
async fn cross_origin_redirects_send_the_destination_jars_cookies() {
    use spire_core::http::http::header::COOKIE;

    let (target, received) = server(|request| {
        if request.starts_with("GET /warm ") {
            answer("200 OK", "Set-Cookie: session=destination; Path=/\r\n")
        } else {
            answer("200 OK", "")
        }
    })
    .await;
    let destination = target.clone();
    let (source, _) =
        server(move |_| answer("302 Found", &format!("Location: {destination}/target\r\n"))).await;
    let backend = ReqwestBackend::from_builder(|| {
        let mut headers = HeaderMap::new();
        headers.insert(COOKIE, HeaderValue::from_static("source=secret"));
        ClientConfig::new().default_headers(headers)
    })
    .unwrap();
    let worker = from_fn(|cx: Context<ReqwestSession>| async move { cx.body().await.map(drop) });
    let stats = Crawler::new(backend, worker)
        .with_concurrency(1)
        .with_seeds([
            Seed::get(format!("{target}/warm")).unwrap(),
            Seed::get(source).unwrap(),
        ])
        .run()
        .await
        .unwrap();
    assert_eq!(stats.succeeded, 2);
    let requests = received.lock().unwrap();
    assert!(requests[1].0.contains("cookie: session=destination"));
    assert!(!requests[1].0.contains("secret"));
}

#[tokio::test]
async fn retrying_a_failed_fetch_readmits_the_source_host() {
    let (target, _) = server(|_| answer("408 Request Timeout", "")).await;
    let (source, received) =
        server(move |_| answer("302 Found", &format!("Location: {target}/target\r\n"))).await;
    let worker = from_fn(|cx: Context<ReqwestSession>| async move {
        assert!(cx.body().await.is_err());
        assert!(cx.body().await.is_err());
    });
    let stats = Crawler::new(ReqwestBackend::new(), worker)
        .with_host_delay(Duration::from_millis(100))
        .with_seed(Seed::get(source).unwrap())
        .run()
        .await
        .unwrap();
    assert_eq!(stats.succeeded, 1);
    let received = received.lock().unwrap();
    assert_eq!(received.len(), 2);
    assert!(received[1].1.duration_since(received[0].1) >= Duration::from_millis(90));
}

#[tokio::test]
async fn method_rewrites_suppress_default_body_headers() {
    let (source, received) = server(|request| {
        if request.starts_with("POST /start ") {
            answer("303 See Other", "Location: /target\r\n")
        } else {
            answer("200 OK", "")
        }
    })
    .await;
    let backend = ReqwestBackend::from_builder(|| {
        let mut headers = HeaderMap::new();
        headers.insert("content-length", HeaderValue::from_static("7"));
        headers.insert("transfer-encoding", HeaderValue::from_static("chunked"));
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("text/plain"));
        headers.insert(CONTENT_ENCODING, HeaderValue::from_static("gzip"));
        ClientConfig::new()
            .default_headers(headers)
            .timeout(Duration::from_secs(2))
    })
    .unwrap();
    let request = Seed::new(Method::POST, format!("{source}/start").parse().unwrap())
        .unwrap()
        .with_body("payload")
        .into_request();
    backend
        .connect()
        .await
        .unwrap()
        .fetch(&request)
        .await
        .unwrap();
    let received = received.lock().unwrap();
    assert_eq!(received.len(), 2);
    assert!(received[1].0.starts_with("GET /target "));
    assert!(!received[1].0.contains("content-length: 7"));
    assert!(!received[1].0.contains("text/plain"));
    assert!(!received[1].0.contains("content-encoding:"));
    assert!(!received[1].0.contains("transfer-encoding:"));
}

#[tokio::test]
async fn normalizing_the_source_url_does_not_consume_another_host_admission() {
    let (source, received) = server(|_| answer("200 OK", "")).await;
    let worker = from_fn(|cx: Context<ReqwestSession>| async move { cx.body().await.map(drop) });
    let stats = Crawler::new(ReqwestBackend::new(), worker)
        .with_host_delay(Duration::from_secs(2))
        .with_request_timeout(Duration::from_secs(1))
        .with_seed(Seed::get(format!("{source}/unused/../target")).unwrap())
        .run()
        .await
        .unwrap();
    assert_eq!((stats.succeeded, stats.failed), (1, 0));
    assert!(received.lock().unwrap()[0].0.starts_with("GET /target "));
}
