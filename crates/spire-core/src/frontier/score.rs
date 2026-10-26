//! Scorers for best-first crawling, see
//! [`FrontierExt::score`](crate::frontier::FrontierExt::score).

use crate::frontier::seed::Seed;

/// Scores a seed: the higher, the sooner a frontier that orders by
/// priority crawls it.
///
/// Closures taking a `&Seed` and returning an `i32` are scorers.
pub trait Score: Send + Sync + 'static {
    /// Returns the score `seed` adds to its priority.
    fn score(&self, seed: &Seed) -> i32;
}

impl<F> Score for F
where
    F: Fn(&Seed) -> i32 + Send + Sync + 'static,
{
    fn score(&self, seed: &Seed) -> i32 {
        self(seed)
    }
}

/// Scores seeds by the keywords in their URL's path and query, compared
/// case-insensitively: `weight` for every keyword found.
///
/// [`matches`](Self::matches) counts keywords in any text, e.g. a link's
/// anchor text.
///
/// ```
/// use spire_core::frontier::FrontierExt;
/// use spire_core::frontier::memory::InMemFrontier;
/// use spire_core::frontier::score::Keywords;
///
/// let frontier = InMemFrontier::priority().score(Keywords::new(["laptop", "review"]));
/// ```
#[derive(Debug, Clone)]
pub struct Keywords {
    words: Vec<String>,
    weight: i32,
}

impl Keywords {
    /// Scores `1` for every keyword of `words` in a seed's URL.
    pub fn new<I>(words: I) -> Self
    where
        I: IntoIterator<Item: AsRef<str>>,
    {
        let words = words
            .into_iter()
            .map(|word| word.as_ref().to_lowercase())
            .filter(|word| !word.is_empty())
            .collect();
        Self { words, weight: 1 }
    }

    /// Scores `weight` for every keyword found, instead of `1`.
    #[must_use]
    pub const fn with_weight(mut self, weight: i32) -> Self {
        self.weight = weight;
        self
    }

    /// Returns `weight` times the number of keywords in `text`.
    #[must_use]
    pub fn matches(&self, text: &str) -> i32 {
        let text = text.to_lowercase();
        let found = self
            .words
            .iter()
            .filter(|word| text.contains(word.as_str()))
            .count();
        i32::try_from(found)
            .unwrap_or(i32::MAX)
            .saturating_mul(self.weight)
    }
}

impl Score for Keywords {
    fn score(&self, seed: &Seed) -> i32 {
        let uri = seed.uri();
        let path_and_query = uri.path_and_query().map_or("", |path| path.as_str());
        self.matches(path_and_query)
    }
}

/// Scores shallower seeds higher: minus their depth, so seeds are crawled
/// a level at a time, as breadth-first.
#[derive(Debug, Clone, Copy, Default)]
pub struct ShallowFirst;

impl Score for ShallowFirst {
    fn score(&self, seed: &Seed) -> i32 {
        i32::try_from(seed.depth().get()).map_or(i32::MIN, |depth| -depth)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frontier::test_util::seed;
    use crate::request::Depth;

    #[test]
    fn keywords_count_case_insensitively_in_path_and_query() {
        let keywords = Keywords::new(["Laptop", "review", ""]).with_weight(5);
        assert_eq!(keywords.score(&seed("laptops/REVIEWS?q=1")), 10);
        assert_eq!(keywords.score(&seed("search?q=laptop")), 5);
        assert_eq!(keywords.score(&seed("phones")), 0);
        assert_eq!(keywords.matches("A laptop"), 5);
    }

    #[test]
    fn keywords_ignore_the_host() {
        let keywords = Keywords::new(["example"]);
        assert_eq!(keywords.score(&seed("page")), 0);
    }

    #[test]
    fn shallow_first_scores_minus_the_depth() {
        assert_eq!(ShallowFirst.score(&seed("root")), 0);
        let deep = seed("deep").with_depth(Depth::ROOT.child().child());
        assert_eq!(ShallowFirst.score(&deep), -2);
    }
}
