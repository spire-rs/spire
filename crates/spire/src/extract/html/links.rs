use std::sync::LazyLock;

use scraper::{ElementRef, Selector};
use spire_core::http::ResponseExt;
use url::Url;

use crate::backend::Session;
use crate::extract::FromContext;
use crate::extract::html::Html;
use crate::frontier::queue::Link;
use crate::http::http::Uri;
use crate::{Context, Error, Result};

/// The links a visitor can follow: `<a href>` and `<area href>`.
static LINKS: LazyLock<Selector> =
    LazyLock::new(|| Selector::parse("a[href], area[href]").expect("valid selector"));

/// A page's `<base href>`, which relative links resolve against.
static BASE: LazyLock<Selector> =
    LazyLock::new(|| Selector::parse("base[href]").expect("valid selector"));

pub use spire_core::backend::document::{LinkKind, Visibility};

/// A link of a page, see [`Links`].
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct PageLink {
    uri: Uri,
    href: String,
    rel: Vec<String>,
    text: String,
    kind: LinkKind,
    visibility: Visibility,
}

impl PageLink {
    /// Returns the URI the link leads to: resolved against the page, as a
    /// browser does, without its fragment.
    #[must_use]
    pub const fn uri(&self) -> &Uri {
        &self.uri
    }

    /// Returns the `href` as written in the page.
    #[must_use]
    pub fn href(&self) -> &str {
        &self.href
    }

    /// Returns the `rel` values, lowercased, e.g. `nofollow` or `next`.
    #[must_use]
    pub fn rel(&self) -> &[String] {
        &self.rel
    }

    /// Whether the link has the `rel` value `value`, compared
    /// case-insensitively.
    #[must_use]
    pub fn has_rel(&self, value: &str) -> bool {
        self.rel.iter().any(|rel| rel.eq_ignore_ascii_case(value))
    }

    /// Returns the text a visitor sees, with whitespace collapsed: the
    /// anchor's text, or an area's `alt`.
    #[must_use]
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Returns the observed visibility. Browser captures use computed
    /// styles and layout; HTML-only checks return `Hidden` or `Unknown`.
    #[must_use]
    pub const fn visibility(&self) -> Visibility {
        self.visibility
    }

    /// Returns the element the link comes from.
    #[must_use]
    pub const fn kind(&self) -> LinkKind {
        self.kind
    }
}

/// Follows the link, with no tag, from the page it was found on.
impl From<PageLink> for Link {
    fn from(link: PageLink) -> Self {
        Self::from(link.uri)
    }
}

impl From<&PageLink> for Link {
    fn from(link: &PageLink) -> Self {
        Self::from(link.uri.clone())
    }
}

/// A page's navigable links, resolved to absolute HTTP(S) URLs.
///
/// Reads `<a>` and `<area>` links, resolving them against the page's
/// `<base href>`, if present, and the URI it was served from. Links that are
/// not HTTP(S) URLs, such as `mailto:`, are left out.
///
/// As an extractor, it reads the page's links in document order; the
/// filters narrow them, and it converts into links to
/// [follow](crate::frontier::queue::Queue::follow_all):
///
/// ```
/// use spire::Result;
/// use spire::extract::html::Links;
/// use spire::frontier::queue::Queue;
///
/// async fn page(links: Links, queue: Queue) -> Result<()> {
///     // The next page of a listing, if the site says which it is.
///     queue.follow_all(links.followable().same_host().with_rel("next")).await?;
///     Ok(())
/// }
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Links {
    /// The host of the page, for [`same_host`](Self::same_host).
    host: Option<String>,
    links: Vec<PageLink>,
}

impl Links {
    /// Reads current links, using a rendered capture when the client has
    /// a DOM, or HTML attributes and inline styles otherwise.
    pub(crate) async fn read<C: Session, S: Sync>(
        cx: &Context<C>,
        state: &S,
        selector: Option<(&str, &Selector)>,
    ) -> Result<Self> {
        cx.response().await?;
        if let Some(document) = cx.session().document() {
            let snapshot = document.links(selector.map(|(css, _)| css)).await?;
            let base = Url::parse(&snapshot.base_uri).ok();
            let resolver = Resolver { base };
            let links = snapshot
                .links
                .into_iter()
                .filter_map(|link| {
                    Some(PageLink {
                        uri: resolver.resolve(&link.href)?,
                        href: link.href,
                        rel: link
                            .rel
                            .into_iter()
                            .map(|rel| rel.to_ascii_lowercase())
                            .collect(),
                        text: link.text.split_whitespace().collect::<Vec<_>>().join(" "),
                        kind: link.kind,
                        visibility: link.visibility,
                    })
                })
                .collect();
            return Ok(Self::new(&snapshot.uri, links));
        }
        let html = Html::from_context(cx, state).await?;
        let response = cx.response().await?;
        let page = response.final_uri().unwrap_or_else(|| cx.request().uri());
        match selector {
            Some((_, selector)) => {
                let elements = html
                    .0
                    .select(&LINKS)
                    .filter(|element| selector.matches(element));
                Ok(Self::from_elements(&html, page, elements))
            }
            None => Ok(html.links(page)),
        }
    }

