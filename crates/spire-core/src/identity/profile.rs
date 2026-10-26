//! Browser profiles: a user agent, and the headers a real browser sends with
//! it, for an [`Identity`](super::Identity).

use std::fmt;
use std::ops::RangeInclusive;

use http::header::{ACCEPT, ACCEPT_LANGUAGE, HeaderName, UPGRADE_INSECURE_REQUESTS, USER_AGENT};
use http::{HeaderMap, HeaderValue};

use crate::{Error, Result};

/// A browser a [`Profile`] presents as.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Browser {
    /// Google Chrome.
    Chrome,
    /// Microsoft Edge.
    Edge,
    /// Mozilla Firefox.
    Firefox,
    /// Apple Safari.
    Safari,
}

impl Browser {
    /// The major version current when this release of spire was made.
    #[must_use]
    pub const fn latest(self) -> u32 {
        match self {
            Self::Chrome | Self::Edge => 156,
            Self::Firefox => 154,
            Self::Safari => 26,
        }
    }

    /// The platforms the browser runs on.
    #[must_use]
    pub const fn platforms(self) -> &'static [Platform] {
        match self {
            Self::Chrome | Self::Firefox => &[
                Platform::Windows,
                Platform::MacOs,
                Platform::Linux,
                Platform::Android,
            ],
            Self::Edge => &[Platform::Windows, Platform::MacOs],
            Self::Safari => &[Platform::MacOs, Platform::Ios],
        }
    }

    /// Whether the browser is built on Chromium, and sends client hints.
    const fn is_chromium(self) -> bool {
        matches!(self, Self::Chrome | Self::Edge)
    }
}

/// The platform a [`Profile`] runs on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Platform {
    /// Windows 10 or 11, 64-bit.
    Windows,
    /// macOS.
    MacOs,
    /// Desktop Linux, 64-bit.
    Linux,
    /// An Android phone.
    Android,
    /// An iPhone.
    Ios,
}

impl Platform {
    /// The desktop platforms.
    pub const DESKTOP: [Self; 3] = [Self::Windows, Self::MacOs, Self::Linux];

    const fn is_mobile(self) -> bool {
        matches!(self, Self::Android | Self::Ios)
    }

    /// The platform as Chromium's `Sec-CH-UA-Platform` names it.
    const fn client_hint(self) -> &'static str {
        match self {
            Self::Windows => "\"Windows\"",
            Self::MacOs => "\"macOS\"",
            Self::Linux => "\"Linux\"",
            Self::Android => "\"Android\"",
            Self::Ios => "\"iOS\"",
        }
    }
}

/// A user agent, and the headers a real browser sends with it on a
/// navigation: client hints for Chromium browsers, `Accept`,
/// `Accept-Language`, `Upgrade-Insecure-Requests` and the `Sec-Fetch-*`
/// headers of a page opened directly. Give it to an identity with
/// [`Identity::with_profile`](super::Identity::with_profile), for an HTTP
/// backend: a real browser sends its own.
///
/// `Accept-Encoding` is left to the backend, which knows which encodings it
/// can decode. A backend cannot match a browser's header order or TLS
/// fingerprint, and a real browser sends its own client hints, so profiles
/// suit HTTP backends; see [`UserAgents`] for picking them.
///
/// ```
/// use spire_core::identity::{Browser, Platform, Profile};
///
/// let profile = Profile::new(Browser::Chrome, 156, Platform::Windows)?;
/// assert_eq!(
///     profile.user_agent(),
///     "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/156.0.0.0 Safari/537.36",
/// );
/// # Ok::<_, spire_core::Error>(())
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Profile {
    browser: Browser,
    version: u32,
    platform: Platform,
    language: HeaderValue,
}

impl Profile {
    /// The `Accept-Language` of a profile by default.
    const LANGUAGE: &'static str = "en-US,en;q=0.9";

