use std::fmt;

/// A pattern over URI paths: `*` matches within one segment, `**` across
/// segments, and every other character itself.
///
/// The literal prefix before the first wildcard and suffix after the last are
/// checked first, which rules out most paths at once; only the part between
/// is matched wildcard by wildcard.
#[derive(Clone, PartialEq, Eq)]
pub(crate) struct PathPattern {
    source: String,
    /// The bytes before the first wildcard, or the whole pattern without one.
    prefix: Vec<u8>,
    /// The bytes after the last wildcard.
    suffix: Vec<u8>,
    /// The tokens from the first wildcard to the last; empty without one.
    middle: Vec<Token>,
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

        let literal = |tokens: &[Token]| -> Vec<u8> {
            tokens
                .iter()
                .map(|token| match token {
                    Token::Byte(byte) => *byte,
                    Token::Segment | Token::Any => unreachable!("literal tokens only"),
                })
                .collect()
        };
        let is_wildcard = |token: &Token| !matches!(token, Token::Byte(_));
        let (prefix, suffix, middle) = match tokens.iter().position(is_wildcard) {
            None => (literal(&tokens), Vec::new(), Vec::new()),
            Some(first) => {
                let last = tokens.iter().rposition(is_wildcard).unwrap_or(first);
                (
                    literal(&tokens[..first]),
                    literal(&tokens[last + 1..]),
                    tokens[first..=last].to_vec(),
                )
            }
        };

        Self {
            source: pattern.to_owned(),
            prefix,
            suffix,
            middle,
        }
    }

    /// Returns whether the pattern matches all of `path`.
    ///
    /// Runs in `O(pattern × path)` at worst, so no path backtracks
    /// exponentially.
    pub(crate) fn matches(&self, path: &str) -> bool {
        let path = path.as_bytes();
        if self.middle.is_empty() {
            return path == self.prefix;
        }
        if path.len() < self.prefix.len() + self.suffix.len()
            || !path.starts_with(&self.prefix)
            || !path.ends_with(&self.suffix)
        {
            return false;
        }

        let rest = &path[self.prefix.len()..path.len() - self.suffix.len()];
        match self.middle.as_slice() {
            [Token::Any] => true,
            [Token::Segment] => !rest.contains(&b'/'),
            middle => matches_tokens(middle, rest),
        }
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.source
    }
}

/// Whether `tokens` match all of `path`, by dynamic programming over two
/// reused rows.
fn matches_tokens(tokens: &[Token], path: &[u8]) -> bool {
    // `matched[j]`: the tokens so far match the first `j` bytes of `path`.
    let mut matched = vec![false; path.len() + 1];
    let mut next = vec![false; path.len() + 1];
    matched[0] = true;

    for token in tokens {
        for j in 0..=path.len() {
            next[j] = match token {
                Token::Byte(byte) => j > 0 && matched[j - 1] && path[j - 1] == *byte,
                Token::Segment => matched[j] || (j > 0 && next[j - 1] && path[j - 1] != b'/'),
                Token::Any => matched[j] || (j > 0 && next[j - 1]),
            };
        }
        std::mem::swap(&mut matched, &mut next);
    }

    matched[path.len()]
}

impl fmt::Debug for PathPattern {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.source)
    }
}

#[cfg(test)]
mod tests {
    use super::{PathPattern, Token, matches_tokens};

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
            // Literal prefixes and suffixes, alone and around wildcards.
            ("/exact", "/exact", true),
            ("/exact", "/exact/", false),
            ("/a*a", "/a", false),
            ("/a*a", "/aa", true),
            ("/a**z", "/a/b/z", true),
            ("/a**z", "/a/b/y", false),
            ("/*/*.html", "/x/y.html", true),
            ("/*/*.html", "/x/y/z.html", false),
            ("/**/item-*.html", "/a/b/item-42.html", true),
            ("/**/item-*.html", "/a/b/item-42/x.html", false),
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

    /// Every string of up to `len` parts from `alphabet`.
    fn strings(alphabet: &[&str], len: usize) -> Vec<String> {
        let mut all = vec![String::new()];
        let mut last = vec![String::new()];
        for _ in 0..len {
            last = last
                .iter()
                .flat_map(|prefix| alphabet.iter().map(move |part| format!("{prefix}{part}")))
                .collect();
            all.extend(last.iter().cloned());
        }
        all
    }

    #[test]
    fn agrees_with_matching_the_whole_pattern() {
        let patterns = strings(&["a", "/", "*", "**"], 4);
        let paths = strings(&["a", "b", "/"], 5);
        for pattern in patterns.iter().map(|rest| format!("/{rest}")) {
            let parsed = PathPattern::new(&pattern);
            // The pattern's tokens matched whole, without the shortcuts.
            let mut tokens: Vec<Token> = parsed.prefix.iter().map(|&b| Token::Byte(b)).collect();
            tokens.extend(&parsed.middle);
            tokens.extend(parsed.suffix.iter().map(|&b| Token::Byte(b)));

            for path in paths.iter().map(|rest| format!("/{rest}")) {
                let expected = matches_tokens(&tokens, path.as_bytes());
                let matched = parsed.matches(&path);
                assert_eq!(matched, expected, "`{pattern}` against `{path}`");
            }
        }
    }

    #[test]
    #[should_panic(expected = "does not start with `/`")]
    fn relative_patterns_panic() {
        let _ = PathPattern::new("products/*");
    }
}
