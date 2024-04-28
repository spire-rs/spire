//! Various utility [`Backend`]s, [`Client`]s and [`Worker`]s.
//!
//! [`Backend`]: crate::backend::Backend
//! [`Client`]: crate::backend::Client
//! [`Worker`]: crate::backend::Worker

pub use debug::DebugEntity;
#[cfg(feature = "tracing")]
#[cfg_attr(docsrs, doc(cfg(feature = "tracing")))]
pub use trace::TraceEntity;

mod debug;
#[cfg(feature = "tracing")]
#[cfg_attr(docsrs, doc(cfg(feature = "tracing")))]
mod trace;
