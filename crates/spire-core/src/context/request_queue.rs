//! Request queue for managing web scraping tasks.
//!
//! This module provides the [`RequestQueue`] type for managing HTTP requests
//! in a web scraping pipeline.

use std::fmt;
use std::num::NonZeroU32;

use crate::context::{Depth, Request, RequestSource, Tag};
use crate::dataset::Dataset;
use crate::dataset::utils::BoxCloneDataset;
use crate::{Error, Result};

/// [`Request`] queue backed by a [`Dataset`].
///
/// The queue manages requests and their metadata (tags, depth) for processing
/// by the scraping engine. It can optionally attach default tags and depths
/// to all requests added to the queue.
///
/// # Examples
///
/// ## Basic usage
///
/// ```no_run
/// use spire_core::context::RequestQueue;
/// use spire_core::dataset::InMemDataset;
/// use spire_core::dataset::utils::DatasetExt;
/// use std::num::NonZeroU32;
///
/// # async fn example() -> Result<(), spire_core::Error> {
/// # let dataset = InMemDataset::queue().map_err(Into::into).boxed_clone();
/// # let queue = RequestQueue::new(dataset, NonZeroU32::new(1).unwrap());
/// // Add requests to the queue
/// queue.append("https://example.com").await?;
/// queue.branch("https://example.com/page2").await?;
/// # Ok(())
/// # }
/// ```
///
/// ## With tags
///
/// ```no_run
/// use spire_core::context::{RequestQueue, Tag};
/// use spire_core::dataset::InMemDataset;
/// use spire_core::dataset::utils::DatasetExt;
/// use std::num::NonZeroU32;
///
/// # async fn example() -> Result<(), spire_core::Error> {
/// # let dataset = InMemDataset::queue().map_err(Into::into).boxed_clone();
/// # let queue = RequestQueue::new(dataset, NonZeroU32::new(1).unwrap());
/// // Add requests with specific tags
/// queue.append_with_tag(Tag::from("list"), "https://example.com/list").await?;
/// queue.branch_with_tag(Tag::from("detail"), "https://example.com/details").await?;
/// # Ok(())
/// # }
/// ```
///
/// ## With defaults
///
/// ```no_run
/// use spire_core::context::{RequestQueue, Tag, Depth};
/// use spire_core::dataset::InMemDataset;
/// use spire_core::dataset::utils::DatasetExt;
/// use std::num::NonZeroU32;
///
/// # async fn example() -> Result<(), spire_core::Error> {
/// # let dataset = InMemDataset::queue().map_err(Into::into).boxed_clone();
/// // Create a queue with default tag and depth
/// let queue = RequestQueue::new(dataset, NonZeroU32::new(1).unwrap())
///     .with_default_tag(Tag::from("crawl"))
///     .with_default_depth(Depth::new(2));
///
/// // All requests will inherit the defaults
/// queue.append("https://example.com").await?;
/// # Ok(())
/// # }
/// ```
///
/// See [`Client::with_request_queue`].
///
/// [`Dataset`]: crate::dataset::Dataset
/// [`Client::with_request_queue`]: crate::Client::with_request_queue
#[must_use]
#[derive(Clone)]
pub struct RequestQueue {
    inner: BoxCloneDataset<Request, Error>,
    depth: NonZeroU32,
    default_tag: Option<Tag>,
    default_depth: Option<Depth>,
}

impl RequestQueue {
    /// Creates a new [`RequestQueue`].
    pub const fn new(inner: BoxCloneDataset<Request, Error>, depth: NonZeroU32) -> Self {
        Self {
            inner,
            depth,
            default_tag: None,
            default_depth: None,
        }
    }

    /// Creates a new [`RequestQueue`] with a default tag that will be attached to all requests.
    pub fn with_default_tag(mut self, tag: Tag) -> Self {
        self.default_tag = Some(tag);
        self
    }

    /// Creates a new [`RequestQueue`] with a default depth that will be attached to all requests.
    pub fn with_default_depth(mut self, depth: Depth) -> Self {
        self.default_depth = Some(depth);
        self
    }

    /// Inserts another [`Request`] into the queue.
    ///
    /// This method will attach the queue's default tag and depth if they are set,
    /// but will not override existing extensions on the request.
    pub async fn append<S>(&self, source: S) -> Result<()>
    where
        S: TryInto<RequestSource>,
        S::Error: Into<Error>,
    {
        let source: RequestSource = source.try_into().map_err(Into::into)?;
        let mut request: Request = source.try_into()?;
        self.apply_defaults(&mut request);
        self.inner.write(request).await
    }

    /// Inserts another [`Request`] into the queue with an increased depth.
    ///
    /// This method increases the request's depth by 1 compared to the queue's current depth,
    /// and applies default tag if set.
    pub async fn branch<S>(&self, source: S) -> Result<()>
    where
        S: TryInto<RequestSource>,
        S::Error: Into<Error>,
    {
        let source: RequestSource = source.try_into().map_err(Into::into)?;
        let mut request: Request = source.try_into()?;

        // Set increased depth
        let new_depth = Depth::new(self.depth.saturating_add(1).into());
        let _ = request.extensions_mut().get_or_insert_with(|| new_depth);

        // Apply default tag if set
        if let Some(ref default_tag) = self.default_tag {
            let _ = request
                .extensions_mut()
                .get_or_insert_with(|| default_tag.clone());
        }

        self.inner.write(request).await
    }

