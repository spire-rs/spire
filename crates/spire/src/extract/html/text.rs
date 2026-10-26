use std::sync::LazyLock;

use scraper::{ElementRef, Selector};

use crate::extract::html::Html;

/// Elements whose text is never shown.
pub(super) const HIDDEN: &[&str] = &["script", "style", "noscript", "template", "head"];

/// Elements around a page's main content, left out of [`Html::main_text`].
const AROUND: &[&str] = &["nav", "aside"];

/// Elements around the content of a page without a main element, left out
/// as well when the main content is the whole `<body>`. Inside `<main>` or
/// `<article>`, a header or footer belongs to the content.
const AROUND_BODY: &[&str] = &["header", "footer"];

/// Elements that start a new line of text.
const BLOCKS: &[&str] = &[
    "address",
    "article",
    "aside",
    "blockquote",
    "br",
    "dd",
    "details",
    "div",
    "dl",
    "dt",
    "fieldset",
    "figcaption",
    "figure",
    "footer",
    "form",
    "h1",
    "h2",
    "h3",
    "h4",
    "h5",
    "h6",
    "header",
    "hr",
    "li",
    "main",
    "nav",
    "ol",
    "p",
    "pre",
    "section",
    "summary",
    "table",
    "td",
    "th",
    "tr",
    "ul",
];

/// Where a page's main content is, in order of preference.
pub(super) static MAIN: LazyLock<Vec<Selector>> = LazyLock::new(|| {
    ["main", "article", r#"[role="main"]"#, "body"]
        .iter()
        .map(|css| Selector::parse(css).expect("valid selector"))
        .collect()
});

impl Html {
    /// Returns the text a reader sees on the page: the `<body>`, without
    /// scripts, styles and other hidden elements, one line per block such
    /// as a paragraph or a list item, with whitespace collapsed.
    #[must_use]
    pub fn visible_text(&self) -> String {
        let body = MAIN.last().and_then(|body| self.0.select(body).next());
        text_of(body.unwrap_or_else(|| self.0.root_element()), HIDDEN)
    }

    /// Returns the text of the page's main content, as
    /// [`visible_text`](Self::visible_text) does: the first `<main>`, or
    /// else `<article>`, or else an element with `role="main"`, or else the
    /// `<body>`, without navigation and asides, nor, for the `<body>`, its
    /// header and footer.
    ///
    /// It is a heuristic: pages that mark up their content otherwise get
    /// more or less than their main content.
    #[must_use]
    pub fn main_text(&self) -> String {
        let main = self.main_content();
        text_of(main.root, &main.skip)
    }

    /// Finds the main content: the element, and the elements under it to
    /// leave out.
    pub(crate) fn main_content(&self) -> MainContent<'_> {
        let found = MAIN.iter().enumerate().find_map(|(index, selector)| {
            let element = self.0.select(selector).next()?;
            Some((element, index == MAIN.len() - 1))
        });
        let (root, body) = found.unwrap_or_else(|| (self.0.root_element(), true));
        let mut skip: Vec<&'static str> = HIDDEN.iter().chain(AROUND).copied().collect();
        if body {
            skip.extend(AROUND_BODY);
        }
        MainContent { root, skip }
    }
}

/// A page's main content, see [`Html::main_content`].
pub(crate) struct MainContent<'a> {
    /// The element holding it.
    pub(crate) root: ElementRef<'a>,
    /// The elements under it that are not part of it.
    pub(crate) skip: Vec<&'static str>,
}

/// Collects the text under `root`, skipping the elements named in `skip`.
fn text_of(root: ElementRef<'_>, skip: &[&str]) -> String {
    let mut raw = String::new();
    walk(root, skip, &mut raw);
    raw.lines()
        .map(|line| line.split_whitespace().collect::<Vec<_>>().join(" "))
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

fn walk(element: ElementRef<'_>, skip: &[&str], out: &mut String) {
    let name = element.value().name();
    if skip.contains(&name) {
        return;
    }
    let block = BLOCKS.contains(&name);
    if block {
        out.push('\n');
    }
    for child in element.children() {
        if let Some(text) = child.value().as_text() {
            // Line breaks in the source are only whitespace; blocks make
            // the lines.
            out.extend(text.chars().map(|c| if c == '\n' { ' ' } else { c }));
        } else if let Some(child) = ElementRef::wrap(child) {
            walk(child, skip, out);
        }
    }
    if block {
        out.push('\n');
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PAGE: &str = r#"<html><head><title>Shop</title><style>p { color: red }</style></head>
        <body>
          <nav><a href="/">Home</a> <a href="/shop">Shop</a></nav>
          <main>
            <h1>The   desk
              lamp</h1>
            <p>Bright, <em>warm</em> light.</p>
            <script>track()</script>
            <aside>Related: chairs</aside>
            <ul><li>Brass</li><li>Steel</li></ul>
          </main>
          <footer>© Shop</footer>
        </body></html>"#;

    #[test]
    fn visible_text_skips_hidden_elements_and_keeps_blocks_apart() {
        let text = Html::parse(PAGE).visible_text();
        assert_eq!(
            text,
            "Home Shop\nThe desk lamp\nBright, warm light.\nRelated: chairs\nBrass\nSteel\n© Shop"
        );
    }

    #[test]
    fn main_text_reads_the_main_content() {
        let text = Html::parse(PAGE).main_text();
        assert_eq!(text, "The desk lamp\nBright, warm light.\nBrass\nSteel");

        // Without `<main>`, an `<article>`, and without either, the body.
        let article = Html::parse("<body><nav>Menu</nav><article><p>Story</p></article></body>");
        assert_eq!(article.main_text(), "Story");
        let body = Html::parse(
            "<body><header>Logo</header><nav>Menu</nav><p>Text</p><footer>©</footer></body>",
        );
        assert_eq!(body.main_text(), "Text");

        // Inside `<article>`, its header and footer are part of the story.
        let article = Html::parse(
            "<body><article><header>Title</header><p>Story</p><footer>By me</footer></article></body>",
        );
        assert_eq!(article.main_text(), "Title\nStory\nBy me");
    }
}
