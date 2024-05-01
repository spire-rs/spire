use std::any::{Any, TypeId};
use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use crate::dataset::util::BoxCloneDataset;
use crate::dataset::{Dataset, DatasetExt, InMemDataset};
use crate::Error;

/// Type-erased collection of `Dataset`s.
#[must_use]
#[derive(Clone, Default)]
pub struct Datasets {
    inner: Arc<DatasetsInner>,
}

#[derive(Default)]
struct DatasetsInner {
    mx: Mutex<HashMap<TypeId, Box<dyn Any + Send + Sync>>>,
}

impl Datasets {
    /// Creates an empty collection of [`Dataset`]s.
    pub fn new() -> Self {
        Self::default()
    }

    /// Inserts the provided [`Dataset`].
    ///
    /// ### Note
    ///
    /// Replaces the dataset of the same type if it is already inserted.
    /// Does not move items from the replaced `Dataset`.
    pub fn set<D, T, E>(&self, dataset: D)
    where
        D: Dataset<T, Error = E> + Clone,
        Error: From<E>,
        T: Send + Sync + 'static,
    {
        let dataset = Box::new(boxed(dataset));
        let mut guard = self.inner.mx.lock().unwrap();
        let _ = guard.insert(TypeId::of::<T>(), dataset);
    }

    /// Returns the [`Dataset`] of the requested type.
    pub fn try_get<T>(&self) -> Option<BoxCloneDataset<T, Error>>
    where
        T: Send + Sync + 'static,
    {
        let guard = self.inner.mx.lock().unwrap();
        let dataset = guard.get(&TypeId::of::<T>());

        type Ds<T> = BoxCloneDataset<T, Error>;
        dataset.and_then(|x| x.downcast_ref::<Ds<T>>()).cloned()
    }

    /// Returns the [`Dataset`] of the requested type.
    ///
    /// ### Note
    ///
    /// Inserts and returns the `first-in first-out` [`InMemDataset`]
    /// if none were found.
    pub fn get<T>(&self) -> BoxCloneDataset<T, Error>
    where
        T: Send + Sync + 'static,
    {
        let mut guard = self.inner.mx.lock().unwrap();
        let make_mem = || boxed(InMemDataset::<T>::queue());
        let dataset = match guard.entry(TypeId::of::<T>()) {
            Entry::Occupied(x) => x.into_mut(),
            Entry::Vacant(x) => x.insert(Box::new(make_mem())),
        };

        type Ds<T> = BoxCloneDataset<T, Error>;
        dataset.downcast_ref::<Ds<T>>().cloned().unwrap()
    }

    /// Returns the total amount of inserted [`Dataset`]s.
    #[must_use]
    pub fn len(&self) -> usize {
        let guard = self.inner.mx.lock().unwrap();
        guard.len()
    }

    /// Returns `true` if no [`Dataset`]s were inserted.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

fn boxed<D, T, E>(dataset: D) -> BoxCloneDataset<T, Error>
where
    D: Dataset<T, Error = E> + Clone,
    Error: From<E>,
    T: Send + Sync + 'static,
{
    dataset.map_err(|x| Error::from(x)).boxed_clone()
}

#[cfg(test)]
mod test {
    use crate::dataset::{Datasets, InMemDataset};

    #[test]
    fn same_type() {
        let ds = Datasets::default();

        ds.set(InMemDataset::<u32>::new());
        assert!(ds.try_get::<u32>().is_some());
        assert_eq!(ds.len(), 1);

        ds.set(InMemDataset::<u32>::new());
        assert!(ds.try_get::<u32>().is_some());
        assert_eq!(ds.len(), 1);
    }

    #[test]
    fn different_type() {
        let ds = Datasets::default();

        ds.set(InMemDataset::<u32>::new());
        assert!(ds.try_get::<u32>().is_some());
        assert_eq!(ds.len(), 1);

        ds.set(InMemDataset::<u64>::new());
        assert!(ds.try_get::<u64>().is_some());
        assert_eq!(ds.len(), 2);
    }

    #[test]
    fn take_many() {
        let ds = Datasets::default();
        assert_eq!(ds.len(), 0);

        ds.set(InMemDataset::<u32>::new());
        assert_eq!(ds.len(), 1);
        assert!(ds.try_get::<u32>().is_some());
        assert!(ds.try_get::<u32>().is_some());
        assert_eq!(ds.len(), 1);
    }
}
