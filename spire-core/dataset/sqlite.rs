use std::marker::PhantomData;

use crate::dataset::{Dataset, Result};

#[derive(Debug)]
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

impl<T> Dataset<T> for SqliteDataset<T> {
    fn append(&self, data: T) -> Result<()> {
        todo!()
    }

    fn evict(&self) -> Option<T> {
        todo!()
    }

    fn len(&self) -> usize {
        todo!()
    }
}
