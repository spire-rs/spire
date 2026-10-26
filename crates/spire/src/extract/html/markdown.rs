use std::sync::LazyLock;

use html_to_markdown_rs::{ConversionOptions, PreprocessingOptions};
use scraper::Selector;
use spire_core::request::ResponseExt;

use super::text::{HIDDEN, MAIN};
use crate::backend::Client;
use crate::extract::FromContext;
use crate::extract::html::Html;
use crate::request::http::Uri;
use crate::{Context, Error, Result};

/// A page's `<base href>`, which relative links resolve against.
static BASE: LazyLock<Selector> =
    LazyLock::new(|| Selector::parse("base[href]").expect("valid selector"));

/// The page's main content as Markdown, with its links and images resolved
/// against the URI the page was served from.
///
/// As an extractor, it converts with the default [`MarkdownOptions`] and
/// the response's final URI as the base; [`from_html`](Self::from_html)
/// converts with other options.
///
/// ```
/// use spire::extract::html::Markdown;
///
/// async fn page(Markdown(markdown): Markdown) {
///     println!("{markdown}");
/// }
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Markdown(pub String);

impl<C, S> FromContext<C, S> for Markdown
where
    C: Client,
    S: Sync,
{
    type Rejection = Error;

    async fn from_context(cx: &Context<C>, state: &S) -> Result<Self> {
        let html = Html::from_context(cx, state).await?;
        let response = cx.response().await?;
        let base = response
            .final_uri()
            .unwrap_or_else(|| cx.request().uri())
            .clone();
        Self::from_html(&html, &MarkdownOptions::new().with_base(base))
    }
}

/// How [`Markdown::from_html`] converts a page.
#[derive(Debug, Clone)]
pub struct MarkdownOptions {
    base: Option<Uri>,
    whole_body: bool,
    images: bool,
    links: bool,
}

impl Default for MarkdownOptions {
    fn default() -> Self {
        Self {
            base: None,
            whole_body: false,
            images: true,
            links: true,
        }
    }
}

impl MarkdownOptions {
    /// Converts the main content, keeping links and images as written.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Resolves relative links and images against `base`, e.g. the URI the
    /// page was served from, and its `<base href>` if it has one.
    #[must_use]
    pub fn with_base(mut self, base: Uri) -> Self {
        self.base = Some(base);
        self
    }

    /// Converts the whole `<body>`, navigation and footers included,
    /// instead of the main content.
    #[must_use]
    pub const fn whole_body(mut self) -> Self {
        self.whole_body = true;
        self
    }

    /// Leaves images out.
    #[must_use]
    pub const fn without_images(mut self) -> Self {
        self.images = false;
        self
    }

    /// Keeps the text of links, without their targets.
    #[must_use]
    pub const fn without_links(mut self) -> Self {
        self.links = false;
        self
    }
}

