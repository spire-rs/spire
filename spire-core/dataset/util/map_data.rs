use std::fmt;

use crate::dataset::Dataset;

/// Transforms the data type of the [`Dataset`] for a [`map_data`] method.
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
