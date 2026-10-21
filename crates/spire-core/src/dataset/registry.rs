use std::any::{Any, TypeId, type_name};
use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;

use crate::dataset::{Data, Dataset};
use crate::{Error, ErrorKind, Result};

/// The datasets of a crawl, at most one per item type.
#[derive(Clone, Default)]
pub(crate) struct Datasets {
    map: HashMap<TypeId, Arc<dyn Any + Send + Sync>>,
}

impl Datasets {
    /// Registers `dataset` for items of type `T`, replacing any previous one.
    pub(crate) fn insert<T, D>(&mut self, dataset: D)
    where
        T: Send + 'static,
        D: Dataset<T>,
    {
        self.map
            .insert(TypeId::of::<T>(), Arc::new(Data::new(dataset)));
    }

    /// Returns the dataset for items of type `T`.
    pub(crate) fn get<T: Send + 'static>(&self) -> Result<Data<T>> {
        self.map
            .get(&TypeId::of::<T>())
            .and_then(|data| data.downcast_ref::<Data<T>>())
            .cloned()
            .ok_or_else(|| {
                let message = format!("no dataset registered for `{}`", type_name::<T>());
                Error::new(ErrorKind::Dataset).with_message(message)
            })
    }
}

impl fmt::Debug for Datasets {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Datasets")
            .field("len", &self.map.len())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dataset::InMemDataset;

    #[tokio::test]
    async fn typed_lookup() {
        let mut datasets = Datasets::default();
        datasets.insert(InMemDataset::<u32>::new());

        datasets.get::<u32>().unwrap().write(1).await.unwrap();
        assert_eq!(
            datasets.get::<u32>().unwrap().read().await.unwrap(),
            Some(1)
        );

        let error = datasets.get::<String>().unwrap_err();
        assert_eq!(error.kind(), ErrorKind::Dataset);
        assert!(error.to_string().contains("String"));
    }
}
