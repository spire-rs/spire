use std::fmt;
use std::sync::{Arc, Weak};

use super::{Identity, Inner};

/// A weak reference to an [`Identity`], from [`Identity::downgrade`].
#[derive(Clone)]
pub struct WeakIdentity(pub(super) Weak<Inner>);

impl WeakIdentity {
    /// Returns the identity, unless it is gone.
    #[must_use]
    pub fn upgrade(&self) -> Option<Identity> {
        self.0.upgrade().map(Identity)
    }

    /// Whether the identity is still in use.
    #[must_use]
    pub fn is_alive(&self) -> bool {
        self.0.strong_count() > 0
    }

    /// Whether this refers to `identity` itself, not merely one with the
    /// same name.
    #[must_use]
    pub fn refers_to(&self, identity: &Identity) -> bool {
        std::ptr::eq(self.0.as_ptr(), Arc::as_ptr(&identity.0))
    }
}

impl fmt::Debug for WeakIdentity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("WeakIdentity")
            .field(&self.upgrade().as_ref().map(Identity::name))
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn weak_references_outlive_their_identity() {
        let identity = Identity::new("a");
        let weak = identity.downgrade();
        assert!(weak.is_alive() && weak.refers_to(&identity));
        assert!(!weak.refers_to(&Identity::new("a")));
        drop(identity);
        assert!(!weak.is_alive());
        assert!(weak.upgrade().is_none());
    }
}