    /// Creates the profile of `browser` at major `version` on `platform`.
    ///
    /// # Errors
    ///
    /// Returns an [`Invalid`](crate::ErrorKind::Invalid) error if the
    /// browser does not run on the platform, or `version` is zero.
    pub fn new(browser: Browser, version: u32, platform: Platform) -> Result<Self> {
        if !browser.platforms().contains(&platform) {
            return Err(Error::invalid(format!(
                "{browser:?} does not run on {platform:?}"
            )));
        }
        if version == 0 {
            return Err(Error::invalid("a browser version must be above zero"));
        }
        Ok(Self {
            browser,
            version,
            platform,
            language: HeaderValue::from_static(Self::LANGUAGE),
        })
    }

    /// Sends `language` as `Accept-Language`, `en-US,en;q=0.9` by default,
    /// e.g. to match where a proxy exits.
    #[must_use]
    pub fn with_language(mut self, language: HeaderValue) -> Self {
        self.language = language;
        self
    }

    /// Returns the browser.
    #[must_use]
    pub const fn browser(&self) -> Browser {
        self.browser
    }

    /// Returns the major version.
    #[must_use]
    pub const fn version(&self) -> u32 {
        self.version
    }

    /// Returns the platform.
    #[must_use]
    pub const fn platform(&self) -> Platform {
        self.platform
    }

    /// Returns the `User-Agent`, as the browser reduces it: only the major
    /// version and a fixed description of the platform.
    #[must_use]
    pub fn user_agent(&self) -> String {
        let version = self.version;
        let chromium = |platform: &str, safari: &str| {
            format!(
                "Mozilla/5.0 ({platform}) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/{version}.0.0.0 {safari}"
            )
        };
        let chromium_platform = match self.platform {
            Platform::Windows => "Windows NT 10.0; Win64; x64",
            Platform::MacOs => "Macintosh; Intel Mac OS X 10_15_7",
            Platform::Linux => "X11; Linux x86_64",
            Platform::Android => "Linux; Android 10; K",
            Platform::Ios => "iPhone; CPU iPhone OS 18_6 like Mac OS X",
        };
        let chromium_safari = if self.platform.is_mobile() {
            "Mobile Safari/537.36"
        } else {
            "Safari/537.36"
        };

        match self.browser {
            Browser::Chrome => chromium(chromium_platform, chromium_safari),
            Browser::Edge => format!(
                "{} Edg/{version}.0.0.0",
                chromium(chromium_platform, chromium_safari)
            ),
            Browser::Firefox => match self.platform {
                Platform::Android => {
                    format!(
                        "Mozilla/5.0 (Android 15; Mobile; rv:{version}.0) Gecko/{version}.0 Firefox/{version}.0"
                    )
                }
                platform => {
                    let platform = match platform {
                        Platform::Windows => "Windows NT 10.0; Win64; x64",
                        Platform::MacOs => "Macintosh; Intel Mac OS X 10.15",
                        _ => "X11; Linux x86_64",
                    };
                    format!(
                        "Mozilla/5.0 ({platform}; rv:{version}.0) Gecko/20100101 Firefox/{version}.0"
                    )
                }
            },
            Browser::Safari => match self.platform {
                Platform::Ios => format!(
                    "Mozilla/5.0 (iPhone; CPU iPhone OS 18_6 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/{version}.0 Mobile/15E148 Safari/604.1"
                ),
                _ => format!(
                    "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/{version}.0 Safari/605.1.15"
                ),
            },
        }
    }

