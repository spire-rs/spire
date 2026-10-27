//! Browser-computed link observations against a local site.
//! Set `SPIRE_WEBDRIVER` to run against a WebDriver server with BiDi.

use spire_core::backend::document::{LinkKind, Visibility};
use spire_core::backend::{Backend, Session};
use spire_core::frontier::seed::Seed;
use spire_thirtyfour::ThirtyfourBackend;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

const PAGE: &str = r##"<!doctype html><html><head>
<link rel="stylesheet" href="/style.css"><base href="/docs/">
<style>.inline-rule { display:none } .restored { visibility:visible }</style>
</head><body>
<a id="normal" href="same">normal</a>
<a id="hidden" class="external-rule" href="same">hidden duplicate</a>
<a id="inline" class="inline-rule" href="inline">inline rule</a>
<div class="external-rule"><a id="ancestor" href="ancestor">ancestor</a></div>
<a id="negative" href="negative" style="position:absolute;left:-9999px">negative</a>
<div style="width:0;height:0;overflow:hidden"><a id="zero" href="zero">zero</a></div>
<a id="clipped" href="clipped" style="clip-path:inset(50%)">clipped</a>
<a id="transparent" href="transparent" style="opacity:0">transparent</a>
<a id="below" href="below" style="position:absolute;top:4000px">below the fold</a>
<div style="visibility:hidden"><a id="restored" class="restored" href="restored">restored</a></div>
<a id="image" href="image" style="display:contents"><img alt="image" width="20" height="20" src="data:image/gif;base64,R0lGODlhAQABAAD/ACwAAAAAAQABAAACADs="></a>
<a id="aria" aria-hidden="true" tabindex="-1" href="aria">aria</a>
<map name="map"><area id="area" href="area" shape="rect" coords="0,0,20,20" alt="Map area"><area hidden href="hidden-area" coords="0,0,20,20"></map>
<img usemap="#map" width="20" height="20" src="data:image/gif;base64,R0lGODlhAQABAAD/ACwAAAAAAQABAAACADs=">
</body></html>"##;

#[tokio::test]
async fn captures_stylesheets_layout_duplicates_and_current_dom_without_mutation() {
    let Ok(remote) = std::env::var("SPIRE_WEBDRIVER") else {
        return;
    };
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let server = tokio::spawn(async move {
        while let Ok((mut stream, _)) = listener.accept().await {
            tokio::spawn(async move {
                let mut buffer = [0; 8192];
                let read = stream.read(&mut buffer).await.unwrap();
                let request = String::from_utf8_lossy(&buffer[..read]);
                let (content_type, body) = if request.starts_with("GET /style.css ") {
                    ("text/css", ".external-rule { display:none }")
                } else {
                    ("text/html", PAGE)
                };
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                let _ = stream.write_all(response.as_bytes()).await;
            });
        }
    });
    let backend = ThirtyfourBackend::builder()
        .remote(remote)
        .max_sessions(1)
        .build()
        .unwrap();
    let session = backend.connect().await.unwrap();
    assert!(session.document().unwrap().links(None).await.is_err());
    let response = session
        .fetch(&Seed::get(&base).unwrap().into_request())
        .await
        .unwrap();
    let before = session.page_source().await.unwrap();
    assert_eq!(String::from_utf8_lossy(response.body()), before);
    let document = session.document().unwrap();
    let snapshot = document.links(None).await.unwrap();
    assert_eq!(snapshot.base_uri, format!("{base}/docs/"));
    assert_eq!(snapshot.links.len(), 14);
    let observed: Vec<_> = snapshot.links.iter().map(|link| link.visibility).collect();
    assert_eq!(
        observed,
        [
            Visibility::Visible,
            Visibility::Hidden,
            Visibility::Hidden,
            Visibility::Hidden,
            Visibility::Hidden,
            Visibility::Hidden,
            Visibility::Hidden,
            Visibility::Hidden,
            Visibility::Visible,
            Visibility::Visible,
            Visibility::Visible,
            Visibility::Hidden,
            Visibility::Unknown,
            Visibility::Hidden,
        ]
    );
    assert_eq!(snapshot.links[0].href, snapshot.links[1].href);
    assert_eq!(snapshot.links[12].kind, LinkKind::Area);
    assert_eq!(session.page_source().await.unwrap(), before);
    let scroll = session
        .execute("return [scrollX, scrollY]", vec![])
        .await
        .unwrap();
    assert_eq!(scroll.json(), &serde_json::json!([0, 0]));
    let selected = document.links(Some("#normal, #below")).await.unwrap();
    assert_eq!(selected.links.len(), 2);
    assert!(document.links(Some("a[")).await.is_err());
    session.execute("document.getElementById('normal').className = 'external-rule'; document.getElementById('hidden').className = '';", vec![]).await.unwrap();
    let changed = document.links(Some("#normal, #hidden")).await.unwrap();
    assert_eq!(changed.links[0].visibility, Visibility::Hidden);
    assert_eq!(changed.links[1].visibility, Visibility::Visible);
    assert_ne!(session.page_source().await.unwrap(), before);
    assert_eq!(String::from_utf8_lossy(response.body()), before);
    drop(session);
    backend.close().await;
    server.abort();
}
