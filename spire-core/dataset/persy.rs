use std::fmt;
use std::marker::PhantomData;

use crate::dataset::Dataset;
use crate::BoxError;

pub struct PersyDataset<T> {
    marker: PhantomData<T>,
}

impl<T> PersyDataset<T> {
    pub fn new() -> Self {
        todo!()
    }
}

impl<T> Clone for PersyDataset<T> {
    fn clone(&self) -> Self {
        todo!()
    }
}

impl<T> fmt::Debug for PersyDataset<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PersyDataset").finish_non_exhaustive()
    }
}

#[async_trait::async_trait]
impl<T> Dataset<T> for PersyDataset<T>
where
    T: Send + Sync + 'static,
{
    type Error = BoxError;

    async fn add(&self, data: T) -> Result<(), Self::Error> {
        todo!()
    }

    async fn get(&self) -> Result<Option<T>, Self::Error> {
        todo!()
    }

    fn len(&self) -> usize {
        todo!()
    }
}
