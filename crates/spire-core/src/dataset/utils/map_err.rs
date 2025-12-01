use std::fmt;

use crate::dataset::Dataset;

/// Adapter that transforms the error type of a [`Dataset`].
///
/// `MapErr` wraps an existing [`Dataset`] and converts its error type using a
/// provided function. This is useful for adapting datasets to work with different
/// error types in your application, enabling better error composition and handling.
///
/// # Examples
///
/// ## Converting to a Custom Error Type
///
/// ```ignore
/// use spire_core::dataset::{Dataset, DatasetExt, InMemDataset};
/// use std::convert::Infallible;
///
/// #[derive(Debug)]
/// enum AppError {
///     Database(String),
/// }
///
/// # async fn example() -> Result<(), AppError> {
/// let dataset = InMemDataset::<String>::queue()
///     .map_err(|_: Infallible| AppError::Database("impossible".into()));
///
/// dataset.write("data".to_string()).await?;
/// # Ok(())
/// # }
/// ```
///
/// ## Converting to anyhow::Error
///
/// ```ignore
/// use spire_core::dataset::{Dataset, DatasetExt, InMemDataset};
/// use anyhow::Error;
///
/// # async fn example() -> Result<(), Error> {
/// let dataset = InMemDataset::<i32>::queue()
///     .map_err(|e| anyhow::anyhow!("Dataset error: {:?}", e));
///
/// dataset.write(42).await?;
/// # Ok(())
/// # }
/// ```
///
/// ## Chaining Error Transformations
///
/// ```ignore
/// use spire_core::dataset::{Dataset, DatasetExt, InMemDataset};
///
/// #[derive(Debug)]
/// struct IoError(String);
///
/// #[derive(Debug)]
/// struct AppError(String);
///
/// # async fn example() -> Result<(), AppError> {
/// let dataset = InMemDataset::<String>::queue()
///     .map_err(|_| IoError("io error".into()))
///     .map_err(|e: IoError| AppError(format!("app error: {:?}", e)));
///
/// dataset.write("data".to_string()).await?;
/// # Ok(())
/// # }
/// ```
///
/// ## Making Infallible Errors Explicit
///
/// ```ignore
/// use spire_core::dataset::{Dataset, DatasetExt, InMemDataset};
/// use std::convert::Infallible;
///
/// # async fn example() -> Result<(), String> {
/// // InMemDataset is infallible, but we want a String error type
/// let dataset = InMemDataset::<i32>::queue()
///     .map_err(|_: Infallible| "This will never happen".to_string());
///
/// match dataset.read().await {
///     Ok(Some(data)) => println!("Got: {}", data),
///     Ok(None) => println!("Empty"),
///     Err(e) => println!("Error: {}", e), // Never reached
/// }
/// # Ok(())
/// # }
/// ```
///
/// [`map_err`]: crate::dataset::DatasetExt::map_err
#[must_use]
#[derive(Clone)]
pub struct MapErr<D, F> {
    inner: D,
    f: F,
}

impl<D, F> MapErr<D, F> {
    /// Creates a new [`MapErr`].
    #[inline]
    pub const fn new(inner: D, f: F) -> Self {
        Self { inner, f }
    }
}

impl<D, F> fmt::Debug for MapErr<D, F>
where
    D: fmt::Debug,
{
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&self.inner, f)
    }
}

#[async_trait::async_trait]
impl<T, D, F, E2> Dataset<T> for MapErr<D, F>
where
    T: Send + Sync + 'static,
    D: Dataset<T> + 'static,
    F: Fn(D::Error) -> E2 + Send + Sync + 'static,
{
    type Error = E2;

    async fn write(&self, data: T) -> Result<(), Self::Error> {
        self.inner.write(data).await.map_err(&self.f)
    }

    async fn read(&self) -> Result<Option<T>, Self::Error> {
        self.inner.read().await.map_err(&self.f)
    }

    #[inline]
    fn len(&self) -> usize {
        self.inner.len()
    }
}
