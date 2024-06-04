use std::fmt;

use crate::dataset::Dataset;

/// Transforms the error type of the [`Dataset`] for a [`map_err`] method.
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
    pub const fn new(inner: D, f: F) -> Self {
        Self { inner, f }
    }
}

impl<D, F> fmt::Debug for MapErr<D, F>
where
    D: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&self.inner, f)
    }
}

#[async_trait::async_trait]
impl<T, D, F, E2> Dataset<T> for MapErr<D, F>
where
    T: Send + Sync + 'static,
    D: Dataset<T>,
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
