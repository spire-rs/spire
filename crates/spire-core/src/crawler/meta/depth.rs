use std::fmt;

/// How many links away from a seed a request is.
///
/// Seeds have depth 0; every followed link is one deeper than its parent.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Depth(u32);

impl Depth {
    /// The depth of a seed.
    pub const ROOT: Self = Self(0);

    /// Creates a depth.
    #[must_use]
    pub const fn new(depth: u32) -> Self {
        Self(depth)
    }

    /// Returns the depth as a number.
    #[must_use]
    pub const fn get(self) -> u32 {
        self.0
    }

    /// Returns the depth of a link followed from this one.
    #[must_use]
    pub const fn child(self) -> Self {
        Self(self.0.saturating_add(1))
    }
}

impl fmt::Display for Depth {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}
