use std::fmt;

use derive_more::{Deref, DerefMut};

use super::{DataSink, DataStream};
use crate::Error;
use crate::dataset::DatasetExt;
use crate::dataset::utils::BoxCloneDataset;

/// Convenient wrapper around [`BoxCloneDataset`] for ergonomic dataset handling.
///
/// `Data` provides a simple, cloneable handle to a type-erased dataset with
/// convenient methods to convert into streams or sinks. This is the recommended
/// type for passing datasets around in application code.
///
/// # Examples
///
/// ```no_run
/// use spire_core::dataset::{Data, DatasetExt, InMemDataset};
///
/// let dataset = InMemDataset::<String>::queue().boxed_clone();
/// let data = Data::new(dataset);
///
/// // Clone and use in different contexts
/// let stream_data = data.clone();
/// let sink_data = data.clone();
///
/// let stream = stream_data.into_stream();
/// let sink = sink_data.into_sink();
/// ```
#[must_use]
#[derive(Clone, Deref, DerefMut)]
pub struct Data<T, E = Error>(pub BoxCloneDataset<T, E>)
where
    T: 'static,
    E: 'static;

impl<T, E> Data<T, E>
where
    T: Send + Sync + 'static,
    E: Send + Sync + 'static,
{
    /// Creates a new [`Data`].
    #[inline]
    pub const fn new(inner: BoxCloneDataset<T, E>) -> Self {
        Self(inner)
    }

    /// Returns a reference to the underlying [`BoxCloneDataset`].
    #[inline]
    pub const fn as_dataset(&self) -> &BoxCloneDataset<T, E> {
        &self.0
    }

    /// Returns a new [`DataStream`].
    #[inline]
    pub fn into_stream(self) -> DataStream<T, E> {
        self.0.into_stream()
    }

    /// Returns a new [`DataSink`].
    #[inline]
    pub fn into_sink(self) -> DataSink<T, E> {
        self.0.into_sink()
    }
}

impl<T> fmt::Debug for Data<T> {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Data").finish_non_exhaustive()
    }
}
