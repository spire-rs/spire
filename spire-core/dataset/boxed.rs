use std::fmt;

use crate::dataset::Dataset;

pub struct BoxDataset<T> {
    inner: Box<dyn Dataset<T>>,
}

impl<T> BoxDataset<T> {
    pub fn new<D>(dataset: D) -> Self
    where
        D: Dataset<T> + 'static,
    {
        Self {
            inner: Box::new(dataset),
        }
    }
}

impl<T> fmt::Debug for BoxDataset<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        todo!()
    }
}

impl<T> Clone for BoxDataset<T> {
    fn clone(&self) -> Self {
        todo!()
    }
}

impl<T> Dataset<T> for BoxDataset<T> {
    fn append(&self, data: T) -> crate::dataset::Result<()> {
        todo!()
    }

    fn evict(&self) -> Option<T> {
        todo!()
    }

    fn len(&self) -> usize {
        todo!()
    }
}
