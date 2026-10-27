use crate::backend::Session;
use crate::extract::FromContext;
use crate::http::ResponseExt;
use crate::http::http::{HeaderMap, StatusCode, Uri};
use crate::{Context, Error, Result};

/// Extracts the head of the response: its status, headers, and the URI it
/// was served from.
///
/// Error statuses fail the request before the handler runs, unless the
/// backend's [`StatusPolicy`](crate::backend::StatusPolicy) accepts them, so
/// `Head` is mostly useful for headers, redirects, and statuses a handler
/// decides about itself.
///
/// ```
/// use spire::extract::Head;
/// use spire::http::http::header::LAST_MODIFIED;
///
/// async fn page(head: Head) {
///     println!("{} from {}", head.status, head.uri);
///     if let Some(modified) = head.headers.get(LAST_MODIFIED) {
///         println!("last modified {modified:?}");
///     }
/// }
/// ```
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct Head {
    /// The status of the response.
    pub status: StatusCode,
    /// The headers of the response.
    pub headers: HeaderMap,
    /// The URI the response was served from, after redirects: the request's
    /// URI if the backend does not report another.
    pub uri: Uri,
}

impl<C, S> FromContext<C, S> for Head
where
    C: Session,
    S: Sync,
{
    type Rejection = Error;

    async fn from_context(cx: &Context<C>, _: &S) -> Result<Self> {
        let response = cx.response().await?;
        let uri = response
            .final_uri()
            .unwrap_or_else(|| cx.request().uri())
            .clone();
        Ok(Self {
            status: response.status(),
            headers: response.headers().clone(),
            uri,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frontier::seed::Seed;
    use crate::http::http::HeaderValue;
    use crate::http::http::header::ETAG;
    use crate::testing::{MockPage, MockSession, MockSite};
    use crate::worker::{Outcome, from_fn};

    #[tokio::test]
    async fn reads_the_status_headers_and_final_uri() {
        let page = MockPage::text("").with_header(ETAG, HeaderValue::from_static("\"v1\""));
        let site = MockSite::new()
            .page(
                "https://example.com/old",
                MockPage::redirect("https://example.com/new"),
            )
            .page("https://example.com/new", page);

        let worker = from_fn(|cx: Context<MockSession>| async move {
            let head = Head::from_context(&cx, &()).await?;
            assert_eq!(head.status, StatusCode::OK);
            assert_eq!(head.headers[ETAG], "\"v1\"");
            assert_eq!(head.uri, "https://example.com/new");
            Ok::<_, Error>(())
        });
        let visit = site.visit(Seed::get("https://example.com/old").unwrap());
        assert!(matches!(visit.run(&worker).await, Outcome::Continue));
    }
}
