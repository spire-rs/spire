use std::fmt;

use crate::dataset::Dataset;

/// Adapter that transforms data types during dataset read and write operations.
///
/// `MapData` wraps an existing [`Dataset`] and applies bidirectional transformations:
/// - `to_inner`: Converts data before writing to the underlying dataset
/// - `from_inner`: Converts data after reading from the underlying dataset
///
/// This is useful for adapting datasets to work with different data types without
/// changing the underlying storage format.
///
/// # Examples
///
/// ## Basic Type Conversion
///
/// ```ignore
/// use spire_core::dataset::{Dataset, DatasetExt, InMemDataset};
///
/// # async fn example() -> Result<(), std::convert::Infallible> {
/// // Store integers as strings internally
/// let dataset = InMemDataset::<String>::queue()
///     .map_data(
///         |num: i32| num.to_string(),
///         |s: String| s.parse().unwrap(),
///     );
///
/// dataset.write(42).await?;
/// assert_eq!(dataset.read().await?, Some(42));
/// # Ok(())
/// # }
/// ```
///
/// ## Data Normalization
///
/// ```ignore
/// use spire_core::dataset::{Dataset, DatasetExt, InMemDataset};
///
/// # async fn example() -> Result<(), std::convert::Infallible> {
/// // Normalize strings to uppercase when storing
/// let dataset = InMemDataset::<String>::queue()
///     .map_data(
///         |s: &str| s.to_uppercase(),
///         |s: String| s, // Keep as-is when reading
///     );
///
/// dataset.write("hello").await?;
/// assert_eq!(dataset.read().await?, Some("HELLO".to_string()));
/// # Ok(())
/// # }
/// ```
///
/// ## Serialization/Deserialization
///
/// ```ignore
/// use spire_core::dataset::{Dataset, DatasetExt, InMemDataset};
/// use serde::{Deserialize, Serialize};
///
/// #[derive(Serialize, Deserialize)]
/// struct User {
///     name: String,
///     age: u32,
/// }
///
/// # async fn example() -> Result<(), std::convert::Infallible> {
/// // Store User objects as JSON strings
/// let dataset = InMemDataset::<String>::queue()
///     .map_data(
///         |user: User| serde_json::to_string(&user).unwrap(),
///         |json: String| serde_json::from_str(&json).unwrap(),
///     );
///
/// dataset.write(User { name: "Alice".into(), age: 30 }).await?;
/// let user = dataset.read().await?.unwrap();
/// assert_eq!(user.name, "Alice");
/// # Ok(())
/// # }
/// ```
///
/// [`map_data`]: crate::dataset::DatasetExt::map_data
#[must_use]
#[derive(Clone)]
pub struct MapData<D, F, F2> {
    inner: D,
    to_inner: F,
    from_inner: F2,
}

impl<D, F, F2> MapData<D, F, F2> {
    /// Creates a new [`MapData`].
    #[inline]
    pub const fn new(inner: D, to: F, from: F2) -> Self {
        Self {
            inner,
            to_inner: to,
            from_inner: from,
        }
    }
}

impl<D, F, F2> fmt::Debug for MapData<D, F, F2>
where
    D: fmt::Debug,
{
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&self.inner, f)
    }
}

#[async_trait::async_trait]
impl<D, F, F2, T, T2> Dataset<T2> for MapData<D, F, F2>
where
    T: Send + Sync + 'static,
    T2: Send + Sync + 'static,
    D: Dataset<T> + 'static,
    F: Fn(T2) -> T + Send + Sync + 'static,
    F2: Fn(T) -> T2 + Send + Sync + 'static,
{
    type Error = D::Error;

    async fn write(&self, data: T2) -> Result<(), Self::Error> {
        let data = (self.to_inner)(data);
        self.inner.write(data).await
    }

    async fn read(&self) -> Result<Option<T2>, Self::Error> {
        let data = self.inner.read().await;
        data.map(|x| x.map(&self.from_inner))
    }

    #[inline]
    fn len(&self) -> usize {
        self.inner.len()
    }
}
