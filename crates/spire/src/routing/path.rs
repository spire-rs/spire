use std::fmt;

/// A pattern over URI paths: `*` matches within one segment, `**` across
/// segments, and every other character itself.
#[derive(Clone, PartialEq, Eq)]
pub(crate) struct PathPattern {
    source: String,
    tokens: Vec<Token>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Token {
    Byte(u8),
    /// `*`: any run of bytes without a `/`.
    Segment,
    /// `**`: any run of bytes.
    Any,
}

impl PathPattern {
    /// Parses `pattern`.
    ///
    /// # Panics
    ///
    /// Panics if `pattern` does not start with `/`.
    pub(crate) fn new(pattern: &str) -> Self {
        assert!(
            pattern.starts_with('/'),
            "path pattern `{pattern}` does not start with `/`"
        );

        let mut tokens = Vec::with_capacity(pattern.len());
        let mut bytes = pattern.bytes().peekable();
        while let Some(byte) = bytes.next() {
            tokens.push(match byte {
                b'*' if bytes.next_if_eq(&b'*').is_some() => {
                    // `***` and beyond match what `**` does.
                    while bytes.next_if_eq(&b'*').is_some() {}
                    Token::Any
                }
                b'*' => Token::Segment,
                byte => Token::Byte(byte),
            });
        }

        Self {
            source: pattern.to_owned(),
            tokens,
        }
    }

    /// Returns whether the pattern matches all of `path`.
    ///
    /// Runs in `O(pattern × path)`, so no path backtracks exponentially.
    pub(crate) fn matches(&self, path: &str) -> bool {
        let path = path.as_bytes();
        // `matched[j]`: the tokens so far match the first `j` bytes of `path`.
        let mut matched = vec![false; path.len() + 1];
        matched[0] = true;

        for token in &self.tokens {
            let mut next = vec![false; path.len() + 1];
            for j in 0..=path.len() {
                next[j] = match token {
                    Token::Byte(byte) => j > 0 && matched[j - 1] && path[j - 1] == *byte,
                    Token::Segment => matched[j] || (j > 0 && next[j - 1] && path[j - 1] != b'/'),
                    Token::Any => matched[j] || (j > 0 && next[j - 1]),
                };
            }
            matched = next;
        }

        matched[path.len()]
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.source
    }
}

impl fmt::Debug for PathPattern {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.source)
    }
}

#[cfg(test)]
mod tests {
    use super::PathPattern;

    #[test]
    fn matches_literals_and_wildcards() {
        let cases = [
            ("/", "/", true),
            ("/", "/a", false),
            ("/products/*", "/products/42", true),
            ("/products/*", "/products/", true),
            ("/products/*", "/products/42/reviews", false),
            ("/products/*", "/products", false),
            ("/products/**", "/products/42/reviews", true),
            ("/products/*/reviews", "/products/42/reviews", true),
            ("/products/*/reviews", "/products/a/b/reviews", false),
            ("/**/reviews", "/products/42/reviews", true),
            ("/item-*.html", "/item-42.html", true),
            ("/item-*.html", "/item-42.htm", false),
            ("/***", "/a/b", true),
        ];
        for (pattern, path, expected) in cases {
            let matched = PathPattern::new(pattern).matches(path);
            assert_eq!(matched, expected, "`{pattern}` against `{path}`");
        }
    }

    #[test]
    fn many_wildcards_stay_fast() {
        let pattern = PathPattern::new(&format!("/{}b", "**a".repeat(20)));
        let path = format!("/{}", "a".repeat(10_000));
        assert!(!pattern.matches(&path));
    }

    #[test]
    #[should_panic(expected = "does not start with `/`")]
    fn relative_patterns_panic() {
        let _ = PathPattern::new("products/*");
    }
}
