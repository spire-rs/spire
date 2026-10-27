//! The browser profiles of identities: in each browser, one user context per
//! identity, with its proxy, user agent and headers, kept while the identity
//! is in use.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, PoisonError};

use futures::StreamExt;
use serde_json::{Value, json};
use spire_core::http::http::Uri;
use spire_core::http::http::header::USER_AGENT;
use spire_core::identity::{Identity, WeakIdentity};
use spire_core::{Error, Result};
use thirtyfour::bidi::modules::browser::CreateUserContext;
use thirtyfour::bidi::modules::emulation::SetUserAgentOverride;
use thirtyfour::bidi::modules::network::{AddIntercept, InterceptPhase};
use thirtyfour::bidi::modules::session::Subscribe;
use thirtyfour::bidi::{BiDi, BrowsingContextId, UserContextId};
use tokio::task::JoinHandle;

use crate::error::from_bidi;

/// The user context every browser starts with; it cannot be removed.
pub(crate) const DEFAULT_USER_CONTEXT: &str = "default";

/// Credentials a proxy asks for, from the userinfo of its URL.
#[derive(Clone)]
struct Credentials {
    username: String,
    password: String,
}

/// The profile of an identity in one browser.
struct Profile {
    identity: WeakIdentity,
    context: UserContextId,
    credentials: Option<Credentials>,
}

/// The identities' profiles in one browser, and the tabs requests use.
pub(crate) struct Profiles {
    /// The profile of each identity, by name.
    by_identity: Mutex<HashMap<Box<str>, Profile>>,
    /// The tabs opened for requests, with the credentials their proxy is
    /// answered with.
    tabs: Arc<Mutex<HashMap<String, Option<Credentials>>>>,
    /// Answers proxies asking for credentials, once a profile has some.
    auth: Mutex<Option<JoinHandle<()>>>,
}

impl Profiles {
    pub(crate) fn new() -> Self {
        Self {
            by_identity: Mutex::default(),
            tabs: Arc::default(),
            auth: Mutex::new(None),
        }
    }

    /// Returns the user context of `identity`, creating it on first use.
    ///
    /// # Errors
    ///
    /// Returns an [`Invalid`](spire_core::ErrorKind::Invalid) error if the
    /// proxy is not supported, by the backend or the browser, or an error
    /// if the browser fails.
    pub(crate) async fn context_of(
        &self,
        bidi: &BiDi,
        identity: &Identity,
    ) -> Result<UserContextId> {
        if let Some(profile) = lock(&self.by_identity).get(identity.name())
            && profile.identity.refers_to(identity)
        {
            return Ok(profile.context.clone());
        }

        let (proxy, credentials) = match identity.proxy() {
            Some(proxy) => {
                let (config, credentials) = proxy_config(identity, proxy)?;
                (Some(config), credentials)
            }
            None => (None, None),
        };
        let created = bidi.send(CreateUserContext {
            proxy,
            ..CreateUserContext::default()
        });
        let context = created.await.map_err(from_bidi)?.user_context;
        configure(bidi, identity, &context).await?;
        if credentials.is_some() {
            self.answer_proxies(bidi).await?;
        }

        tracing::debug!(target: crate::TRACING_TARGET, identity = identity.name(), "browser profile created");
        let profile = Profile {
            identity: identity.downgrade(),
            context: context.clone(),
            credentials,
        };
        lock(&self.by_identity).insert(identity.name().into(), profile);
        Ok(context)
    }

    /// Records a tab opened for a request going out as `identity`, so its
    /// proxy is answered with the identity's credentials, and the tab is
    /// closed when the browser is reset.
    pub(crate) fn opened(&self, tab: &BrowsingContextId, identity: &Identity) {
        let credentials = lock(&self.by_identity)
            .get(identity.name())
            .and_then(|profile| profile.credentials.clone());
        lock(&self.tabs).insert(tab.as_str().to_owned(), credentials);
    }

