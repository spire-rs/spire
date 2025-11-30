//! Futures integration for datasets via [`Stream`] and [`Sink`] adapters.
//!
//! This module provides integration between datasets and the `futures` crate's
//! asynchronous stream and sink abstractions, enabling datasets to be used in
//! async pipelines and reactive programming patterns.
//!
//! # Core Types
//!
//! - [`Data`] - Ergonomic wrapper around [`BoxCloneDataset`] for common use cases
//! - [`DataStream`] - Adapts a dataset into a `futures::Stream` for consuming items
//! - [`DataSink`] - Adapts a dataset into a `futures::Sink` for producing items
//!
//! # Examples
//!
//! ## Using DataStream
//!
//! ```ignore
//! use futures::StreamExt;
//! use spire_core::dataset::{Dataset, DatasetExt, InMemDataset};
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let dataset = InMemDataset::queue();
//! dataset.write("item1").await?;
//! dataset.write("item2").await?;
//!
//! let mut stream = dataset.into_stream();
//! while let Some(Ok(item)) = stream.next().await {
//!     println!("Received: {}", item);
//! }
//! # Ok(())
//! # }
//! ```
//!
//! ## Using DataSink
//!
//! ```ignore
//! use futures::SinkExt;
//! use spire_core::dataset::{DatasetExt, InMemDataset};
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let dataset = InMemDataset::queue();
//! let mut sink = dataset.into_sink();
//!
//! sink.send("item1").await?;
//! sink.send("item2").await?;
//! # Ok(())
//! # }
//! ```
//!
//! [`Stream`]: futures::Stream
//! [`Sink`]: futures::Sink
//! [`BoxCloneDataset`]: crate::dataset::utils::BoxCloneDataset

mod data;
mod sink;
mod stream;

pub use data::Data;
pub use sink::DataSink;
pub use stream::DataStream;

#[cfg(test)]
mod test {
    use futures::{SinkExt, StreamExt};

    use crate::dataset::{DatasetExt, InMemDataset};
    use crate::Result;

    #[tokio::test]
    async fn memory() -> Result<()> {
        let dataset = InMemDataset::<i32>::new();
        let (mut rx, mut tx) = dataset.into_split();

        rx.send(1).await?;
        let x = tx.next().await;
        assert_eq!(x, Some(Ok(1)));

        rx.send(2).await?;
        let x = tx.next().await;
        assert_eq!(x, Some(Ok(2)));

        Ok(())
    }
}
