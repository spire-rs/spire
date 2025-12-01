use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::sync::{Arc, Mutex};

use crate::Error;
use crate::dataset::utils::BoxCloneDataset;
use crate::dataset::{Dataset, DatasetExt, InMemDataset};

/// Type alias for a thread-safe, type-erased boxed value.
type AnyBox = Box<dyn Any + Send + Sync>;

/// A type-erased registry that stores multiple [`Dataset`] implementations,
/// indexed by their data type.
///
/// `DatasetRegistry` allows you to store different datasets for different types
/// in a single collection, retrieving them by type at runtime. This is useful
/// for managing multiple data streams in a web scraping context where different
/// handlers may produce different types of data.
///
/// # Examples
///
/// ```no_run
/// use spire_core::dataset::{DatasetRegistry, InMemDataset};
///
/// let registry = DatasetRegistry::new();
///
/// // Store different datasets for different types
/// registry.set(InMemDataset::<String>::queue());
/// registry.set(InMemDataset::<u64>::queue());
///
/// // Retrieve datasets by type
/// let string_dataset = registry.get::<String>();
/// let number_dataset = registry.get::<u64>();
/// ```
#[must_use]
#[derive(Clone, Default)]
pub struct DatasetRegistry {
    inner: Arc<Mutex<DatasetRegistryInner>>,
}

#[derive(Default)]
struct DatasetRegistryInner {
    container: HashMap<TypeId, AnyBox>,
}

impl DatasetRegistry {
    /// Creates an empty [`DatasetRegistry`].
    #[inline]
    pub fn new() -> Self {
        Self::default()
    }

    /// Inserts a [`Dataset`] into the registry for type `T`.
    ///
    /// If a dataset for this type already exists, it will be replaced.
    /// Note that items from the replaced dataset are not migrated to the new one.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use spire_core::dataset::{DatasetRegistry, InMemDataset};
    ///
    /// let registry = DatasetRegistry::new();
    /// registry.set(InMemDataset::<String>::queue());
    /// ```
    pub fn set<D, T, E>(&self, dataset: D)
    where
        D: Dataset<T, Error = E> + Clone + 'static,
        Error: From<E>,
        T: Send + Sync + 'static,
    {
        let type_id = TypeId::of::<T>();
        let dataset = Box::new(boxed(dataset));
        let mut guard = self.inner.lock().expect("DatasetRegistry mutex poisoned");
        let _ = guard.container.insert(type_id, dataset);
    }

    /// Attempts to retrieve a [`Dataset`] for type `T`.
    ///
    /// Returns `None` if no dataset for this type has been registered.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use spire_core::dataset::DatasetRegistry;
    ///
    /// let registry = DatasetRegistry::new();
    /// assert!(registry.try_get::<String>().is_none());
    /// ```
    pub fn try_get<T>(&self) -> Option<BoxCloneDataset<T, Error>>
    where
        T: Send + Sync + 'static,
    {
        let guard = self.inner.lock().expect("DatasetRegistry mutex poisoned");
        let dataset = guard.container.get(&TypeId::of::<T>());

        type Ds<T> = BoxCloneDataset<T, Error>;
        dataset.and_then(|x| x.downcast_ref::<Ds<T>>()).cloned()
    }

    /// Retrieves a [`Dataset`] for type `T`, creating a default one if it doesn't exist.
    ///
    /// If no dataset exists for type `T`, a FIFO [`InMemDataset`] is automatically
    /// created, inserted, and returned.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use spire_core::dataset::DatasetRegistry;
    ///
    /// let registry = DatasetRegistry::new();
    /// let dataset = registry.get::<String>(); // Creates InMemDataset if not present
    /// ```
    pub fn get<T>(&self) -> BoxCloneDataset<T, Error>
    where
        T: Send + Sync + 'static,
    {
        let mut guard = self.inner.lock().expect("DatasetRegistry mutex poisoned");
        let make_mem = || boxed(InMemDataset::<T>::queue());
        let dataset = match guard.container.entry(TypeId::of::<T>()) {
            Entry::Occupied(x) => x.into_mut(),
            Entry::Vacant(x) => x.insert(Box::new(make_mem())),
        };

        type Ds<T> = BoxCloneDataset<T, Error>;
        dataset
            .downcast_ref::<Ds<T>>()
            .expect("DatasetRegistry type mismatch: this is a bug")
            .clone()
    }

    /// Returns the total number of registered datasets.
    #[must_use]
    pub fn len(&self) -> usize {
        let guard = self.inner.lock().expect("DatasetRegistry mutex poisoned");
        guard.container.len()
    }

    /// Returns `true` if no datasets have been registered.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        let guard = self.inner.lock().expect("DatasetRegistry mutex poisoned");
        guard.container.is_empty()
    }
}

fn boxed<D, T, E>(dataset: D) -> BoxCloneDataset<T, Error>
where
    D: Dataset<T, Error = E> + Clone + 'static,
    Error: From<E>,
    T: Send + Sync + 'static,
{
    dataset.map_err(|x| Error::from(x)).boxed_clone()
}

#[cfg(test)]
mod test {
    use crate::dataset::{DatasetRegistry, InMemDataset};

    #[test]
    fn same_type() {
        let ds = DatasetRegistry::default();

        ds.set(InMemDataset::<u32>::new());
        assert!(ds.try_get::<u32>().is_some());
        assert_eq!(ds.len(), 1);

        ds.set(InMemDataset::<u32>::new());
        assert!(ds.try_get::<u32>().is_some());
        assert_eq!(ds.len(), 1);
    }

    #[test]
    fn different_type() {
        let ds = DatasetRegistry::default();

        ds.set(InMemDataset::<u32>::new());
        assert!(ds.try_get::<u32>().is_some());
        assert_eq!(ds.len(), 1);

        ds.set(InMemDataset::<u64>::new());
        assert!(ds.try_get::<u64>().is_some());
        assert_eq!(ds.len(), 2);
    }

    #[test]
    fn take_many() {
        let ds = DatasetRegistry::default();
        assert_eq!(ds.len(), 0);

        ds.set(InMemDataset::<u32>::new());
        assert_eq!(ds.len(), 1);
        assert!(ds.try_get::<u32>().is_some());
        assert!(ds.try_get::<u32>().is_some());
        assert_eq!(ds.len(), 1);
    }
}
