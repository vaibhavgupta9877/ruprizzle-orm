//! Compile-fail tests for runtime type-safe guards.
//!
//! `trybuild` snapshots are recorded against the default feature set. They are
//! skipped when either optional native backend is enabled because the generated
//! `RowDecode` bound would otherwise require every test fixture to implement
//! `FromRusqliteRow` or `FromTokioPostgresRow`, producing different diagnostics.

#[test]
#[cfg(all(
    not(feature = "sqlite-rusqlite"),
    not(feature = "postgres-tokio-postgres")
))]
fn compile_failures() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/trybuild/*.rs");
}