    /// Returns the links inside the elements matching `css`, e.g. a page's
    /// navigation or its article, resolved against the page served from
    /// `page`.
    ///
    /// # Errors
    ///
    /// Returns an [`Invalid`](crate::ErrorKind::Invalid) error if `css` is
    /// not a valid CSS selector.
    pub fn within(html: &Html, page: &Uri, css: &str) -> Result<Self> {
        let scope = Selector::parse(css)
            .map_err(|error| Error::invalid(format!("invalid CSS selector `{css}`: {error}")))?;
        let elements = html.0.select(&scope).flat_map(|scope| scope.select(&LINKS));
        Ok(Self::from_elements(html, page, elements))
    }

    /// Returns the links among `elements`, e.g. those a selector matched.
    pub(crate) fn from_elements<'a>(
        html: &Html,
        page: &Uri,
        elements: impl Iterator<Item = ElementRef<'a>>,
    ) -> Self {
        let resolver = Resolver::new(html, page);
        let links = elements
            .filter_map(|element| resolver.link(element))
            .collect();
        Self::new(page, links)
    }

    fn new(page: &Uri, links: Vec<PageLink>) -> Self {
        Self {
            host: page.host().map(str::to_ascii_lowercase),
            links,
        }
    }

    /// Keeps links not hidden by attributes or inline styles on themselves
    /// or an ancestor: `hidden`, `display:none`, `visibility:hidden`,
    /// `aria-hidden="true"` paired with `tabindex="-1"`, clipped zero-sized
    /// boxes, or absolute/fixed positioning at least 1000px off-screen.
    ///
    /// Unknown links are kept. As an extractor on a DOM client, visibility
    /// comes from the browser's computed styles and layout. HTML parsed by
    /// hand and HTTP clients only estimate it, without stylesheets or layout.
    #[must_use]
    pub fn visible(self) -> Self {
        self.retain(|link| link.visibility != Visibility::Hidden)
    }

    /// Keeps the links a crawler may follow: those without `rel="nofollow"`.
    #[must_use]
    pub fn followable(self) -> Self {
        self.retain(|link| !link.has_rel("nofollow"))
    }

    /// Keeps the links to the page's own host, compared case-insensitively;
    /// subdomains are other hosts.
    #[must_use]
    pub fn same_host(self) -> Self {
        let host = self.host.clone();
        self.retain(|link| link.uri.host().map(str::to_ascii_lowercase) == host)
    }

    /// Keeps the links with the `rel` value `value`, e.g. `next`.
    #[must_use]
    pub fn with_rel(self, value: &str) -> Self {
        self.retain(|link| link.has_rel(value))
    }

    /// Keeps the links from `kind` of element.
    #[must_use]
    pub fn of_kind(self, kind: LinkKind) -> Self {
        self.retain(|link| link.kind == kind)
    }

    fn retain(mut self, keep: impl FnMut(&PageLink) -> bool) -> Self {
        self.links.retain(keep);
        self
    }

    /// Returns the links in document order.
    pub fn iter(&self) -> std::slice::Iter<'_, PageLink> {
        self.links.iter()
    }

    /// Returns the number of links.
    #[must_use]
    pub fn len(&self) -> usize {
        self.links.len()
    }

    /// Whether there is no link.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.links.is_empty()
    }
}

impl IntoIterator for Links {
    type IntoIter = std::vec::IntoIter<PageLink>;
    type Item = PageLink;

    fn into_iter(self) -> Self::IntoIter {
        self.links.into_iter()
    }
}

impl<'a> IntoIterator for &'a Links {
    type IntoIter = std::slice::Iter<'a, PageLink>;
    type Item = &'a PageLink;

    fn into_iter(self) -> Self::IntoIter {
        self.links.iter()
    }
}

impl<C, S> FromContext<C, S> for Links
where
    C: Session,
    S: Sync,
{
    type Rejection = Error;

    async fn from_context(cx: &Context<C>, state: &S) -> Result<Self> {
        Self::read(cx, state, None).await
    }
}

