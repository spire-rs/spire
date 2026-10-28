//! The compile errors of `#[derive(Select)]` and `#[derive(Tags)]`.
//!
//! The expected `.stderr` files are pinned to the toolchain CI uses; after a
//! toolchain bump, regenerate them with `TRYBUILD=overwrite`.

#![cfg(feature = "macros")]

#[test]
fn ui() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/ui/*.rs");
}
