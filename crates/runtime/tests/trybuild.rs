//! Compile-fail tests for runtime type-safe guards.
//!
//! `trybuild` snapshots are recorded against the default feature set. They are
//! skipped when the optional native `tokio-postgres` backend is enabled because
//! the generated `RowDecode` bound would otherwise require every test fixture to
//! implement `FromTokioPostgresRow`.

#[test]
#[cfg(not(feature = "postgres-tokio-postgres"))]
fn compile_failures() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/trybuild/*.rs");
}