    /// Returns Chromium's `Sec-CH-UA`: the browser, Chromium and a made-up
    /// brand, ordered and named by the major version, as Chromium derives
    /// them.
    fn brands(&self) -> String {
        const GREASE_CHARS: [&str; 11] = [" ", "(", ":", "-", ".", "/", ")", ";", "=", "?", "_"];
        const GREASE_VERSIONS: [&str; 3] = ["8", "99", "24"];
        const ORDERS: [[usize; 3]; 6] = [
            [0, 1, 2],
            [0, 2, 1],
            [1, 0, 2],
            [1, 2, 0],
            [2, 0, 1],
            [2, 1, 0],
        ];

        let seed = self.version as usize;
        let grease = format!(
            "\"Not{}A{}Brand\";v=\"{}\"",
            GREASE_CHARS[seed % GREASE_CHARS.len()],
            GREASE_CHARS[(seed + 1) % GREASE_CHARS.len()],
            GREASE_VERSIONS[seed % GREASE_VERSIONS.len()],
        );
        let brand = match self.browser {
            Browser::Edge => "Microsoft Edge",
            _ => "Google Chrome",
        };
        let order = ORDERS[seed % ORDERS.len()];
        let mut brands = [String::new(), String::new(), String::new()];
        brands[order[0]] = grease;
        brands[order[1]] = format!("\"Chromium\";v=\"{}\"", self.version);
        brands[order[2]] = format!("\"{brand}\";v=\"{}\"", self.version);
        brands.join(", ")
    }

    /// Returns the headers to send: the user agent and those that go with
    /// it.
    #[must_use]
    pub fn headers(&self) -> HeaderMap {
        let mut headers = HeaderMap::new();
        let text = |value: String| {
            HeaderValue::try_from(value)
                .unwrap_or_else(|_| unreachable!("profile headers are ASCII"))
        };

        if self.browser.is_chromium() {
            headers.insert(HeaderName::from_static("sec-ch-ua"), text(self.brands()));
            let mobile = if self.platform.is_mobile() {
                "?1"
            } else {
                "?0"
            };
            headers.insert(
                HeaderName::from_static("sec-ch-ua-mobile"),
                HeaderValue::from_static(mobile),
            );
            headers.insert(
                HeaderName::from_static("sec-ch-ua-platform"),
                HeaderValue::from_static(self.platform.client_hint()),
            );
        }
        headers.insert(UPGRADE_INSECURE_REQUESTS, HeaderValue::from_static("1"));
        headers.insert(USER_AGENT, text(self.user_agent()));
        let accept = if self.browser.is_chromium() {
            "text/html,application/xhtml+xml,application/xml;q=0.9,image/jxl,image/avif,image/webp,image/apng,*/*;q=0.8,application/signed-exchange;v=b3;q=0.7"
        } else {
            "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8"
        };
        headers.insert(ACCEPT, HeaderValue::from_static(accept));
        for (name, value) in [
            ("sec-fetch-site", "none"),
            ("sec-fetch-mode", "navigate"),
            ("sec-fetch-user", "?1"),
            ("sec-fetch-dest", "document"),
        ] {
            headers.insert(
                HeaderName::from_static(name),
                HeaderValue::from_static(value),
            );
        }
        headers.insert(ACCEPT_LANGUAGE, self.language.clone());
        if self.browser == Browser::Firefox {
            headers.insert(
                HeaderName::from_static("priority"),
                HeaderValue::from_static("u=0, i"),
            );
        }
        headers
    }
}

/// Picks a [`Profile`] for each identity, with browsers and platforms as
/// common as they are on the web, and versions among the latest.
///
/// [`pick`](Self::pick) is deterministic: identity `n` gets the same
/// profile in every run, so a resumed crawl keeps each identity's profile.
///
/// ```
/// use spire_core::identity::{Browser, Identities, Identity, UserAgents};
/// use spire_core::http::HeaderValue;
///
/// let agents = UserAgents::new()
///     .browsers([Browser::Chrome, Browser::Firefox])
///     .language(HeaderValue::from_static("de-DE,de;q=0.9"));
///
/// let identities = Identities::from_fn(move |n| {
///     Ok(Identity::new(format!("session-{n}")).with_profile(agents.pick(n)))
/// });
/// ```
#[derive(Clone)]
pub struct UserAgents {
    browsers: Vec<(Browser, u32)>,
    platforms: Vec<Platform>,
    versions: Vec<(Browser, RangeInclusive<u32>)>,
    language: Option<HeaderValue>,
}