    /// Resets the browser between requests: closes the tabs of requests, and
    /// removes every user context but the default one and those of
    /// identities still in use.
    pub(crate) async fn reset(&self, bidi: &BiDi) -> Result<()> {
        let tabs: Vec<String> = lock(&self.tabs).drain().map(|(tab, _)| tab).collect();
        for tab in tabs {
            // Best effort: a tab whose user context is removed below closes
            // with it anyway.
            let _ = bidi
                .send_raw("browsingContext.close", json!({ "context": tab }))
                .await;
        }

        let kept: Vec<String> = {
            let mut profiles = lock(&self.by_identity);
            profiles.retain(|_, profile| profile.identity.is_alive());
            profiles
                .values()
                .map(|profile| profile.context.as_str().to_owned())
                .collect()
        };
        let browser = bidi.browser();
        let contexts = browser.get_user_contexts().await.map_err(from_bidi)?;
        for context in contexts.user_contexts {
            let id = context.user_context.as_str();
            if id != DEFAULT_USER_CONTEXT && !kept.iter().any(|kept| kept == id) {
                browser
                    .remove_user_context(context.user_context)
                    .await
                    .map_err(from_bidi)?;
            }
        }
        Ok(())
    }

    /// Answers proxies asking for credentials, from now on: an intercept
    /// pauses every request challenged for them, and a task answers with
    /// the credentials of the identity whose tab sent it, or lets the
    /// browser handle any other challenge as it would.
    async fn answer_proxies(&self, bidi: &BiDi) -> Result<()> {
        if lock(&self.auth).is_some() {
            return Ok(());
        }
        // The stream is opened before subscribing, so no event is missed.
        let mut events = bidi.subscribe_raw();
        bidi.send(Subscribe {
            events: vec!["network.authRequired".to_owned()],
            contexts: Vec::new(),
            user_contexts: Vec::new(),
        })
        .await
        .map_err(from_bidi)?;
        bidi.send(AddIntercept {
            phases: vec![InterceptPhase::AuthRequired],
            contexts: Vec::new(),
            url_patterns: None,
        })
        .await
        .map_err(from_bidi)?;

        let bidi = bidi.clone();
        let tabs = Arc::clone(&self.tabs);
        let task = tokio::spawn(async move {
            while let Some(event) = events.next().await {
                if event.method != "network.authRequired" {
                    continue;
                }
                let Some(request) = event.params["request"]["request"].as_str() else {
                    continue;
                };
                let context = event.params["context"].as_str().unwrap_or_default();
                let credentials = lock(&tabs).get(context).cloned().flatten();
                // thirtyfour's typed command names the action in lowercase,
                // which drivers reject, so it is sent as is.
                let answer = match credentials {
                    Some(Credentials { username, password }) => json!({
                        "request": request,
                        "action": "provideCredentials",
                        "credentials": { "type": "password", "username": username, "password": password },
                    }),
                    None => json!({ "request": request, "action": "default" }),
                };
                if let Err(error) = bidi.send_raw("network.continueWithAuth", answer).await {
                    tracing::debug!(target: crate::TRACING_TARGET, %error, "failed to answer a proxy");
                }
            }
        });
        *lock(&self.auth) = Some(task);
        Ok(())
    }
}

impl Drop for Profiles {
    fn drop(&mut self) {
        if let Some(task) = self
            .auth
            .get_mut()
            .unwrap_or_else(PoisonError::into_inner)
            .take()
        {
            task.abort();
        }
    }
}

fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Sets the identity's user agent, and its other headers, on every request
/// from `context`, the page's scripts and images included.
async fn configure(bidi: &BiDi, identity: &Identity, context: &UserContextId) -> Result<()> {
    if let Some(agent) = identity.headers().get(USER_AGENT) {
        let agent = agent.to_str().map_err(|error| {
            let message = format!("user agent of identity `{}` is not text", identity.name());
            Error::invalid(message).with_source(error)
        })?;
        bidi.send(SetUserAgentOverride {
            user_agent: Some(agent.to_owned()),
            contexts: None,
            user_contexts: Some(vec![context.clone()]),
        })
        .await
        .map_err(from_bidi)?;
    }

    let headers: Vec<Value> = identity
        .headers()
        .iter()
        .filter(|(name, _)| **name != USER_AGENT)
        .filter_map(|(name, value)| {
            let value = value.to_str().ok()?;
            Some(json!({ "name": name.as_str(), "value": { "type": "string", "value": value } }))
        })
        .collect();
    if !headers.is_empty() {
        let params = json!({ "headers": headers, "userContexts": [context.as_str()] });
        bidi.send_raw("network.setExtraHeaders", params)
            .await
            .map_err(from_bidi)?;
    }
    Ok(())
}

/// The BiDi proxy configuration of `proxy`, and the credentials in its URL.
fn proxy_config(identity: &Identity, proxy: &Uri) -> Result<(Value, Option<Credentials>)> {
    let invalid =
        |reason: &str| Error::invalid(format!("proxy of identity `{}` {reason}", identity.name()));
    let authority = proxy.authority().ok_or_else(|| invalid("has no host"))?;
    let (userinfo, host) = match authority.as_str().rsplit_once('@') {
        Some((userinfo, host)) => (Some(userinfo), host),
        None => (None, authority.as_str()),
    };
    let credentials = userinfo.map(|userinfo| {
        let (username, password) = userinfo.split_once(':').unwrap_or((userinfo, ""));
        Credentials {
            username: percent_decode(username),
            password: percent_decode(password),
        }
    });

    let config = match proxy.scheme_str() {
        Some("http" | "https") => json!({
            "proxyType": "manual",
            "httpProxy": host,
            "sslProxy": host,
        }),
        Some("socks5" | "socks5h") if credentials.is_some() => {
            return Err(invalid(
                "is a SOCKS proxy with credentials, which browsers do not support",
            ));
        }
        Some("socks5" | "socks5h") => json!({
            "proxyType": "manual",
            "socksProxy": host,
            "socksVersion": 5,
        }),
        _ => return Err(invalid("is not an http, https or socks5 URL")),
    };
    Ok((config, credentials))
}

/// Decodes `%XX` escapes, keeping anything else as is.
fn percent_decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        let escaped = (bytes[index] == b'%')
            .then(|| bytes.get(index + 1..index + 3))
            .flatten()
            .and_then(|hex| u8::from_str_radix(std::str::from_utf8(hex).ok()?, 16).ok());
        if let Some(byte) = escaped {
            decoded.push(byte);
            index += 3;
        } else {
            decoded.push(bytes[index]);
            index += 1;
        }
    }
    String::from_utf8_lossy(&decoded).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config(proxy: &str) -> Result<(Value, Option<Credentials>)> {
        let identity = Identity::new("a").with_proxy(proxy).unwrap();
        proxy_config(&identity, identity.proxy().unwrap())
    }

    #[test]
    fn http_proxies_carry_their_credentials() {
        // Fake credentials, for the test.
        let (http, credentials) = config("http://user%40x:p%3Ass@proxy.test:8080").unwrap(); // trufflehog:ignore
        assert_eq!(http["httpProxy"], "proxy.test:8080");
        assert_eq!(http["sslProxy"], "proxy.test:8080");
        let credentials = credentials.unwrap();
        assert_eq!(
            (credentials.username.as_str(), credentials.password.as_str()),
            ("user@x", "p:ss")
        );
    }

    #[test]
    fn socks_proxies_are_supported_without_credentials() {
        let (socks, credentials) = config("socks5://proxy.test:1080").unwrap();
        assert_eq!(socks["socksProxy"], "proxy.test:1080");
        assert!(credentials.is_none());
        assert!(config("socks5://user:pass@proxy.test:1080").is_err());
    }

    #[test]
    fn percent_decoding_keeps_malformed_escapes() {
        assert_eq!(percent_decode("a%20b%zz%4"), "a b%zz%4");
    }
}
