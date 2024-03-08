use std::marker::PhantomData;

use crate::dataset::Dataset;

#[derive(Debug, Clone)]
pub struct MapData<T, D, F, F2> {
    inner: D,
    marker: PhantomData<T>,
    f_to_inner: F,
    f_from_inner: F2,
}

impl<T, D, F, F2> MapData<T, D, F, F2> {
    pub fn new(inner: D, to: F, from: F2) -> Self {
        Self {
            inner,
            marker: PhantomData,
            f_to_inner: to,
            f_from_inner: from,
        }
    }
}

#[async_trait::async_trait]
impl<T, T2, D, F, F2> Dataset<T2> for MapData<T, D, F, F2>
where
    T: Send + Sync + 'static,
    T2: Send + Sync + 'static,
    D: Dataset<T>,
    F: FnOnce(T2) -> T + Clone + Send + Sync + 'static,
    F2: FnOnce(T) -> T2 + Clone + Send + Sync + 'static,
{
    type Error = D::Error;

    async fn add(&self, data: T2) -> Result<(), Self::Error> {
        let data = self.f_to_inner.clone()(data);
        self.inner.add(data).await
    }

    async fn get(&self) -> Result<Option<T2>, Self::Error> {
        let data = self.inner.get().await;
        data.map(|x| x.map(self.f_from_inner.clone()))
    }

    fn len(&self) -> usize {
        self.inner.len()
    }
}

#[derive(Debug, Clone)]
pub struct MapErr<D, F> {
    inner: D,
    f: F,
}

impl<D, F> MapErr<D, F> {
    pub fn new(inner: D, f: F) -> Self {
        Self { inner, f }
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
