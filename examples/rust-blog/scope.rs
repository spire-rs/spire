//! Which pages are posts.

use spire::frontier::seed::Seed;

use crate::pages::Page;

/// Whether `seed` may be crawled: the sitemap and home page also list pages
/// that are not posts, so only dated posts are crawled as posts.
pub fn in_scope(seed: &Seed) -> bool {
    seed.tag() != Some(&Page::Post.into()) || is_post(seed.uri().path())
}

/// Whether `path` is a post's: `/2015/05/15/Rust-1.0/`, or the same under
/// `/inside-rust/`.
fn is_post(path: &str) -> bool {
    let segments: Vec<_> = path
        .split('/')
        .filter(|segment| !segment.is_empty())
        .collect();
    let segments = segments.strip_prefix(&["inside-rust"]).unwrap_or(&segments);
    let numeric =
        |segment: &str, len| segment.len() == len && segment.bytes().all(|b| b.is_ascii_digit());
    matches!(segments, [year, month, day, _slug]
        if numeric(year, 4) && numeric(month, 2) && numeric(day, 2))
}

#[cfg(test)]
mod tests {
    use super::is_post;

    #[test]
    fn recognizes_posts() {
        assert!(is_post("/2015/05/15/Rust-1.0/"));
        assert!(is_post("/inside-rust/2024/01/02/update/"));
        assert!(!is_post("/"));
        assert!(!is_post("/releases/"));
        assert!(!is_post("/2015/05/Rust-1.0/"));
        assert!(!is_post("/15/05/15/Rust-1.0/"));
    }
}
