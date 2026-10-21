//! Testing handlers without a network: [`MockSite`] serves fixed
//! [`MockPage`]s, and a [`Visit`] runs one request through a worker.
//!
//! A [`MockSite`] is a [`Backend`](crate::backend::Backend), so a whole crawl
//! can run against it. To test what a handler does with one page, without
//! crawling the links it queues, [`visit`](MockSite::visit) the page:
//!
//! ```
//! use spire_core::dataset::InMemDataset;
//! use spire_core::frontier::seed::Seed;
//! use spire_core::testing::{MockPage, MockSite};
//! use spire_core::worker::{Outcome, from_fn};
//! use spire_core::{Context, Result};
//!
//! async fn page(cx: Context<impl spire_core::backend::Client>) -> Result<()> {
//!     let body = cx.body().await?;
//!     cx.dataset::<usize>()?.write(body.len()).await?;
//!     cx.queue().follow("/next").await?;
//!     Ok(())
//! }
//!
//! # #[tokio::main(flavor = "current_thread")]
//! # async fn main() -> Result<()> {
//! let site = MockSite::new().page("https://example.com/", MockPage::html("<p>Hi</p>"));
//! let visit = site
//!     .visit(Seed::get("https://example.com/")?)
//!     .with_dataset(InMemDataset::<usize>::new());
//!
//! assert!(matches!(visit.run(&from_fn(page)).await, Outcome::Continue));
//! assert_eq!(visit.dataset::<usize>()?.read_all().await?, [9]);
//! assert_eq!(visit.queued()[0].uri(), "https://example.com/next");
//! assert_eq!(site.requests(), ["https://example.com/"]);
//! # Ok(())
//! # }
//! ```

mod page;
mod site;
mod visit;

pub use page::MockPage;
pub use site::{MockClient, MockSite};
pub use visit::Visit;

#[cfg(test)]
mod tests;