impl Default for UserAgents {
    /// Desktop Chrome, Safari, Edge and Firefox, at their latest two major
    /// versions, Safari at its latest, see [`UserAgents::new`].
    fn default() -> Self {
        Self {
            browsers: [
                Browser::Chrome,
                Browser::Safari,
                Browser::Edge,
                Browser::Firefox,
            ]
            .map(|browser| (browser, default_share(browser)))
            .to_vec(),
            platforms: Platform::DESKTOP.to_vec(),
            versions: Vec::new(),
            language: None,
        }
    }
}

impl UserAgents {
    /// Picks among desktop Chrome, Safari, Edge and Firefox, in proportion
    /// to their shares: 65, 18, 10 and 7, roughly how common each is. Each
    /// runs on its own mix of desktop platforms, at its latest two major
    /// versions, Safari at its latest.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Picks only among `browsers`, keeping their shares, so they stay as
    /// common relative to each other as on the web.
    ///
    /// # Panics
    ///
    /// Panics if none of `browsers` runs on the
    /// [platforms](Self::platforms).
    #[must_use]
    pub fn browsers(mut self, browsers: impl IntoIterator<Item = Browser>) -> Self {
        self.browsers = browsers
            .into_iter()
            .map(|browser| {
                let share = self
                    .browsers
                    .iter()
                    .find(|(other, _)| *other == browser)
                    .map_or_else(|| default_share(browser), |(_, share)| *share);
                (browser, share)
            })
            .collect();
        self.check();
        self
    }

    /// Picks `browser` in proportion to `share` among the others' shares,
    /// adding it if it was left out; a share of zero leaves it out.
    ///
    /// # Panics
    ///
    /// Panics if no browser left runs on the [platforms](Self::platforms).
    #[must_use]
    pub fn share(mut self, browser: Browser, share: u32) -> Self {
        self.browsers.retain(|(other, _)| *other != browser);
        if share > 0 {
            self.browsers.push((browser, share));
        }
        self.check();
        self
    }

    /// Picks among `platforms`, the desktop ones by default; each browser
    /// runs on those of them it supports.
    ///
    /// # Panics
    ///
    /// Panics if none of the browsers runs on any of `platforms`.
    #[must_use]
    pub fn platforms(mut self, platforms: impl IntoIterator<Item = Platform>) -> Self {
        self.platforms = platforms.into_iter().collect();
        self.check();
        self
    }

    /// Picks `browser`'s major version among `versions`, instead of its
    /// latest.
    ///
    /// # Panics
    ///
    /// Panics if `versions` is empty or starts at zero.
    #[must_use]
    pub fn versions(mut self, browser: Browser, versions: RangeInclusive<u32>) -> Self {
        assert!(
            !versions.is_empty() && *versions.start() > 0,
            "`UserAgents::versions` needs versions above zero"
        );
        self.versions.retain(|(other, _)| *other != browser);
        self.versions.push((browser, versions));
        self
    }

    /// Sends `language` as every profile's `Accept-Language`.
    #[must_use]
    pub fn language(mut self, language: HeaderValue) -> Self {
        self.language = Some(language);
        self
    }

    fn check(&self) {
        assert!(
            self.browsers.iter().any(|(browser, _)| {
                browser
                    .platforms()
                    .iter()
                    .any(|platform| self.platforms.contains(platform))
            }),
            "`UserAgents` has no browser that runs on its platforms"
        );
    }

