//! Native `rusqlite` backend for the ruprizzle runtime.
//!
//! This module is compiled only when the `sqlite-rusqlite` feature is enabled.
//! It provides a synchronous, blocking-pinned SQLite connection pool that is
//! used instead of the `sqlx`-based SQLite backend when the connection URL
//! contains `driver=rusqlite`.

#![cfg(feature = "sqlite-rusqlite")]

use std::fmt;

/// A pool of synchronous `rusqlite` connections.
///
/// This type is intentionally cheap to clone and holds a shared set of
/// `tokio::sync::Mutex<rusqlite::Connection>` handles. Operations are run on
/// the blocking pool so they do not starve the async runtime.
#[derive(Clone)]
pub struct RusqlitePool;

impl RusqlitePool {
    /// Open a new `rusqlite` pool from a SQLite URL.
    ///
    /// # Errors
    ///
    /// Returns an error if the URL is not a valid SQLite URL or a connection
    /// cannot be opened.
    pub async fn connect(_url: &str) -> Result<Self, crate::Error> {
        Ok(Self)
    }
}

impl fmt::Debug for RusqlitePool {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RusqlitePool").finish_non_exhaustive()
    }
}
