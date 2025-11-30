//! Various utility [`Dataset`]s.

pub use boxed::BoxDataset;
pub use boxed_clone::BoxCloneDataset;
pub use map_data::MapData;
pub use map_err::MapErr;

use crate::dataset::future::{DataSink, DataStream};
use crate::dataset::Dataset;

mod boxed;
mod boxed_clone;
mod map_data;
mod map_err;

/// Extension trait for [`Dataset`]s that provides a set of adapters.
pub trait DatasetExt<T>: Dataset<T> {
    /// Returns both [`DataSink`] and [`DataStream`].
    fn into_split(self) -> (DataSink<T, Self::Error>, DataStream<T, Self::Error>)
    where
        Self: Sized + Clone,
        T: Send + 'static,
        Self::Error: Send + 'static;

    /// Returns a new [`DataStream`].
    fn into_stream(self) -> DataStream<T, Self::Error>
    where
        Self: Sized,
        T: Send + 'static,
        Self::Error: Send + 'static;

    /// Returns a new [`DataSink`].
    fn into_sink(self) -> DataSink<T, Self::Error>
    where
        Self: Sized,
        T: Send + 'static,
        Self::Error: Send + 'static;

    /// Wraps `self` in a `BoxDataset` for type-erased [`Dataset`].
    fn boxed(self) -> BoxDataset<T, Self::Error>;

    /// Wraps `self` in a `BoxCloneDataset` for cloneable, type-erased [`Dataset`].
    fn boxed_clone(self) -> BoxCloneDataset<T, Self::Error>
    where
        Self: Clone;

    /// Transforms data before storing using `to` fn and after retrieving using `from` fn.
    fn map_data<F, F2>(self, to: F, from: F2) -> MapData<Self, F, F2>
    where
        Self: Sized;

    /// Converts errors using a `from` function.
    fn map_err<F>(self, from: F) -> MapErr<Self, F>
    where
        Self: Sized;
}

impl<T, D> DatasetExt<T> for D
where
    D: Dataset<T> + 'static,
{
    /// Returns both [`DataSink`] and [`DataStream`].
    #[inline]
    fn into_split(self) -> (DataSink<T, Self::Error>, DataStream<T, Self::Error>)
    where
        Self: Sized + Clone,
        T: Send + 'static,
        Self::Error: Send + 'static,
    {
        (self.clone().into_sink(), self.into_stream())
    }

    /// Returns a new [`DataStream`].
    #[inline]
    fn into_stream(self) -> DataStream<T, Self::Error>
    where
        Self: Sized,
        T: Send + 'static,
        Self::Error: Send + 'static,
    {
        DataStream::new(self)
    }

    /// Returns a new [`DataSink`].
    #[inline]
    fn into_sink(self) -> DataSink<T, Self::Error>
    where
        Self: Sized,
        T: Send + 'static,
        Self::Error: Send + 'static,
    {
        DataSink::new(self)
    }

    #[inline]
    fn boxed(self) -> BoxDataset<T, Self::Error>
    where
        Self: Send + Sync + 'static,
    {
        BoxDataset::new(self)
    }

    #[inline]
    fn boxed_clone(self) -> BoxCloneDataset<T, Self::Error>
    where
        Self: Clone + Send + Sync + 'static,
    {
        BoxCloneDataset::new(self)
    }

    #[inline]
    fn map_data<F, F2>(self, to: F, from: F2) -> MapData<Self, F, F2>
    where
        Self: Sized + 'static,
    {
        MapData::new(self, to, from)
    }

    #[inline]
    fn map_err<F>(self, from: F) -> MapErr<Self, F>
    where
        Self: Sized + 'static,
    {
        MapErr::new(self, from)
    }
}