    /// Returns the profile of identity `n`.
    #[must_use]
    pub fn pick(&self, n: u64) -> Profile {
        let mut draws = Draws(n);
        let browsers: Vec<(Browser, u32)> = self
            .browsers
            .iter()
            .copied()
            .filter(|(browser, _)| {
                browser
                    .platforms()
                    .iter()
                    .any(|platform| self.platforms.contains(platform))
            })
            .collect();
        let browser = weighted(&browsers, draws.next());

        let platforms: Vec<(Platform, u32)> = browser
            .platforms()
            .iter()
            .copied()
            .filter(|platform| self.platforms.contains(platform))
            .map(|platform| (platform, share(browser, platform)))
            .collect();
        let platform = weighted(&platforms, draws.next());

        let versions = self
            .versions
            .iter()
            .find(|(other, _)| *other == browser)
            .map_or_else(
                || default_versions(browser),
                |(_, versions)| versions.clone(),
            );
        let span = u64::from(versions.end() - versions.start()) + 1;
        let offset = u32::try_from(draws.next() % span).unwrap_or_default();
        let version = versions.start() + offset;

        let profile = Profile::new(browser, version, platform)
            .unwrap_or_else(|_| unreachable!("browsers are only picked on their platforms"));
        match &self.language {
            Some(language) => profile.with_language(language.clone()),
            None => profile,
        }
    }
}

impl fmt::Debug for UserAgents {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("UserAgents")
            .field("browsers", &self.browsers)
            .field("platforms", &self.platforms)
            .field("versions", &self.versions)
            .finish_non_exhaustive()
    }
}

/// How common `browser` is, relative to the others.
const fn default_share(browser: Browser) -> u32 {
    match browser {
        Browser::Chrome => 65,
        Browser::Safari => 18,
        Browser::Edge => 10,
        Browser::Firefox => 7,
    }
}

/// A browser's latest two major versions, Safari's latest.
fn default_versions(browser: Browser) -> RangeInclusive<u32> {
    let latest = browser.latest();
    match browser {
        Browser::Safari => latest..=latest,
        _ => latest - 1..=latest,
    }
}

/// How common `platform` is among `browser`'s users.
const fn share(browser: Browser, platform: Platform) -> u32 {
    match (browser, platform) {
        (Browser::Chrome | Browser::Firefox, Platform::Windows) => 70,
        (Browser::Chrome, Platform::MacOs) => 22,
        (Browser::Chrome, Platform::Linux) => 8,
        (Browser::Firefox, Platform::MacOs | Platform::Linux) => 15,
        (Browser::Edge, Platform::Windows) => 90,
        (Browser::Edge, Platform::MacOs) => 10,
        _ => 100,
    }
}

/// Picks among `choices` in proportion to their weights, by `draw`.
fn weighted<T: Copy>(choices: &[(T, u32)], draw: u64) -> T {
    let total: u64 = choices.iter().map(|(_, weight)| u64::from(*weight)).sum();
    let mut point = draw % total.max(1);
    for (choice, weight) in choices {
        let weight = u64::from(*weight);
        if point < weight {
            return *choice;
        }
        point -= weight;
    }
    choices[0].0
}

/// Numbers drawn from `n`, the same in every run: splitmix64.
struct Draws(u64);

impl Draws {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn header(headers: &HeaderMap, name: &str) -> String {
        headers[name].to_str().unwrap().to_owned()
    }

    /// The headers Chrome 156 sent on macOS, captured from the browser.
    #[test]
    fn chrome_matches_the_real_browser() {
        let headers = Profile::new(Browser::Chrome, 156, Platform::MacOs)
            .unwrap()
            .headers();
        assert_eq!(
            header(&headers, "sec-ch-ua"),
            r#""Not:A-Brand";v="8", "Chromium";v="156", "Google Chrome";v="156""#
        );
        assert_eq!(header(&headers, "sec-ch-ua-mobile"), "?0");
        assert_eq!(header(&headers, "sec-ch-ua-platform"), "\"macOS\"");
        assert_eq!(
            header(&headers, "user-agent"),
            "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/156.0.0.0 Safari/537.36"
        );
        assert_eq!(
            header(&headers, "accept"),
            "text/html,application/xhtml+xml,application/xml;q=0.9,image/jxl,image/avif,image/webp,image/apng,*/*;q=0.8,application/signed-exchange;v=b3;q=0.7"
        );
        assert_eq!(header(&headers, "upgrade-insecure-requests"), "1");
        assert_eq!(header(&headers, "sec-fetch-mode"), "navigate");
        assert!(!headers.contains_key("accept-encoding"));
    }

