//! Classification of WebDriver failures.

use spire_core::{Error, ErrorKind};
use thirtyfour::error::{WebDriverError, WebDriverErrorInner as Inner};

/// Converts a WebDriver failure into an [`Error`], so the crawler retries
/// only what may succeed on another attempt.
pub(crate) fn from_webdriver(error: WebDriverError) -> Error {
    let kind = match error.as_inner() {
        Inner::Timeout(_) | Inner::WebDriverTimeout(_) | Inner::ScriptTimeout(_) => {
            ErrorKind::Timeout
        }
        Inner::InvalidUrl(_) | Inner::InvalidArgument(_) => ErrorKind::Invalid,
        // A bad certificate stays bad on the next attempt.
        Inner::InsecureCertificate(_) => ErrorKind::Rejected,
        // The session or the driver is gone; a fresh session may work.
        Inner::InvalidSessionId(_)
        | Inner::SessionNotCreated(_)
        | Inner::SessionCreateError(_)
        | Inner::NoSuchWindow(_)
        | Inner::RequestFailed(_)
        | Inner::HttpError(_)
        | Inner::FatalError(_) => ErrorKind::Backend,
        // Page interaction failures are bugs or page changes, not transient.
        Inner::JavascriptError(_)
        | Inner::NoSuchElement(_)
        | Inner::NoSuchFrame(_)
        | Inner::NoSuchAlert(_)
        | Inner::StaleElementReference(_)
        | Inner::ElementNotInteractable(_)
        | Inner::ElementClickIntercepted(_)
        | Inner::InvalidSelector(_)
        | Inner::InvalidElementState(_) => ErrorKind::Worker,
        // Navigation failures such as DNS or connection errors surface as
        // unknown errors and are usually transient.
        _ => ErrorKind::Http,
    };

    Error::new(kind).with_source(error)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_timeouts_and_navigation_failures() {
        let timeout = WebDriverError::from_inner(Inner::Timeout("page load".to_owned()));
        assert_eq!(from_webdriver(timeout).kind(), ErrorKind::Timeout);

        let unknown = WebDriverError::from_inner(Inner::ParseError("garbled".to_owned()));
        assert_eq!(from_webdriver(unknown).kind(), ErrorKind::Http);

        let failed = WebDriverError::from_inner(Inner::RequestFailed("refused".to_owned()));
        assert_eq!(from_webdriver(failed).kind(), ErrorKind::Backend);
    }
}
