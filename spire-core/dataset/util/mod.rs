//! Various utility [`Dataset`]s.

pub use boxed::BoxDataset;
pub use boxed_clone::BoxCloneDataset;
pub use map_data::MapData;
pub use map_err::MapErr;

use crate::dataset::Dataset;

mod boxed;
mod boxed_clone;
mod map_data;
mod map_err;

/// Extension trait for [`Dataset`]s that provides a set of adapters.
pub trait DatasetExt<T>: Dataset<T> {
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
    D: Dataset<T>,
{
    fn boxed(self) -> BoxDataset<T, Self::Error> {
        BoxDataset::new(self)
    }

    fn boxed_clone(self) -> BoxCloneDataset<T, Self::Error>
    where
        Self: Clone,
    {
        BoxCloneDataset::new(self)
    }

    fn map_data<F, F2>(self, to: F, from: F2) -> MapData<Self, F, F2>
    where
        Self: Sized,
    {
        MapData::new(self, to, from)
    }

    fn map_err<F>(self, from: F) -> MapErr<Self, F>
    where
        Self: Sized,
    {
        MapErr::new(self, from)
    }
}
