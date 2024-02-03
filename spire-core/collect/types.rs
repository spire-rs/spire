use std::marker::PhantomData;

#[cfg(feature = "driver")]
use crate::driver::DriverPool;

#[derive(Debug, Clone, Default)]
pub struct Label(pub String);

/// TODO.
#[derive(Debug, Clone)]
pub struct Metrics {
    requests: usize,
    responses: usize,
}

/// Framework-specific request type.
/// Crawler [`Service`]s are expected to take it.
pub struct Request<T> {
    marker: PhantomData<T>,
}

/// Framework-specific response type.
/// Crawler [`Service`]s are expected to return it.
pub struct Response<T> {
    marker: PhantomData<T>,
}

pub struct Context<T, U = T> {
    #[cfg(feature = "driver")]
    drivers: DriverPool,
    request: Request<T>,
    response: Response<U>,
}
