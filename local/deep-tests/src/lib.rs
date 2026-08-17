//! Local-only deep-test fixtures.
//!
//! All helpers in this crate are SQLite-only and keep their artefacts under the
//! `local/` directory, so the suite never needs Docker, a remote database, or any
//! secrets stored outside the repository.

#![forbid(unsafe_code)]
#![warn(missing_docs, clippy::pedantic)]

use std::path::PathBuf;
use tempfile::TempDir;

/// Directory under which per-test `SQLite` files are created.
///
/// This lives inside the crate manifest directory, which is `local/deep-tests`,
/// so every database created by the suite is inside the repo-local `local/`
/// tree.
///
/// # Panics
///
/// Panics if the local `db/` directory cannot be created.
#[must_use]
pub fn db_dir() -> PathBuf {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let dir = manifest.join("db");
    std::fs::create_dir_all(&dir).expect("create local deep-test db dir");
    dir
}

/// Creates an isolated `SQLite` pool in a temporary directory under
/// [`db_dir`].
///
/// Keep the returned [`TempDir`] alive for the lifetime of the pool; the
/// directory is removed when it is dropped.
///
/// The returned pool follows `RUPRIZZLE_TEST_RUSQLITE` and uses `driver=rusqlite`
/// when the environment variable is set, otherwise the default `sqlx` `SQLite`
/// driver.
///
/// # Panics
///
/// Panics if the temporary directory or `SQLite` file path cannot be created.
pub async fn fresh_pool() -> (ruprizzle::Pool, TempDir) {
    let dir = tempfile::tempdir_in(db_dir()).expect("create temp dir under local db dir");
    let path = dir.path().join("test.sqlite");
    let file = path.to_str().unwrap().replace('\\', "/");
    let driver = if std::env::var("RUPRIZZLE_TEST_RUSQLITE").is_ok() {
        "&driver=rusqlite"
    } else {
        ""
    };
    let url = format!("sqlite:///{file}?mode=rwc{driver}");
    let pool = ruprizzle::connect(&url)
        .await
        .expect("connect to local sqlite");
    (pool, dir)
}
