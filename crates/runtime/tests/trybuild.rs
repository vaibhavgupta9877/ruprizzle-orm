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

#[test]
#[cfg(all(
    not(feature = "sqlite-rusqlite"),
    not(feature = "postgres-tokio-postgres")
))]
fn offline_schema_validation() {
    // `trybuild` invokes `cargo` in a subprocess; `std::env::set_var` is not
    // guaranteed to propagate to that subprocess on Windows, so we re-exec this
    // test with `RUPRIZZLE_OFFLINE_SCHEMA` set in the child environment.
    if std::env::var("_RZ_OFFLINE_SCHEMA_TEST").is_err() {
        let manifest_dir = std::env::var_os("CARGO_MANIFEST_DIR")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| std::env::current_dir().unwrap());
        let schema = manifest_dir.join("tests/trybuild/offline/schema.ruprizzle");
        let canonical = std::fs::canonicalize(&schema).unwrap_or(schema);
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .arg("offline_schema_validation")
            .arg("--exact")
            .arg("--nocapture")
            .env("RUPRIZZLE_OFFLINE_SCHEMA", canonical)
            .env("_RZ_OFFLINE_SCHEMA_TEST", "1")
            .status()
            .expect("failed to spawn offline schema validation subprocess");
        assert!(
            status.success(),
            "offline schema validation subprocess failed"
        );
        return;
    }

    let t = trybuild::TestCases::new();
    t.compile_fail("tests/trybuild/offline/*.rs");
}
