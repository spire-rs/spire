//! Synchronization helpers.

use std::sync::{Mutex, MutexGuard, PoisonError};

/// Locks the mutex, recovering the guard if a previous holder panicked.
///
/// Every mutex in this crate guards plain data whose invariants hold between
/// statements, so a poisoned lock is still safe to use.
pub(crate) fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}
