use std::fmt;

use crate::dataset::Dataset;

#[derive(Clone)]
pub struct MapErr<D, F> {
    inner: D,
    f: F,
}

impl<D, F> MapErr<D, F> {
    /// Creates a new [`MapErr`].
    pub fn new(inner: D, f: F) -> Self {
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
    F: FnOnce(D::Error) -> E2 + Clone + Send + Sync + 'static,
{
    type Error = E2;

    async fn add(&self, data: T) -> Result<(), Self::Error> {
        self.inner.add(data).await.map_err(self.f.clone())
    }

    async fn get(&self) -> Result<Option<T>, Self::Error> {
        self.inner.get().await.map_err(self.f.clone())
    }

    fn len(&self) -> usize {
        self.inner.len()
    }
}
