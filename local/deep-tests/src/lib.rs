//! Local-only deep-test fixtures.
//!
//! All helpers in this crate are SQLite-only and keep their artefacts under the
//! `local/` directory, so the suite never needs Docker, a remote database, or any
//! secrets stored outside the repository.

#![forbid(unsafe_code)]
#![warn(missing_docs, clippy::pedantic)]

use std::path::PathBuf;
use tempfile::TempDir;

/// Directory under which per-test SQLite files are created.
///
/// This lives inside the crate manifest directory, which is `local/deep-tests`,
/// so every database created by the suite is inside the repo-local `local/`
/// tree.
pub fn db_dir() -> PathBuf {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let dir = manifest.join("db");
    std::fs::create_dir_all(&dir).expect("create local deep-test db dir");
    dir
}

/// Creates an isolated SQLite pool in a temporary directory under
/// [`db_dir`].
///
/// Keep the returned [`TempDir`] alive for the lifetime of the pool; the
/// directory is removed when it is dropped.
///
/// The returned pool uses the `Any` driver so that tests can also exercise raw
/// `sqlx` queries against the same connection.
pub async fn fresh_pool() -> (ruprizzle::Pool, TempDir) {
    sqlx::any::install_default_drivers();
    let dir = tempfile::tempdir_in(db_dir()).expect("create temp dir under local db dir");
    let path = dir.path().join("test.sqlite");
    let file = path.to_str().unwrap().replace('\\', "/");
    let url = format!("sqlite:///{}?mode=rwc", file);
    let any = sqlx::any::AnyPoolOptions::new()
        .max_connections(4)
        .acquire_timeout(std::time::Duration::from_secs(5))
        .connect(&url)
        .await
        .expect("connect to local sqlite");
    let pool = ruprizzle::Pool::Any(any);
    (pool, dir)
}
