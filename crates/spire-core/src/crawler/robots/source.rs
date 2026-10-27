//! Fetching robots.txt with any backend.

use std::future::Future;
use std::sync::Arc;

use ::http::{StatusCode, Uri};
use futures::FutureExt;
use futures::future::BoxFuture;
use robotxt::{AccessResult, Robots};

use crate::backend::{Backend, Session};
use crate::crawler::connector::Connector;
use crate::frontier::seed::Seed;
use crate::http::Request;
use crate::{Error, ErrorKind, Result};

/// Fetches robots.txt files; type-erased so the backend may differ from the
/// crawl's.
pub(crate) trait RobotsSource: Send + Sync + 'static {
    /// Fetches `url` and parses it for `agent`.
    ///
    /// Returns an error only if robots.txt is unreachable, which RFC 9309
    /// answers by disallowing everything; every other outcome is already
    /// resolved into rules.
    fn fetch(&self, url: Uri, agent: Arc<str>) -> BoxFuture<'static, Result<Robots>>;
}

/// A [`RobotsSource`] that fetches with a [`Backend`], through a
/// [`Connector`]: as one of the crawl's identities, the one robots.txt's
/// host is kept on, so it does not go out from the crawler's own IP.
pub(crate) struct BackendSource<B>(Arc<Connector<B>>);

impl<B: Backend> BackendSource<B> {
    pub(crate) const fn new(connector: Arc<Connector<B>>) -> Self {
        Self(connector)
    }
}

impl<B: Backend> RobotsSource for BackendSource<B> {
    fn fetch(&self, url: Uri, agent: Arc<str>) -> BoxFuture<'static, Result<Robots>> {
        let connector = self.0.clone();
        async move {
            let seed = Seed::try_from(url)?;
            let mut request = seed.clone().into_request();
            let identity = connector.identify(&seed, &mut request)?;
            let download = download(connector.connect(identity.as_ref()), request).await;
            parse(download, &agent)
        }
        .boxed()
    }
}

/// Parses a downloaded robots.txt for `agent`.
fn parse(download: Download, agent: &str) -> Result<Robots> {
    match download {
        Download::Body(body) => Ok(Robots::from_access(AccessResult::Successful(&body), agent)),
        Download::Access(access) => Ok(Robots::from_access(access, agent)),
        Download::Unreachable(error) => Err(error),
    }
}

/// The outcome of downloading a robots.txt.
enum Download {
    Body(bytes::Bytes),
    /// Resolved without a body: unavailable or too many redirects.
    Access(AccessResult<'static>),
    Unreachable(Error),
}

async fn download<C: Session>(
    connect: impl Future<Output = Result<C>>,
    request: Request,
) -> Download {
    let client = match connect.await {
        Ok(client) => client,
        Err(error) => return classify_error(error),
    };
    let response = match client.fetch(&request).await {
        Ok(response) => response,
        Err(error) => return classify_error(error),
    };

    let status = response.status();
    if !status.is_success() {
        return classify_status(status).map_or_else(
            || {
                let message = format!("robots.txt responded with {status}");
                Download::Unreachable(Error::new(ErrorKind::Http).with_message(message))
            },
            Download::Access,
        );
    }

    Download::Body(response.into_body())
}

/// Classifies an error status per RFC 9309, or `None` if it means robots.txt
/// is unreachable. `429` counts as unreachable: it asks to slow down, not to
/// crawl everything.
fn classify_status(status: StatusCode) -> Option<AccessResult<'static>> {
    if status.is_redirection() {
        // The backend did not follow the redirects.
        Some(AccessResult::TooManyRedirects)
    } else if status.is_client_error() && status != StatusCode::TOO_MANY_REQUESTS {
        Some(AccessResult::Unavailable)
    } else {
        None
    }
}

/// Classifies a failed fetch, e.g. from a backend turning statuses into
/// errors, per RFC 9309.
fn classify_error(error: Error) -> Download {
    if let Some(status) = error.status() {
        return classify_status(status).map_or(Download::Unreachable(error), Download::Access);
    }
    if error.kind() == ErrorKind::Rejected {
        // A redirect loop or too many redirects.
        return Download::Access(AccessResult::TooManyRedirects);
    }
    Download::Unreachable(error)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn access(download: &Download) -> Option<AccessResult<'static>> {
        match download {
            Download::Access(access) => Some(*access),
            Download::Body(_) | Download::Unreachable(_) => None,
        }
    }

    #[test]
    fn classifies_statuses() {
        let status = |code| classify_status(StatusCode::from_u16(code).unwrap());
        assert_eq!(status(404), Some(AccessResult::Unavailable));
        assert_eq!(status(403), Some(AccessResult::Unavailable));
        assert_eq!(status(301), Some(AccessResult::TooManyRedirects));
        assert_eq!(status(429), None);
        assert_eq!(status(500), None);
        assert_eq!(status(503), None);
    }

    #[test]
    fn classifies_errors() {
        let gone = Error::new(ErrorKind::Rejected).with_status(StatusCode::NOT_FOUND);
        assert_eq!(
            access(&classify_error(gone)),
            Some(AccessResult::Unavailable)
        );

        let limited = Error::new(ErrorKind::Http).with_status(StatusCode::TOO_MANY_REQUESTS);
        assert!(matches!(classify_error(limited), Download::Unreachable(_)));

        let looping = Error::new(ErrorKind::Rejected);
        assert_eq!(
            access(&classify_error(looping)),
            Some(AccessResult::TooManyRedirects)
        );

        let offline = Error::new(ErrorKind::Http);
        assert!(matches!(classify_error(offline), Download::Unreachable(_)));
    }
}