    /// Chrome 141's brands, as Chrome documented them.
    #[test]
    fn chrome_brands_follow_the_version() {
        let profile = Profile::new(Browser::Chrome, 141, Platform::Windows).unwrap();
        assert_eq!(
            profile.brands(),
            r#""Google Chrome";v="141", "Not?A_Brand";v="8", "Chromium";v="141""#
        );
        let edge = Profile::new(Browser::Edge, 141, Platform::Windows).unwrap();
        assert!(edge.brands().contains(r#""Microsoft Edge";v="141""#));
        assert!(edge.user_agent().ends_with(" Edg/141.0.0.0"));
    }

    /// The headers Firefox 154 sent on macOS, captured from the browser.
    #[test]
    fn firefox_matches_the_real_browser() {
        let headers = Profile::new(Browser::Firefox, 154, Platform::MacOs)
            .unwrap()
            .headers();
        assert_eq!(
            header(&headers, "user-agent"),
            "Mozilla/5.0 (Macintosh; Intel Mac OS X 10.15; rv:154.0) Gecko/20100101 Firefox/154.0"
        );
        assert_eq!(
            header(&headers, "accept"),
            "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8"
        );
        assert_eq!(header(&headers, "accept-language"), "en-US,en;q=0.9");
        assert_eq!(header(&headers, "priority"), "u=0, i");
        assert!(!headers.contains_key("sec-ch-ua"));
    }

    #[test]
    fn browsers_only_run_on_their_platforms() {
        assert!(Profile::new(Browser::Safari, 26, Platform::Windows).is_err());
        assert!(Profile::new(Browser::Edge, 156, Platform::Linux).is_err());
        assert!(Profile::new(Browser::Chrome, 0, Platform::Linux).is_err());
        let mobile = Profile::new(Browser::Chrome, 156, Platform::Android).unwrap();
        assert!(mobile.user_agent().contains("Mobile Safari"));
        assert_eq!(header(&mobile.headers(), "sec-ch-ua-mobile"), "?1");
    }

    #[test]
    fn picks_are_deterministic_and_follow_the_weights() {
        let agents = UserAgents::new();
        assert_eq!(agents.pick(7), agents.pick(7));

        let picks: Vec<Profile> = (0..1000).map(|n| agents.pick(n)).collect();
        let chrome = picks
            .iter()
            .filter(|profile| profile.browser() == Browser::Chrome)
            .count();
        assert!((550..750).contains(&chrome), "{chrome} of 1000 are Chrome");
        assert!(picks.iter().all(|profile| {
            Platform::DESKTOP.contains(&profile.platform())
                && default_versions(profile.browser()).contains(&profile.version())
        }));
    }

    #[test]
    fn picks_respect_the_settings() {
        let agents = UserAgents::new()
            .browsers([Browser::Safari])
            .platforms([Platform::Ios])
            .versions(Browser::Safari, 25..=25)
            .language(HeaderValue::from_static("fr-FR"));
        let profile = agents.pick(3);
        assert_eq!(
            (profile.browser(), profile.platform(), profile.version()),
            (Browser::Safari, Platform::Ios, 25)
        );
        assert_eq!(header(&profile.headers(), "accept-language"), "fr-FR");
    }

    #[test]
    fn browsers_keep_their_shares_and_share_changes_one() {
        let agents = UserAgents::new().browsers([Browser::Firefox, Browser::Chrome]);
        assert_eq!(
            agents.browsers,
            [(Browser::Firefox, 7), (Browser::Chrome, 65)]
        );

        let agents = agents.share(Browser::Firefox, 30).share(Browser::Chrome, 0);
        assert_eq!(agents.browsers, [(Browser::Firefox, 30)]);
        assert!((0..50).all(|n| agents.pick(n).browser() == Browser::Firefox));
    }

    #[test]
    #[should_panic(expected = "no browser that runs on its platforms")]
    fn impossible_settings_panic() {
        let _ = UserAgents::new()
            .browsers([Browser::Edge])
            .platforms([Platform::Linux]);
    }
}