    /// Inserts another [`Request`] into the queue with a specific tag.
    ///
    /// The tag will override any existing tag on the request, but default depth
    /// will still be applied if set and not already present.
    pub async fn append_with_tag<S>(&self, tag: impl Into<Tag>, source: S) -> Result<()>
    where
        S: TryInto<RequestSource>,
        S::Error: Into<Error>,
    {
        let source: RequestSource = source.try_into().map_err(Into::into)?;
        let mut request: Request = source.try_into()?;
        request.extensions_mut().insert(tag.into());

        // Apply default depth if set and not present
        if let Some(ref default_depth) = self.default_depth {
            let _ = request
                .extensions_mut()
                .get_or_insert_with(|| *default_depth);
        }

        self.inner.write(request).await
    }

    /// Inserts another [`Request`] into the queue with a specific tag and increased depth.
    ///
    /// This combines the behavior of `branch` and `append_with_tag` - it sets the
    /// provided tag and increases the depth by 1.
    pub async fn branch_with_tag<S>(&self, tag: impl Into<Tag>, source: S) -> Result<()>
    where
        S: TryInto<RequestSource>,
        S::Error: Into<Error>,
    {
        let source: RequestSource = source.try_into().map_err(Into::into)?;
        let mut request: Request = source.try_into()?;
        request.extensions_mut().insert(tag.into());

        // Always set increased depth for branching
        let new_depth = Depth::new(self.depth.saturating_add(1).into());
        request.extensions_mut().insert(new_depth);

        self.inner.write(request).await
    }

    /// Applies the queue's default tag and depth to a request if they are set
    /// and not already present on the request.
    fn apply_defaults(&self, request: &mut Request) {
        // Apply default tag if set and not present
        if let Some(ref default_tag) = self.default_tag {
            let _ = request
                .extensions_mut()
                .get_or_insert_with(|| default_tag.clone());
        }

        // Apply default depth if set and not present
        if let Some(ref default_depth) = self.default_depth {
            let _ = request
                .extensions_mut()
                .get_or_insert_with(|| *default_depth);
        }
    }
}

impl fmt::Debug for RequestQueue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RequestQueue")
            .field("depth", &self.depth)
            .field("has_default_tag", &self.default_tag.is_some())
            .field("has_default_depth", &self.default_depth.is_some())
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dataset::InMemDataset;

    fn create_test_queue() -> RequestQueue {
        use crate::dataset::utils::DatasetExt;
        let dataset = InMemDataset::queue().map_err(Into::into).boxed_clone();
        RequestQueue::new(dataset, NonZeroU32::new(1).unwrap())
    }

    #[tokio::test]
    async fn test_append_with_url() {
        let queue = create_test_queue();
        let result = queue.append("https://example.com").await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_branch_with_url() {
        let queue = create_test_queue();
        let result = queue.branch("https://example.com").await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_append_with_tag() {
        let queue = create_test_queue();
        let result = queue
            .append_with_tag(Tag::from("test"), "https://example.com")
            .await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_branch_with_tag() {
        let queue = create_test_queue();
        let result = queue
            .branch_with_tag(Tag::from("test"), "https://example.com")
            .await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_with_default_tag() {
        let queue = create_test_queue().with_default_tag(Tag::from("default"));
        let result = queue.append("https://example.com").await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_with_default_depth() {
        let queue = create_test_queue().with_default_depth(Depth::new(5));
        let result = queue.append("https://example.com").await;
        assert!(result.is_ok());
    }

    #[test]
    fn test_debug_display() {
        let queue = create_test_queue()
            .with_default_tag(Tag::from("test"))
            .with_default_depth(Depth::new(3));
        let debug_str = format!("{:?}", queue);
        assert!(debug_str.contains("RequestQueue"));
        assert!(debug_str.contains("has_default_tag: true"));
        assert!(debug_str.contains("has_default_depth: true"));
    }

    #[tokio::test]
    async fn test_try_into_ergonomics() {
        let queue = create_test_queue();

        // Test with &str
        let result1 = queue.append("https://example.com").await;
        assert!(result1.is_ok());

        // Test with String
        let url = String::from("https://example.com/page");
        let result2 = queue.branch(url).await;
        assert!(result2.is_ok());

        // Test with Request
        let request = Request::builder()
            .uri("https://example.com/api")
            .body(crate::context::Body::default())
            .unwrap();
        let result3 = queue.append_with_tag(Tag::from("api"), request).await;
        assert!(result3.is_ok());

        // Test mixed types in different methods
        let result4 = queue
            .branch_with_tag(Tag::from("detail"), "https://example.com/detail")
            .await;
        assert!(result4.is_ok());
    }
}
