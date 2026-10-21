//! Seeds stored as JSON.

use spire_core::frontier::Seed;
use spire_core::{Error, ErrorKind, Result};

/// Encodes a seed.
pub(crate) fn encode(seed: &Seed) -> Result<Vec<u8>> {
    serde_json::to_vec(seed).map_err(|error| {
        Error::new(ErrorKind::Frontier)
            .with_message("failed to encode a seed")
            .with_source(error)
    })
}

/// Decodes a seed, or `None` if the stored bytes are not one.
pub(crate) fn decode(bytes: &[u8]) -> Option<Seed> {
    match serde_json::from_slice(bytes) {
        Ok(seed) => Some(seed),
        Err(error) => {
            tracing::warn!(target: crate::TRACING_TARGET, %error, "dropping an unreadable seed");
            None
        }
    }
}

/// Returns the priority of an encoded seed.
pub(crate) fn priority(bytes: &[u8]) -> Option<i32> {
    decode(bytes).map(|seed| seed.priority())
}
