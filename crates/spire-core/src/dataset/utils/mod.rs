//! Utility types and adapters for working with [`Dataset`]s.
//!
//! This module provides extension traits and wrapper types that enhance dataset
//! functionality through composition and type transformations.
//!
//! # Core Types
//!
//! ## Type Erasure
//!
//! - [`BoxDataset`] - Type-erased dataset for heterogeneous collections
//! - [`BoxCloneDataset`] - Cloneable type-erased dataset for shared ownership
//!
//! ## Transformation Adapters
//!
//! - [`MapData`] - Transform data types during read/write operations
//! - [`MapErr`] - Convert error types between implementations
//!
//! ## Extension Trait
//!
//! - [`DatasetExt`] - Extension methods for all dataset implementations
//!
//! # Examples
//!
//! ## Type Erasure
//!
//! ```no_run
//! use spire_core::dataset::{DatasetExt, InMemDataset};
//!
//! // Store different dataset types in a vector
//! let datasets: Vec<Box<dyn Dataset<String>>> = vec![
//!     InMemDataset::queue().boxed(),
//!     InMemDataset::stack().boxed(),
//! ];
//! ```
//!
//! ## Data Transformation
//!
//! ```no_run
//! use spire_core::dataset::{Dataset, DatasetExt, InMemDataset};
//!
//! // Store URLs as normalized lowercase
//! let dataset = InMemDataset::<String>::queue()
//!     .map_data(
//!         |url: &str| url.to_lowercase(),
//!         |url: String| url,
//!     );
//! ```

use crate::dataset::Dataset;
use crate::dataset::future::{DataSink, DataStream};

mod boxed;
mod boxed_clone;
mod map_data;
mod map_err;

pub use boxed::BoxDataset;
pub use boxed_clone::BoxCloneDataset;
pub use map_data::MapData;
pub use map_err::MapErr;

/// Extension trait providing adapter methods for all [`Dataset`] implementations.
///
/// This trait is automatically implemented for all types that implement [`Dataset`],
/// providing methods for type erasure, error conversion, data transformation, and
/// integration with the `futures` ecosystem.
///
/// # Examples
///
/// ```no_run
/// use spire_core::dataset::{DatasetExt, InMemDataset};
///
/// let dataset = InMemDataset::<String>::queue()
///     .map_data(
///         |s: String| s.to_uppercase(),
///         |s: String| s.to_lowercase(),
///     )
///     .boxed_clone();
/// ```
pub trait DatasetExt<T>: Dataset<T> {
    /// Splits the dataset into separate [`DataSink`] and [`DataStream`] handles.
    ///
    /// This allows using the dataset with the `futures` crate's `Sink` and `Stream`
    /// traits for producer-consumer patterns.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use futures::{SinkExt, StreamExt};
    /// use spire_core::dataset::{DatasetExt, InMemDataset};
    ///
    /// # async fn example() -> Result<(), std::convert::Infallible> {
    /// let dataset = InMemDataset::<i32>::queue();
    /// let (mut sink, mut stream) = dataset.into_split();
    ///
    /// sink.send(42).await?;
    /// assert_eq!(stream.next().await, Some(Ok(42)));
    /// # Ok(())
    /// # }
    /// ```
    fn into_split(self) -> (DataSink<T, Self::Error>, DataStream<T, Self::Error>)
    where
        Self: Sized + Clone,
        T: Send + 'static,
        Self::Error: Send + 'static;

    /// Converts the dataset into a [`DataStream`] for consuming items.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use futures::StreamExt;
    /// use spire_core::dataset::{Dataset, DatasetExt, InMemDataset};
    ///
    /// # async fn example() -> Result<(), std::convert::Infallible> {
    /// let dataset = InMemDataset::<i32>::queue();
    /// dataset.write(1).await?;
    ///
    /// let mut stream = dataset.into_stream();
    /// assert_eq!(stream.next().await, Some(Ok(1)));
    /// # Ok(())
    /// # }
    /// ```
    fn into_stream(self) -> DataStream<T, Self::Error>
    where
        Self: Sized,
        T: Send + 'static,
        Self::Error: Send + 'static;

    /// Converts the dataset into a [`DataSink`] for producing items.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use futures::SinkExt;
    /// use spire_core::dataset::{DatasetExt, InMemDataset};
    ///
    /// # async fn example() -> Result<(), std::convert::Infallible> {
    /// let dataset = InMemDataset::<i32>::queue();
    /// let mut sink = dataset.into_sink();
    ///
    /// sink.send(42).await?;
    /// # Ok(())
    /// # }
    /// ```
    fn into_sink(self) -> DataSink<T, Self::Error>
    where
        Self: Sized,
        T: Send + 'static,
        Self::Error: Send + 'static;

    /// Wraps the dataset in a type-erased [`BoxDataset`].
    ///
    /// Use this when you need to store datasets of different concrete types
    /// together, but don't need cloning support.
    fn boxed(self) -> BoxDataset<T, Self::Error>;

    /// Wraps the dataset in a cloneable, type-erased [`BoxCloneDataset`].
    ///
    /// Use this when you need both type erasure and the ability to clone
    /// dataset handles.
    fn boxed_clone(self) -> BoxCloneDataset<T, Self::Error>
    where
        Self: Clone;

    /// Transforms data types during read and write operations.
    ///
    /// The `to` function converts values before writing to storage.
    /// The `from` function converts values after reading from storage.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use spire_core::dataset::{Dataset, DatasetExt, InMemDataset};
    ///
    /// # async fn example() -> Result<(), std::convert::Infallible> {
    /// // Store strings as uppercase, retrieve as lowercase
    /// let dataset = InMemDataset::<String>::queue()
    ///     .map_data(
    ///         |s: &str| s.to_uppercase(),
    ///         |s: String| s.to_lowercase(),
    ///     );
    ///
    /// dataset.write("Hello").await?;
    /// assert_eq!(dataset.read().await?, Some("hello".to_string()));
    /// # Ok(())
    /// # }
    /// ```
    fn map_data<F, F2>(self, to: F, from: F2) -> MapData<Self, F, F2>
    where
        Self: Sized;

    /// Transforms the error type of the dataset.
    ///
    /// Useful for adapting datasets to work with different error types in
    /// your application.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use spire_core::dataset::{DatasetExt, InMemDataset};
    ///
    /// let dataset = InMemDataset::<i32>::queue()
    ///     .map_err(|_infallible| "Never happens");
    /// ```
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
