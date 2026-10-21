//! Stable request fingerprints.

/// A stable 64-bit identifier of a request, see
/// [`Seed::fingerprint`](crate::frontier::seed::Seed::fingerprint).
///
/// The hash is stable across runs and platforms, so it can be persisted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(transparent))]
pub struct Fingerprint(u64);

impl Fingerprint {
    /// Returns the fingerprint as a number.
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}

impl Fingerprint {
    /// Hashes `parts`, separated so that moving bytes between parts changes
    /// the fingerprint.
    pub(crate) fn of(parts: &[&[u8]]) -> Self {
        let mut hasher = Fnv1a::new();
        for (i, part) in parts.iter().enumerate() {
            if i > 0 {
                hasher.write(&[0]);
            }
            hasher.write(part);
        }
        Self(hasher.finish())
    }
}

/// The 64-bit FNV-1a hash, chosen over `DefaultHasher` for its stability.
struct Fnv1a(u64);

impl Fnv1a {
    const OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
    const PRIME: u64 = 0x0000_0100_0000_01b3;

    const fn new() -> Self {
        Self(Self::OFFSET)
    }

    fn write(&mut self, bytes: &[u8]) {
        for byte in bytes {
            self.0 = (self.0 ^ u64::from(*byte)).wrapping_mul(Self::PRIME);
        }
    }

    const fn finish(&self) -> u64 {
        self.0
    }
}