impl Markdown {
    /// Converts the page's main content, as [`Html::main_text`] finds it, to
    /// Markdown: headings, paragraphs, lists, emphasis, links,
    /// images, tables, block quotes and code are kept, scripts, navigation
    /// and asides left out.
    ///
    /// Conversion is done by [html-to-markdown-rs](https://docs.rs/html-to-markdown-rs),
    /// which logs an `html_to_markdown::convert` span for each page.
    ///
    /// ```
    /// use spire::extract::html::{Html, Markdown, MarkdownOptions};
    ///
    /// let html = Html::parse("<main><h1>Lamps</h1><p>See <a href='/desk'>the desk lamp</a>.</p></main>");
    /// let base = "https://shop.test/lamps/".parse().unwrap();
    /// let Markdown(markdown) = Markdown::from_html(&html, &MarkdownOptions::new().with_base(base)).unwrap();
    /// assert!(markdown.contains("[the desk lamp](https://shop.test/desk)"));
    /// ```
    ///
    /// # Errors
    ///
    /// Returns a [`Worker`](crate::ErrorKind::Worker) error if the
    /// conversion fails.
    pub fn from_html(html: &Html, options: &MarkdownOptions) -> Result<Self> {
        let (root, skip) = if options.whole_body {
            let body = MAIN.last().and_then(|body| html.0.select(body).next());
            let body = body.unwrap_or_else(|| html.0.root_element());
            (body, HIDDEN.to_vec())
        } else {
            let main = html.main_content();
            (main.root, main.skip)
        };

        // The main content is converted on its own, so its `<base href>`,
        // from the head, goes in front of it.
        let mut input = String::new();
        if let Some(base) = html.0.select(&BASE).next() {
            input.push_str(&base.html());
        }
        input.push_str(&root.html());

        // spire decides what is content, as for `main_text`; the
        // converter's own cleanup would drop forms and headers it guesses
        // are navigation, even from the whole body.
        let preprocessing = PreprocessingOptions {
            enabled: false,
            ..PreprocessingOptions::default()
        };
        let mut builder = ConversionOptions::builder()
            .preprocessing(preprocessing)
            .extract_metadata(false)
            .skip_images(!options.images)
            .exclude_selectors(skip.iter().map(|tag| (*tag).to_owned()).collect())
            .base_url(options.base.as_ref().map(ToString::to_string));
        if !options.links {
            builder = builder.strip_tags(vec!["a".to_owned()]);
        }
        let converted = html_to_markdown_rs::convert(&input, builder.build()).map_err(|error| {
            Error::worker("failed to convert the page to Markdown").with_source(error)
        })?;
        Ok(Self(
            converted.content.unwrap_or_default().trim().to_owned(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PAGE: &str = r#"<html><head><title>Shop</title><style>p { color: red }</style></head>
        <body>
          <header>Logo</header>
          <nav><a href="/">Home</a></nav>
          <main>
            <h1>Desk lamp</h1>
            <p>Bright, <em>warm</em> light. See <a href="reviews">reviews</a>.</p>
            <img src="/lamp.jpg" alt="The lamp">
            <script>track()</script>
            <aside>Related: chairs</aside>
            <ul><li>Brass</li><li>Steel</li></ul>
            <table><tr><th>Size</th><th>Price</th></tr><tr><td>Small</td><td>$20</td></tr></table>
          </main>
          <footer>© Shop</footer>
        </body></html>"#;

    fn convert(page: &str, options: &MarkdownOptions) -> String {
        Markdown::from_html(&Html::parse(page), options).unwrap().0
    }

    fn base() -> Uri {
        "https://shop.test/lamps/desk".parse().unwrap()
    }

    #[test]
    fn converts_the_main_content() {
        let markdown = convert(PAGE, &MarkdownOptions::new());
        assert!(markdown.starts_with("# Desk lamp"), "{markdown}");
        assert!(markdown.contains("*warm*"), "{markdown}");
        assert!(markdown.contains("[reviews](reviews)"), "{markdown}");
        assert!(markdown.contains("Brass"), "{markdown}");
        assert!(markdown.contains("| Small |"), "{markdown}");
        for left_out in ["Logo", "Home", "track()", "Related", "© Shop", "color: red"] {
            assert!(!markdown.contains(left_out), "{left_out} in {markdown}");
        }
    }

    #[test]
    fn resolves_links_and_images_against_the_base() {
        let options = MarkdownOptions::new().with_base(base());
        let markdown = convert(PAGE, &options);
        assert!(
            markdown.contains("[reviews](https://shop.test/lamps/reviews)"),
            "{markdown}"
        );
        assert!(
            markdown.contains("![The lamp](https://shop.test/lamp.jpg)"),
            "{markdown}"
        );
    }

    #[test]
    fn honours_the_base_element() {
        let page = r#"<html><head><base href="/docs/"></head>
            <body><main><a href="intro">Intro</a></main></body></html>"#;
        let options = MarkdownOptions::new().with_base(base());
        let markdown = convert(page, &options);
        assert!(
            markdown.contains("[Intro](https://shop.test/docs/intro)"),
            "{markdown}"
        );
    }

    #[test]
    fn options_change_what_is_kept() {
        let body = convert(PAGE, &MarkdownOptions::new().whole_body());
        assert!(body.contains("Logo") && body.contains("© Shop"), "{body}");
        assert!(!body.contains("track()"), "{body}");

        let bare = convert(
            PAGE,
            &MarkdownOptions::new().without_images().without_links(),
        );
        assert!(!bare.contains("lamp.jpg"), "{bare}");
        assert!(
            bare.contains("reviews") && !bare.contains("](reviews)"),
            "{bare}"
        );
    }
}
