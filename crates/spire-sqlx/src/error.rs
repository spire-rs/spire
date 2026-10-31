use spire_core::{Error, ErrorKind};

pub(crate) fn dataset_error(error: impl Into<spire_core::BoxError>) -> Error {
    Error::new(ErrorKind::Dataset)
        .with_message("SQL dataset operation failed")
        .with_source(error)
}

pub(crate) fn frontier_error(error: impl Into<spire_core::BoxError>) -> Error {
    Error::new(ErrorKind::Frontier)
        .with_message("SQL crawl storage operation failed")
        .with_source(error)
}
