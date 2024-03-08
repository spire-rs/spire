use std::fmt;
use std::marker::PhantomData;

use crate::dataset::{Dataset, Result};

pub struct SqliteDataset<T> {
    marker: PhantomData<T>,
}

impl<T> SqliteDataset<T> {
    pub fn new() -> Self {
        todo!()
    }
}

impl<T> Clone for SqliteDataset<T> {
    fn clone(&self) -> Self {
        todo!()
    }
}

impl<T> fmt::Debug for SqliteDataset<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        todo!()
    }
}

#[async_trait::async_trait]
impl<T> Dataset<T> for SqliteDataset<T>
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
