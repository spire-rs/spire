//! Navigating a browsing context over WebDriver BiDi.
//!
//! The navigation's network events give the real status and headers of the
//! page, and an intercept on the page's own request sends the seed's method,
//! headers and body, which a plain navigation cannot.

use std::time::Duration;

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use futures::StreamExt;
use serde_json::{Value, json};
use spire_core::http::header::HeaderName;
use spire_core::http::{HeaderMap, Method, StatusCode, Uri};
use spire_core::{Error, ErrorKind, Result};
use thirtyfour::bidi::modules::browsing_context::{Navigate, ReadinessState};
use thirtyfour::bidi::modules::network::{
    AddIntercept, ContinueRequest, InterceptPhase, RemoveIntercept,
};
use thirtyfour::bidi::modules::session::{Subscribe, Unsubscribe};
use thirtyfour::bidi::{BiDi, BrowsingContextId, InterceptId, RawEvent, RawEventStream};
use tokio::time::{Instant, timeout, timeout_at};

use crate::error::from_bidi;

/// The network events a navigation is followed through.
const EVENTS: [&str; 3] = [
    "network.beforeRequestSent",
    "network.responseCompleted",
    "network.fetchError",
];

/// How long to wait for the page's response once the page has loaded.
const RESPONSE_GRACE: Duration = Duration::from_secs(2);

/// The response a navigation ended on, after redirects.
#[derive(Debug, Clone)]
pub struct Navigation {
    uri: Uri,
    status: StatusCode,
    headers: HeaderMap,
}

impl Navigation {
    /// Returns the URI of the page, after redirects.
    #[must_use]
    pub const fn uri(&self) -> &Uri {
        &self.uri
    }

    /// Returns the status of the page's response.
    #[must_use]
    pub const fn status(&self) -> StatusCode {
        self.status
    }

    /// Returns the headers of the page's response.
    #[must_use]
    pub const fn headers(&self) -> &HeaderMap {
        &self.headers
    }

    /// Parses a BiDi `network.ResponseData`.
    fn from_response(response: &Value) -> Option<Self> {
        let uri = response["url"].as_str()?.parse().ok()?;
        let status = u16::try_from(response["status"].as_u64()?).ok()?;
        let status = StatusCode::from_u16(status).ok()?;

        let mut headers = HeaderMap::new();
        for header in response["headers"].as_array().into_iter().flatten() {
            let name = header["name"].as_str().and_then(|name| name.parse().ok());
            let value = bytes_value(&header["value"]).and_then(|value| value.try_into().ok());
            if let (Some(name), Some(value)) = (name, value) {
                headers.append::<HeaderName>(name, value);
            }
        }

        Some(Self {
            uri,
            status,
            headers,
        })
    }
}

/// What a request should be sent as, if not as a plain `GET`.
pub(crate) struct Overrides {
    pub(crate) method: Method,
    pub(crate) headers: HeaderMap,
    pub(crate) body: Vec<u8>,
}

impl Overrides {
    fn is_needed(&self) -> bool {
        self.method != Method::GET || !self.headers.is_empty() || !self.body.is_empty()
    }

    /// Continues the paused page request with the method, headers and body.
    fn continue_request(&self, request: &Value) -> Option<ContinueRequest> {
        let id = request["request"].as_str()?;

        // The headers replace the browser's own, so the browser's are kept
        // unless the request sets the same name.
        let mut headers: Vec<Value> = request["headers"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|header| {
                header["name"]
                    .as_str()
                    .is_none_or(|name| !self.headers.contains_key(name))
            })
            .cloned()
            .collect();
        headers.extend(self.headers.iter().map(|(name, value)| {
            json!({ "name": name.as_str(), "value": to_bytes_value(value.as_bytes()) })
        }));

        Some(ContinueRequest {
            request: id.into(),
            body: (!self.body.is_empty()).then(|| to_bytes_value(&self.body)),
            cookies: None,
            headers: Some(headers),
            method: (self.method != Method::GET).then(|| self.method.to_string()),
            url: None,
        })
    }
}

