//! Shared test helpers for the `ruprizzle` runtime integration tests.

use ruprizzle::Pool;

/// A [`Pool`] paired with the temporary directory that holds its SQLite file.
///
/// On Windows an open SQLite file cannot be deleted while a handle is held, so
/// dropping the temporary directory early "works".  On Unix, deleting an open
/// file removes its path from the filesystem, and later attempts to open
/// additional connections to the same URL fail with "unable to open database
/// file".  Keeping the directory alive until the pool is dropped fixes that.
pub type PoolWithDir = (Pool, tempfile::TempDir);
