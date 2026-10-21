use sitemapo::Record;
use sitemapo::record::Entry;

use crate::backend::Client;
use crate::extract::{FromContext, Sitemap};
use crate::frontier::seed::Seed;
use crate::handler::Handler;
use crate::request::{RequestExt, Tag};
use crate::worker::{IntoOutcome, Outcome};
use crate::{Context, Result, TRACING_TARGET};

/// A handler that follows a sitemap: every page is queued with the `pages`
/// tag, and every sitemap of an index with the tag of the sitemap itself, so
/// indexes of any depth are followed.
///
/// Route it like any handler, by tag, by path or as the fallback; seed
/// sitemaps yourself, or let [`Robots::with_sitemaps`] find them. A sitemap's
/// pages are one level deeper than the sitemap, so a depth limit also bounds
/// how deep indexes are followed.
///
/// A page's `<priority>` ranks it against the other pages of its site, not
/// against the rest of the crawl, so it orders the pages instead of becoming
/// their seed priority: they are queued highest first, at the default seed
/// priority, so a FIFO or priority frontier crawls a site's most important
/// pages first. A LIFO frontier pops them in reverse. To map priorities onto
/// a crawl's own scale, write a handler with the [`Sitemap`] extractor.
///
/// ```
/// use spire::handler::FollowSitemap;
/// use spire::Router;
///
/// async fn product() {}
///
/// let router: Router<spire::backend::Noop> = Router::new()
///     .route("sitemap", FollowSitemap::new("product"))
///     .route("product", product);
/// ```
///
/// [`Robots::with_sitemaps`]: crate::crawler::Robots::with_sitemaps
#[must_use]
#[derive(Debug, Clone)]
pub struct FollowSitemap {
    pages: Tag,
}

impl FollowSitemap {
    /// Follows sitemaps, queueing their pages with the `pages` tag.
    pub fn new(pages: impl Into<Tag>) -> Self {
        Self {
            pages: pages.into(),
        }
    }

    async fn follow<C: Client, S: Sync>(&self, cx: &Context<C>, state: &S) -> Result<()> {
        let queue = cx.queue().clone();
        let tag = cx.request().tag().cloned();
        let Sitemap(records) = Sitemap::from_context(cx, state).await?;

        let (mut pages, mut sitemaps) = (Vec::new(), Vec::new());
        for record in records {
            match record {
                Record::Entry(entry) => pages.push(entry),
                Record::Sitemap(sitemap) => sitemaps.push(sitemap.location),
            }
        }
        by_priority(&mut pages);

        for entry in &pages {
            if let Ok(seed) = Seed::get(entry.location.as_str()) {
                queue.follow(seed.with_tag(self.pages.clone())).await?;
            }
        }
        for location in &sitemaps {
            if let Ok(seed) = Seed::get(location.as_str()) {
                let seed = match &tag {
                    Some(tag) => seed.with_tag(tag.clone()),
                    None => seed,
                };
                queue.follow(seed).await?;
            }
        }

        let (pages, sitemaps) = (pages.len(), sitemaps.len());
        tracing::debug!(target: TRACING_TARGET, pages, sitemaps, "followed a sitemap");
        Ok(())
    }
}

/// Orders pages by their `<priority>`, highest first; pages without one have
/// the protocol's default of `0.5`, and ties keep the sitemap's order.
fn by_priority(pages: &mut [Entry]) {
    let priority = |entry: &Entry| entry.priority.unwrap_or_default().get();
    pages.sort_by(|a, b| priority(b).total_cmp(&priority(a)));
}

impl<C, S> Handler<C, FollowSitemap, S> for FollowSitemap
where
    C: Client,
    S: Send + Sync + 'static,
{
    async fn call(&self, cx: &Context<C>, state: &S) -> Outcome {
        self.follow(cx, state).await.into_outcome()
    }
}

#[cfg(test)]
mod tests {
    use sitemapo::record::Priority;

    use super::*;

    #[test]
    fn orders_pages_by_priority() {
        let page = |path: &str, priority: Option<f32>| {
            let entry = Entry::new(format!("https://example.com/{path}").parse().unwrap());
            match priority {
                Some(priority) => entry.with_priority(Priority::new(priority).unwrap()),
                None => entry,
            }
        };
        let mut pages = vec![
            page("low", Some(0.2)),
            page("unset", None),
            page("top", Some(0.9)),
            page("default", Some(0.5)),
            page("high", Some(0.8)),
        ];

        by_priority(&mut pages);
        let paths: Vec<_> = pages.iter().map(|entry| entry.location.path()).collect();
        assert_eq!(paths, ["/top", "/high", "/unset", "/default", "/low"]);
    }
}
