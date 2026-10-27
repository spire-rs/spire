use cssparser::{Delimiter, Parser, Token};
use scraper::ElementRef;

use super::Visibility;

/// An HTML-only estimate: no stylesheet cascade, layout or script execution.
pub(super) fn visibility(element: ElementRef<'_>) -> Visibility {
    let hidden = std::iter::once(element)
        .chain(element.ancestors().filter_map(ElementRef::wrap))
        .any(hidden);
    if hidden {
        Visibility::Hidden
    } else {
        Visibility::Unknown
    }
}

fn hidden(element: ElementRef<'_>) -> bool {
    let element = element.value();
    if element.attr("hidden").is_some()
        || (element
            .attr("aria-hidden")
            .is_some_and(|value| value.eq_ignore_ascii_case("true"))
            && element
                .attr("tabindex")
                .is_some_and(|value| value.trim() == "-1"))
    {
        return true;
    }
    let style = element
        .attr("style")
        .unwrap_or_default()
        .to_ascii_lowercase();
    let property = |name| property(&style, name);
    if property("display").as_deref() == Some("none")
        || matches!(
            property("visibility").as_deref(),
            Some("hidden" | "collapse")
        )
    {
        return true;
    }
    let zero_width = property("width").as_deref().is_some_and(zero);
    let zero_height = property("height").as_deref().is_some_and(zero);
    let clipped = matches!(property("overflow").as_deref(), Some("hidden" | "clip"));
    if clipped && (zero_width || zero_height) {
        return true;
    }
    let positioned = matches!(property("position").as_deref(), Some("absolute" | "fixed"));
    positioned
        && ["left", "top", "right", "bottom"]
            .into_iter()
            .filter_map(property)
            .any(|value| {
                value
                    .strip_suffix("px")
                    .and_then(|value| value.trim().parse::<f64>().ok())
                    .is_some_and(|value| value <= -1000.0)
            })
}

/// CSS tokenization respects comments, escapes, strings and nested functions.
/// This reads simple values, not the stylesheet cascade or CSS layout.
fn property(style: &str, name: &str) -> Option<String> {
    let mut parser = Parser::new(style);
    let mut found = None;
    while !parser.is_exhausted() {
        let declaration = parser.parse_until_after(Delimiter::Semicolon, |input| {
            let key = input.expect_ident_cloned()?;
            input.expect_colon()?;
            let mut tokens = Vec::new();
            while let Ok(token) = input.next() {
                tokens.push(token.clone());
            }
            let important = matches!(tokens.as_slice(), [.., Token::Delim('!'), Token::Ident(value)] if value.eq_ignore_ascii_case("important"));
            if important { tokens.truncate(tokens.len() - 2); }
            let value = match tokens.as_slice() {
                [Token::Ident(value)] => value.to_ascii_lowercase(),
                [Token::Number { value, .. }] => value.to_string(),
                [Token::Dimension { value, unit, .. }] => format!("{value}{}", unit.to_ascii_lowercase()),
                [Token::Percentage { unit_value, .. }] => format!("{}%", unit_value * 100.0),
                _ => String::new(),
            };
            Ok::<_, cssparser::ParseError<()>>((key, value, important))
        });
        if let Ok((key, value, important)) = declaration
            && key.eq_ignore_ascii_case(name)
            && (important || !found.as_ref().is_some_and(|(_, important)| *important))
        {
            found = Some((value, important));
        }
    }
    found.map(|(value, _)| value)
}

fn zero(value: &str) -> bool {
    let number = value.trim_end_matches(|ch: char| ch.is_ascii_alphabetic() || ch == '%');
    number.parse::<f64>().is_ok_and(|number| number == 0.0)
}

#[cfg(test)]
mod tests {
    use crate::extract::html::Html;

    #[test]
    fn skips_hidden_links_and_ancestors_but_keeps_accessible_links() {
        let html = Html::parse(
            r#"
            <a href="/visible">visible</a>
            <a href="/keyboard" tabindex="-1">keyboard</a>
            <a href="/aria" aria-hidden="true">aria alone</a>
            <div hidden><a href="/hidden">hidden</a></div>
            <div style="DISPLAY: none !important; display: block"><a href="/display">none</a></div>
            <a href="/visibility" style="visibility: hidden">hidden</a>
            <div aria-hidden="true" tabindex="-1"><a href="/trap">trap</a></div>
            <a href="/offscreen" style="position:absolute;left:-9999px">trap</a>
            <a href="/zero" style="width:0px;height:0;overflow:hidden">trap</a>
            <a href="/clipped" style="height:0;overflow:hidden">trap</a>
            <a href="/restored" style="display:none;display:block">restored</a>
            <a href="/small" style="position:relative;left:-2px">small adjustment</a>
        "#,
        );
        let links = html.links(&"https://site.test/".parse().unwrap());
        assert_eq!(links.len(), 12);
        let visible: Vec<_> = links
            .visible()
            .into_iter()
            .map(|link| link.uri().path().to_owned())
            .collect();
        assert_eq!(
            visible,
            ["/visible", "/keyboard", "/aria", "/restored", "/small"]
        );
    }
}

#[cfg(test)]
mod css_tests {
    use super::property;
    use crate::extract::html::{Html, Visibility};

    #[test]
    fn handles_css_comments_escapes_and_important_without_reading_strings_as_rules() {
        assert_eq!(
            property(r"d\69 splay:/**/none ! important", "display").as_deref(),
            Some("none")
        );
        assert_eq!(
            property("display:none!important;display:block", "display").as_deref(),
            Some("none")
        );
        assert_eq!(
            property("display:none;display:block", "display").as_deref(),
            Some("block")
        );
        assert!(property("content:';display:none;';color:red", "display").is_none());
        let html = Html::parse(r#"<a href="/" style="content:';display:none;'">link</a>"#);
        assert_eq!(
            html.links(&"https://site.test/".parse().unwrap())
                .iter()
                .next()
                .unwrap()
                .visibility(),
            Visibility::Unknown
        );
    }
}