/// Navigates `context` to `uri`, returning the response the page ended on.
pub(crate) async fn navigate(
    bidi: &BiDi,
    context: &BrowsingContextId,
    uri: &Uri,
    overrides: &Overrides,
    load_timeout: Duration,
) -> Result<Navigation> {
    let url = uri.to_string();
    // The stream is opened before subscribing, so no event is missed.
    let mut events = bidi.subscribe_raw();
    let subscription = bidi
        .send(Subscribe {
            events: EVENTS.map(String::from).to_vec(),
            contexts: vec![context.clone()],
            user_contexts: Vec::new(),
        })
        .await
        .map_err(from_bidi)?
        .subscription;

    let intercept = if overrides.is_needed() {
        let added = bidi.send(AddIntercept {
            phases: vec![InterceptPhase::BeforeRequestSent],
            contexts: vec![context.clone()],
            url_patterns: Some(vec![json!({ "type": "string", "pattern": url })]),
        });
        Some(added.await.map_err(from_bidi)?.intercept)
    } else {
        None
    };

    let mut tracker = Tracker {
        bidi,
        context: context.as_str(),
        overrides,
        intercept: intercept.as_ref(),
        navigation: None,
        responses: Vec::new(),
        fetch_error: None,
        overridden: false,
    };
    let result = tracker.follow(&url, &mut events, load_timeout).await;

    // Best effort: the session is reset between requests anyway.
    if let Some(intercept) = intercept {
        let _ = bidi.send(RemoveIntercept { intercept }).await;
    }
    let unsubscribe = match subscription {
        Some(id) => Unsubscribe {
            events: Vec::new(),
            subscriptions: vec![id],
            contexts: Vec::new(),
        },
        None => Unsubscribe {
            events: EVENTS.map(String::from).to_vec(),
            subscriptions: Vec::new(),
            contexts: vec![context.clone()],
        },
    };
    let _ = bidi.send(unsubscribe).await;

    result.map_err(|error| error.with_uri(uri.clone()))
}

/// Follows one navigation through its network events.
struct Tracker<'a> {
    bidi: &'a BiDi,
    context: &'a str,
    overrides: &'a Overrides,
    intercept: Option<&'a InterceptId>,
    /// The id of the navigation, from its first request.
    navigation: Option<String>,
    /// Every response of a navigation, in order: redirects, then the page.
    responses: Vec<(String, Value)>,
    /// A navigation's network failure, e.g. a DNS error.
    fetch_error: Option<(String, String)>,
    overridden: bool,
}

impl Tracker<'_> {
    async fn follow(
        &mut self,
        url: &str,
        events: &mut RawEventStream,
        load_timeout: Duration,
    ) -> Result<Navigation> {
        let navigate = self.bidi.send(Navigate {
            context: self.context.into(),
            url: url.to_owned(),
            wait: Some(ReadinessState::Complete),
        });
        let navigate = timeout(load_timeout, navigate);
        tokio::pin!(navigate);

        let navigated = loop {
            tokio::select! {
                navigated = &mut navigate => break navigated,
                Some(event) = events.next() => self.record(event).await?,
            }
        };

        let Ok(navigated) = navigated else {
            let message = format!("loading the page timed out after {load_timeout:?}");
            return Err(Error::new(ErrorKind::Timeout).with_message(message));
        };

        if let Ok(navigated) = &navigated
            && let Some(navigation) = &navigated.navigation
        {
            self.navigation = Some(navigation.to_string());
        }

        // The page's response may be reported just after the navigation
        // ends, whether the page loaded or failed on its status.
        let deadline = Instant::now() + RESPONSE_GRACE;
        while self.page().is_none() && self.fetch_error.is_none() {
            match timeout_at(deadline, events.next()).await {
                Ok(Some(event)) => self.record(event).await?,
                Ok(None) | Err(_) => break,
            }
        }

        match navigated {
            Ok(_) => self
                .page()
                .or_else(|| self.last_response())
                .ok_or_else(|| Error::backend("the browser reported no response for the page")),
            // Chrome fails navigations to error statuses with an empty body,
            // although the response arrived.
            Err(error) => match (self.last_response(), &self.fetch_error) {
                (Some(response), _) => Ok(response),
                (None, Some((_, text))) => {
                    let message = format!("failed to load the page: {text}");
                    Err(Error::new(ErrorKind::Http).with_message(message))
                }
                (None, None) => Err(from_bidi(error)),
            },
        }
    }

    /// Records an event of the context, continuing paused requests.
    async fn record(&mut self, event: RawEvent) -> Result<()> {
        let params = &event.params;
        if params["context"].as_str() != Some(self.context) {
            return Ok(());
        }
        let navigation = params["navigation"].as_str().map(str::to_owned);

        match event.method.as_str() {
            "network.beforeRequestSent" => {
                let first = params["redirectCount"].as_u64().unwrap_or(0) == 0;
                if first && self.navigation.is_none() {
                    self.navigation.clone_from(&navigation);
                }
                if params["isBlocked"].as_bool() == Some(true) {
                    self.resume(params, first && navigation.is_some()).await?;
                }
            }
            "network.responseCompleted" => {
                if let Some(navigation) = navigation {
                    self.responses
                        .push((navigation, params["response"].clone()));
                }
            }
            "network.fetchError" => {
                if let (Some(navigation), Some(text)) = (navigation, params["errorText"].as_str()) {
                    self.fetch_error = Some((navigation, text.to_owned()));
                }
            }
            _ => {}
        }
        Ok(())
    }

    /// Continues a paused request: the page's own with the overrides, any
    /// other one as is.
    async fn resume(&mut self, params: &Value, is_page: bool) -> Result<()> {
        let ours = self.intercept.is_some_and(|intercept| {
            params["intercepts"]
                .as_array()
                .is_some_and(|ids| ids.iter().any(|id| id.as_str() == Some(intercept.as_str())))
        });
        if !ours {
            return Ok(());
        }

        let request = &params["request"];
        let resumed = if is_page && !self.overridden {
            self.overridden = true;
            self.overrides.continue_request(request)
        } else {
            request["request"].as_str().map(|id| ContinueRequest {
                request: id.into(),
                body: None,
                cookies: None,
                headers: None,
                method: None,
                url: None,
            })
        };

        if let Some(resumed) = resumed {
            self.bidi.send(resumed).await.map_err(from_bidi)?;
        }
        Ok(())
    }

    /// The navigation's final, non-redirect response.
    fn page(&self) -> Option<Navigation> {
        let navigation = self.navigation.as_deref()?;
        let (_, response) = self.responses.iter().rev().find(|(id, response)| {
            id == navigation && !is_redirect(response["status"].as_u64().unwrap_or(0))
        })?;
        Navigation::from_response(response)
    }

    /// The navigation's last response, a redirect if nothing followed it.
    fn last_response(&self) -> Option<Navigation> {
        let navigation = self.navigation.as_deref()?;
        let mut responses = self.responses.iter().rev();
        let (_, response) = responses.find(|(id, _)| id == navigation)?;
        Navigation::from_response(response)
    }
}