impl Html {
    /// Returns the page's links, resolved against the page served from
    /// `page`, see [`Links`].
    #[must_use]
    pub fn links(&self, page: &Uri) -> Links {
        Links::from_elements(self, page, self.0.select(&LINKS))
    }
}

/// Resolves the links of a page as a browser does: against its
/// `<base href>`, itself resolved against the URI the page was served from.
struct Resolver {
    base: Option<Url>,
}

impl Resolver {
    fn new(html: &Html, page: &Uri) -> Self {
        let page = Url::parse(&page.to_string()).ok();
        let base = html
            .0
            .select(&BASE)
            .next()
            .and_then(|base| base.value().attr("href"))
            .and_then(|href| page.as_ref()?.join(href.trim()).ok());
        Self {
            base: base.or(page),
        }
    }

    /// Resolves `href` into an `http(s)` URI without its fragment, or `None`
    /// for any other link, e.g. `mailto:` or a malformed one.
    fn resolve(&self, href: &str) -> Option<Uri> {
        let mut url = self.base.as_ref()?.join(href.trim()).ok()?;
        if !matches!(url.scheme(), "http" | "https") {
            return None;
        }
        url.set_fragment(None);
        url.as_str().parse().ok()
    }

    fn link(&self, element: ElementRef<'_>) -> Option<PageLink> {
        let value = element.value();
        let href = value.attr("href")?;
        let kind = match value.name() {
            "area" => LinkKind::Area,
            _ => LinkKind::Anchor,
        };
        let text = match kind {
            LinkKind::Area => value.attr("alt").unwrap_or_default().to_owned(),
            _ => element.text().collect(),
        };
        Some(PageLink {
            uri: self.resolve(href)?,
            href: href.to_owned(),
            rel: value
                .attr("rel")
                .unwrap_or_default()
                .split_ascii_whitespace()
                .map(str::to_ascii_lowercase)
                .collect(),
            text: text.split_whitespace().collect::<Vec<_>>().join(" "),
            kind,
            visibility: super::visibility::visibility(element),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PAGE: &str = r#"<html><head><base href="/docs/"></head><body>
        <nav><a href="/">Home</a> <a href="intro">Intro</a></nav>
        <article>
          <a href="install#linux" rel="Next">  Install
             now </a>
          <a href="https://other.test/x" rel="nofollow noopener">Elsewhere</a>
          <a href="mailto:me@example.com">Mail</a>
          <a href="javascript:void(0)">Nothing</a>
          <map><area href="/map/north" alt="North"></map>
        </article>
    </body></html>"#;

    fn page() -> Uri {
        "https://site.test/old/page".parse().unwrap()
    }

    fn uris(links: &Links) -> Vec<String> {
        links.iter().map(|link| link.uri().to_string()).collect()
    }

    #[test]
    fn resolves_against_the_base_element() {
        let links = Html::parse(PAGE).links(&page());
        assert_eq!(
            uris(&links),
            [
                "https://site.test/",
                "https://site.test/docs/intro",
                "https://site.test/docs/install",
                "https://other.test/x",
                "https://site.test/map/north",
            ]
        );
    }

    #[test]
    fn records_rel_text_and_kind() {
        let links = Html::parse(PAGE).links(&page());
        let install = &links.iter().nth(2).unwrap();
        assert_eq!(install.href(), "install#linux");
        assert_eq!(install.rel(), ["next"]);
        assert_eq!(install.text(), "Install now");
        assert_eq!(install.kind(), LinkKind::Anchor);
        let north = links.iter().last().unwrap();
        assert_eq!((north.text(), north.kind()), ("North", LinkKind::Area));
    }

    #[test]
    fn without_a_base_resolves_against_the_page() {
        let links = Html::parse(r#"<a href="next">Next</a>"#).links(&page());
        assert_eq!(uris(&links), ["https://site.test/old/next"]);
    }

    #[test]
    fn filters_narrow_the_links() {
        let links = Html::parse(PAGE).links(&page());
        assert_eq!(links.clone().followable().len(), 4);
        assert_eq!(
            uris(&links.clone().same_host().with_rel("next")),
            ["https://site.test/docs/install"]
        );
        assert_eq!(links.of_kind(LinkKind::Area).len(), 1);
    }

    #[test]
    fn within_scopes_the_links() {
        let html = Html::parse(PAGE);
        let nav = Links::within(&html, &page(), "nav").unwrap();
        assert_eq!(
            uris(&nav),
            ["https://site.test/", "https://site.test/docs/intro"]
        );
        assert!(Links::within(&html, &page(), "nav[").is_err());
    }
}
