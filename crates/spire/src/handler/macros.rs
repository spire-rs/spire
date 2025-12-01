//! Macro utilities for handler implementations.
//!
//! This module provides macros used internally to generate [`Handler`] trait
//! implementations for functions with different numbers of extractor parameters.

/// Generates macro invocations for tuples of varying lengths.
///
/// This macro is used to automatically generate [`Handler`] implementations
/// for functions that take 0 to 16 extractor parameters. It's a core part
/// of Spire's handler system that allows handlers to accept any combination
/// of extractors as arguments.
///
/// # How it works
///
/// The macro takes another macro name as input and invokes it with progressively
/// longer tuples of type parameters. This allows the `impl_handler` macro in
/// [`super`] to generate implementations for:
///
/// - `Handler<C, ((),), S> for F` - handlers with no extractors
/// - `Handler<C, (M, T1,), S> for F` - handlers with 1 extractor
/// - `Handler<C, (M, T1, T2,), S> for F` - handlers with 2 extractors
/// - ... up to 16 extractors
///
/// # Usage
///
/// ```ignore
/// macro_rules! my_macro {
///     ([$($processed:ident),*], $last:ident) => {
///         // Implementation for tuple ($($processed,)* $last,)
///     };
/// }
///
/// all_the_tuples!(my_macro);
/// ```
///
/// # Limitations
///
/// Currently supports up to 16 extractors per handler function. This limit
/// is based on practical usage patterns and keeps compilation times reasonable.
///
/// # Origin
///
/// Forked from [`axum::macros::all_the_tuples`] with modifications for Spire's
/// handler system.
///
/// [`Handler`]: super::Handler
/// [`axum::macros::all_the_tuples`]: https://github.com/tokio-rs/axum
#[rustfmt::skip]
#[macro_export]
#[doc(hidden)]
macro_rules! all_the_tuples {
    ($name:ident) => {
        $name!([], T1);
        $name!([T1], T2);
        $name!([T1, T2], T3);
        $name!([T1, T2, T3], T4);
        $name!([T1, T2, T3, T4], T5);
        $name!([T1, T2, T3, T4, T5], T6);
        $name!([T1, T2, T3, T4, T5, T6], T7);
        $name!([T1, T2, T3, T4, T5, T6, T7], T8);
        $name!([T1, T2, T3, T4, T5, T6, T7, T8], T9);
        $name!([T1, T2, T3, T4, T5, T6, T7, T8, T9], T10);
        $name!([T1, T2, T3, T4, T5, T6, T7, T8, T9, T10], T11);
        $name!([T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11], T12);
        $name!([T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12], T13);
        $name!([T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13], T14);
        $name!([T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13, T14], T15);
        $name!([T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13, T14, T15], T16);
    };
}

// TODO: Make unavailable from outside the crate to prevent external usage.
pub use all_the_tuples;