const fn is_redirect(status: u64) -> bool {
    matches!(status, 301 | 302 | 303 | 307 | 308)
}

/// Decodes a BiDi `network.BytesValue`.
fn bytes_value(value: &Value) -> Option<Vec<u8>> {
    let data = value["value"].as_str()?;
    match value["type"].as_str()? {
        "string" => Some(data.as_bytes().to_vec()),
        "base64" => STANDARD.decode(data).ok(),
        _ => None,
    }
}

/// Encodes bytes as a BiDi `network.BytesValue`.
fn to_bytes_value(bytes: &[u8]) -> Value {
    match std::str::from_utf8(bytes) {
        Ok(text) => json!({ "type": "string", "value": text }),
        Err(_) => json!({ "type": "base64", "value": STANDARD.encode(bytes) }),
    }
}

#[cfg(test)]
mod tests {
    use spire_core::http::HeaderValue;

    use super::*;

    #[test]
    fn parses_responses() {
        let response = json!({
            "url": "https://example.com/page",
            "status": 404,
            "statusText": "Not Found",
            "headers": [
                { "name": "Content-Type", "value": { "type": "string", "value": "text/html" } },
                { "name": "Set-Cookie", "value": { "type": "string", "value": "a=1" } },
                { "name": "Set-Cookie", "value": { "type": "base64", "value": "Yj0y" } },
            ],
        });

        let navigation = Navigation::from_response(&response).unwrap();
        assert_eq!(navigation.uri(), "https://example.com/page");
        assert_eq!(navigation.status(), StatusCode::NOT_FOUND);
        assert_eq!(navigation.headers()["content-type"], "text/html");
        let cookies: Vec<_> = navigation.headers().get_all("set-cookie").iter().collect();
        assert_eq!(cookies, ["a=1", "b=2"]);
    }

    #[test]
    fn overrides_keep_browser_headers_unless_replaced() {
        let mut headers = HeaderMap::new();
        headers.insert("user-agent", HeaderValue::from_static("MyBot/1.0"));
        headers.insert("content-type", HeaderValue::from_static("application/json"));
        let overrides = Overrides {
            method: Method::POST,
            headers,
            body: b"{}".to_vec(),
        };

        let request = json!({
            "request": "7",
            "headers": [
                { "name": "User-Agent", "value": { "type": "string", "value": "Chrome" } },
                { "name": "Accept", "value": { "type": "string", "value": "*/*" } },
            ],
        });
        let resumed = overrides.continue_request(&request).unwrap();
        assert_eq!(resumed.method.as_deref(), Some("POST"));
        assert_eq!(
            resumed.body,
            Some(json!({ "type": "string", "value": "{}" }))
        );

        let names: Vec<_> = resumed
            .headers
            .unwrap()
            .iter()
            .map(|h| h["name"].clone())
            .collect();
        assert_eq!(names, ["Accept", "user-agent", "content-type"]);
    }

    #[test]
    fn plain_gets_need_no_overrides() {
        let overrides = Overrides {
            method: Method::GET,
            headers: HeaderMap::new(),
            body: Vec::new(),
        };
        assert!(!overrides.is_needed());
        assert_eq!(
            to_bytes_value(&[0xff]),
            json!({ "type": "base64", "value": "/w==" })
        );
    }
}
