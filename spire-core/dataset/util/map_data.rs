use std::fmt;

use crate::dataset::Dataset;

/// Transforms the data type of the [`Dataset`]
#[derive(Clone)]
pub struct MapData<D, F, F2> {
    inner: D,
    f_2inner: F,
    f_inner2: F2,
}

impl<D, F, F2> MapData<D, F, F2> {
    /// Creates a new [`MapData`].
    pub fn new(inner: D, to: F, from: F2) -> Self {
        Self {
            inner,
            f_2inner: to,
            f_inner2: from,
        }
    }
}

impl<D, F, F2> fmt::Debug for MapData<D, F, F2>
where
    D: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&self.inner, f)
    }
}

#[async_trait::async_trait]
impl<D, F, F2, T, T2> Dataset<T2> for MapData<D, F, F2>
where
    T: Send + Sync + 'static,
    T2: Send + Sync + 'static,
    D: Dataset<T>,
    F: FnOnce(T2) -> T + Clone + Send + Sync + 'static,
    F2: FnOnce(T) -> T2 + Clone + Send + Sync + 'static,
{
    type Error = D::Error;

    async fn add(&self, data: T2) -> Result<(), Self::Error> {
        let data = self.f_2inner.clone()(data);
        self.inner.add(data).await
    }

    async fn get(&self) -> Result<Option<T2>, Self::Error> {
        let data = self.inner.get().await;
        data.map(|x| x.map(self.f_inner2.clone()))
    }

    fn len(&self) -> usize {
        self.inner.len()
    }
}
