use std::any::{Any, TypeId};
use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use crate::dataset::util::{BoxCloneDataset, DatasetExt};
use crate::dataset::{Dataset, InMemDataset};
use crate::{BoxError, Error};

#[derive(Clone, Default)]
pub struct Datasets {
    // TODO: Rc -> Arc, + Send + Sync?
    inner: Arc<DatasetsInner>,
}

#[derive(Default)]
struct DatasetsInner {
    ds: Mutex<HashMap<TypeId, Box<dyn Any + Send + Sync>>>,
}

impl Datasets {
    pub fn set<D, T, E>(&self, dataset: D)
    where
        D: Dataset<T, Error = E> + Clone,
        E: Into<BoxError>,
        T: Send + Sync + 'static,
    {
        let dataset = Box::new(boxed(dataset));
        let mut guard = self.inner.ds.lock().unwrap();
        let _ = guard.insert(TypeId::of::<T>(), dataset);
    }

    pub fn try_get<T>(&self) -> Option<BoxCloneDataset<T, Error>>
    where
        T: Send + Sync + 'static,
    {
        let fn_dataset = || boxed(InMemDataset::<T>::queue());

        let mut guard = self.inner.ds.lock().unwrap();
        let dataset = match guard.entry(TypeId::of::<T>()) {
            Entry::Occupied(x) => x.into_mut(),
            Entry::Vacant(x) => x.insert(Box::new(fn_dataset())),
        };

        type Ds<T> = BoxCloneDataset<T, Error>;
        dataset.downcast_ref::<Ds<T>>().cloned()
    }

    pub fn get<T>(&self) -> BoxCloneDataset<T, Error>
    where
        T: Send + Sync + 'static,
    {
        self.try_get::<T>().unwrap()
    }

    pub fn len(&self) -> usize {
        let guard = self.inner.ds.lock().unwrap();
        guard.len()
    }
}

fn boxed<D, T, E>(dataset: D) -> BoxCloneDataset<T, Error>
where
    D: Dataset<T, Error = E> + Clone,
    E: Into<BoxError>,
    T: Send + Sync + 'static,
{
    let f = |x: E| Error::new(x);
    dataset.map_err(f).boxed_clone()
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
    fn diff_type() {
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
