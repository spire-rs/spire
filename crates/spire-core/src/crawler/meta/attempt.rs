use std::fmt;

/// How many times a request was tried before.
///
/// The first try is attempt 0.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Attempt(u32);

impl Attempt {
    /// The first try.
    pub const FIRST: Self = Self(0);

    /// Creates an attempt.
    #[must_use]
    pub const fn new(attempt: u32) -> Self {
        Self(attempt)
    }

    /// Returns the attempt as a number.
    #[must_use]
    pub const fn get(self) -> u32 {
        self.0
    }

    /// Returns the attempt after this one.
    #[must_use]
    pub const fn next(self) -> Self {
        Self(self.0.saturating_add(1))
    }
}

impl fmt::Display for Attempt {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}
