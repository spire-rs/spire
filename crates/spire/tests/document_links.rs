//! The extractor's optional DOM capability, independent of a browser library.
#![cfg(feature = "html")]

use std::future::{Future, ready};

use bytes::Bytes;
use spire::backend::Session;
use spire::backend::document::{DocumentLink, DocumentLinks, LinkKind, Visibility};
use spire::extract::FromContext;
use spire::extract::html::Links;
use spire::frontier::seed::Seed;
use spire::http::{Request, Response};
use spire::{Context, Error, Result};

struct Dom {
    fail: bool,
}

impl Session for Dom {
    fn fetch(&self, _: &Request) -> impl Future<Output = Result<Response>> + Send {
        ready(Ok(Response::new(Bytes::from_static(
            b"<a href='/wrong'>wrong</a>",
        ))))
    }

    fn document(&self) -> Option<&dyn spire::backend::document::Document> {
        Some(self)
    }
}

impl spire::backend::document::Document for Dom {
    fn links<'a>(
        &'a self,
        _: Option<&'a str>,
    ) -> futures::future::BoxFuture<'a, Result<DocumentLinks>> {
        Box::pin(async move {
            if self.fail {
                return Err(Error::backend("capture failed"));
            }
            Ok(DocumentLinks {
                uri: "https://site.test/current/page".parse().unwrap(),
                base_uri: "https://site.test/docs/".to_owned(),
                links: [Visibility::Visible, Visibility::Hidden, Visibility::Unknown]
                    .into_iter()
                    .map(|visibility| DocumentLink {
                        href: "same#fragment".to_owned(),
                        rel: vec!["Next".to_owned()],
                        text: " two  words ".to_owned(),
                        kind: LinkKind::Anchor,
                        visibility,
                    })
                    .collect(),
            })
        })
    }
}

#[tokio::test]
async fn uses_one_capture_preserves_duplicate_observations_and_keeps_unknown_links() {
    let cx = Context::new(
        Seed::get("https://old.test/").unwrap().into_request(),
        Dom { fail: false },
    );
    let links = Links::from_context(&cx, &()).await.unwrap();
    assert_eq!(links.len(), 3);
    for link in &links {
        assert_eq!(link.uri(), "https://site.test/docs/same");
        assert_eq!(link.rel(), ["next"]);
        assert_eq!(link.text(), "two words");
    }
    let visible = links.visible().same_host();
    assert_eq!(visible.len(), 2);
    assert_eq!(
        visible.iter().next().unwrap().visibility(),
        Visibility::Visible
    );
    assert_eq!(
        visible.iter().last().unwrap().visibility(),
        Visibility::Unknown
    );
}

#[tokio::test]
async fn a_failed_supported_capture_is_an_error_instead_of_silent_html_fallback() {
    let cx = Context::new(
        Seed::get("https://site.test/").unwrap().into_request(),
        Dom { fail: true },
    );
    assert_eq!(
        Links::from_context(&cx, &()).await.unwrap_err().to_string(),
        "capture failed"
    );
}
