use std::fmt;
use std::marker::PhantomData;

use crate::dataset::{Dataset, Result};

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
        todo!()
    }
}

#[async_trait::async_trait]
impl<T> Dataset<T> for PersyDataset<T>
where
    T: Send + Sync + 'static,
{
    async fn append(&self, data: T) -> Result<()> {
        todo!()
    }

    async fn evict(&self) -> Option<T> {
        todo!()
    }

    fn len(&self) -> usize {
        todo!()
    }
}
