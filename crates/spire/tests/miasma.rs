//! Run explicitly with `MIASMA_BIN` pointing to a local Miasma executable.
#![cfg(all(feature = "html", feature = "reqwest", feature = "robots"))]

use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use spire::backend::{ReqwestBackend, reqwest};
use spire::crawler::Robots;
use spire::frontier::facade::TrapPolicy;
use spire::frontier::seed::Seed;
use spire::frontier::{FrontierExt, InMemFrontier};
use spire::handler::FollowLinks;
use spire::{Crawler, Router};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::task::JoinHandle;

#[derive(Default)]
struct Site {
    robots: AtomicBool,
    hidden: AtomicBool,
    requests: Mutex<Vec<String>>,
}

struct Instance {
    process: Child,
    server: JoinHandle<()>,
    base: String,
    site: Arc<Site>,
}

impl Drop for Instance {
    fn drop(&mut self) {
        self.server.abort();
        let _ = self.process.kill();
        let _ = self.process.wait();
    }
}

impl Instance {
    async fn start() -> Self {
        let binary =
            std::env::var_os("MIASMA_BIN").expect("set MIASMA_BIN to the Miasma executable");
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let reservation = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = reservation.local_addr().unwrap().port();
        drop(reservation);
        let site = Arc::new(Site::default());
        let state = site.clone();
        let server = tokio::spawn(async move {
            while let Ok((stream, _)) = listener.accept().await {
                let state = state.clone();
                tokio::spawn(serve(stream, state, port));
            }
        });
        let process = Command::new(binary)
            .args([
                "--host",
                "127.0.0.1",
                "--port",
                &port.to_string(),
                "--link-prefix",
                "/naughty-bots",
                "--link-count",
                "5",
                "--max-depth",
                "20",
                "--poison-source",
                &format!("{base}/poison"),
            ])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let mut instance = Self {
            process,
            server,
            base,
            site,
        };
        for _ in 0..100 {
            if TcpStream::connect(("127.0.0.1", port)).await.is_ok() {
                return instance;
            }
            assert!(
                instance.process.try_wait().unwrap().is_none(),
                "Miasma exited during startup"
            );
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        panic!("Miasma did not start within five seconds");
    }
}

async fn serve(mut stream: TcpStream, site: Arc<Site>, port: u16) {
    let mut buffer = vec![0; 8192];
    let Ok(read) = stream.read(&mut buffer).await else {
        return;
    };
    let request = String::from_utf8_lossy(&buffer[..read]);
    let path = request.split_whitespace().nth(1).unwrap_or("/").to_owned();
    site.requests.lock().unwrap().push(path.clone());
    let (status, body) = match path.as_str() {
        "/robots.txt" => (
            200,
            if site.robots.load(Ordering::Relaxed) {
                "User-agent: *\nDisallow: /naughty-bots\n"
            } else {
                "User-agent: *\nDisallow:\n"
            }
            .as_bytes()
            .to_vec(),
        ),
        "/poison" => (200, b"Synthetic test content from a local source.".to_vec()),
        "/" => {
            let hidden = if site.hidden.load(Ordering::Relaxed) {
                r#"style="display: none;" aria-hidden="true" tabindex="-1""#
            } else {
                ""
            };
            (
                200,
                format!(r#"<a href="/naughty-bots/" {hidden}>Test link</a>"#).into_bytes(),
            )
        }
        path if path.starts_with("/naughty-bots/") => {
            let response = reqwest::Client::new()
                .get(format!("http://127.0.0.1:{port}{path}"))
                .header("accept-encoding", "identity")
                .send()
                .await
                .unwrap();
            (
                response.status().as_u16(),
                response.bytes().await.unwrap().to_vec(),
            )
        }
        _ => (404, Vec::new()),
    };
    let header = format!(
        "HTTP/1.1 {status} Test\r\nContent-Type: text/html\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    let _ = stream.write_all(header.as_bytes()).await;
    let _ = stream.write_all(&body).await;
}

#[tokio::test]
#[ignore = "requires MIASMA_BIN; launches a local Miasma instance and proxy"]
async fn avoids_a_real_miasma_instance_with_robots_hidden_and_visible_links() {
    let instance = Instance::start().await;
    for (robots, hidden) in [(true, false), (false, true), (false, false)] {
        instance.site.robots.store(robots, Ordering::Relaxed);
        instance.site.hidden.store(hidden, Ordering::Relaxed);
        instance.site.requests.lock().unwrap().clear();
        let frontier = Arc::new(
            InMemFrontier::fifo()
                .dedup()
                .avoid_traps_with(TrapPolicy::default().with_max_urls(8)),
        );
        let backend =
            ReqwestBackend::from_builder(|| reqwest::Client::builder().no_proxy()).unwrap();
        let stats = Crawler::new(backend, Router::new().fallback(FollowLinks::new("page")))
            .with_frontier(frontier.clone())
            .with_concurrency(1)
            .with_max_requests(20)
            .with_time_limit(Duration::from_secs(10))
            .with_request_timeout(Duration::from_secs(3))
            .with_robots(Robots::new("Spire"))
            .with_seed(Seed::get(&instance.base).unwrap())
            .run()
            .await
            .unwrap();
        let hits = instance
            .site
            .requests
            .lock()
            .unwrap()
            .iter()
            .filter(|path| path.starts_with("/naughty-bots/"))
            .count();
        assert_eq!(stats.failed, 0, "{stats:?}");
        if robots || hidden {
            assert_eq!(hits, 0, "robots={robots}, hidden={hidden}");
        } else {
            assert!(hits > 0 && hits <= 8, "{hits} trap requests");
            assert!(frontier.refused() > 0);
        }
    }
}
