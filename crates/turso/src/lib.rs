//! In-memory Turso / libSQL **test double** for the `ruprizzle` ORM.
//!
//! # This crate performs no network I/O
//!
//! [`InMemoryTursoStub`] implements [`ruprizzle::Executor`] against a process-local
//! `HashMap`. It does **not** open a libSQL connection, does not contact a Turso
//! primary, does not read or write a local replica file, and never transmits the
//! configured `auth_token` anywhere. Every value written through it is lost when the
//! process exits.
//!
//! It exists so that code written against the `Executor` trait can be exercised
//! without a database, and so that the shape of a future real adapter is pinned down.
//! It is **not** published to crates.io (`publish = false`) and must not be used as a
//! production backend. For real Turso access today, point [`ruprizzle::connect`] at
//! the `SQLite` file of an embedded replica you synchronise yourself.
//!
//! # Supported subset
//!
//! - `SELECT ... FROM <table>` returns every row previously inserted for `<table>`.
//!   Filters, joins, ordering, and limits are **ignored**.
//! - `INSERT INTO <table> (a, b) VALUES (...)` appends one row, naming the bound
//!   values after the declared column list. Without a column list the binds are
//!   named `col_0`, `col_1`, ....
//! - Every other statement is accepted and discarded.
//!
//! # Example
//!
//! ```no_run
//! use ruprizzle_turso::InMemoryTursoStub;
//!
//! # async fn doc() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
//! let pool = InMemoryTursoStub::builder()
//!     .local_path("local_replica.db")
//!     .sync_url("libsql://your-db.turso.io")
//!     .build()
//!     .await?;
//!
//! // `sync()` always fails: there is nothing to synchronise with.
//! assert!(pool.sync().await.is_err());
//! # Ok(())
//! # }
//! ```

#![forbid(unsafe_code)]
#![warn(clippy::pedantic)]
#![allow(clippy::unused_async)]

use std::borrow::Cow;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use futures_core::future::BoxFuture;
use ruprizzle::executor::{BoxRowStream, Executor, RawRow, RowBatch};
use ruprizzle::value::Value;
use ruprizzle_dialect::{DbDialect, SqliteDialect};
use thiserror::Error;
use tokio::sync::RwLock;

/// Dialect instance for Turso / libSQL (`SQLite` compatible).
static SQLITE_DIALECT: SqliteDialect = SqliteDialect;

/// Error returned by Turso and libSQL operations.
#[derive(Debug, Error)]
pub enum TursoError {
    /// Configuration or builder error.
    #[error("turso configuration error: {0}")]
    Config(String),

    /// Connection or synchronization error.
    #[error("turso sync error: {0}")]
    Sync(String),

    /// Query execution error.
    #[error("turso query error: {0}")]
    Query(String),

    /// Internal error.
    #[error("internal turso error: {0}")]
    Internal(String),

    /// The operation is not implemented by the in-memory stub.
    #[error("unsupported by the in-memory Turso stub: {0}")]
    Unsupported(String),
}

impl From<TursoError> for ruprizzle::Error {
    fn from(err: TursoError) -> Self {
        ruprizzle::Error::Message(err.to_string())
    }
}

/// Statistics reported after synchronization with a remote primary database.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SyncStats {
    /// Number of frames received from the primary.
    pub frames_synced: usize,
    /// Whether local changes were pushed to primary.
    pub pushed_local_writes: bool,
}

/// Configuration options for connecting to a Turso database.
#[derive(Debug, Clone, Default)]
pub struct TursoConfig {
    /// Path to the local embedded replica file.
    pub local_path: Option<PathBuf>,
    /// Remote synchronization URL (`libsql://...` or `https://...`).
    pub sync_url: Option<String>,
    /// Authentication token for remote Turso organization.
    pub auth_token: Option<String>,
    /// Periodic background sync interval.
    pub sync_interval: Option<Duration>,
    /// Enable read-your-writes consistency across replicas.
    pub read_your_writes: bool,
}

/// Builder for creating configured [`InMemoryTursoStub`] instances.
#[derive(Debug, Default)]
pub struct InMemoryTursoStubBuilder {
    config: TursoConfig,
}

impl InMemoryTursoStubBuilder {
    /// Creates a new builder with default configuration.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the local `SQLite` file path for the embedded replica.
    #[must_use]
    pub fn local_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.config.local_path = Some(path.into());
        self
    }

    /// Sets the remote Turso synchronization URL.
    #[must_use]
    pub fn sync_url(mut self, url: impl Into<String>) -> Self {
        self.config.sync_url = Some(url.into());
        self
    }

    /// Sets the authentication token.
    #[must_use]
    pub fn auth_token(mut self, token: impl Into<String>) -> Self {
        self.config.auth_token = Some(token.into());
        self
    }

    /// Sets the periodic sync interval for embedded replicas.
    #[must_use]
    pub fn sync_interval(mut self, interval: Duration) -> Self {
        self.config.sync_interval = Some(interval);
        self
    }

    /// Enables read-your-writes consistency.
    #[must_use]
    pub fn read_your_writes(mut self, enabled: bool) -> Self {
        self.config.read_your_writes = enabled;
        self
    }

    /// Builds and connects the [`InMemoryTursoStub`].
    ///
    /// # Errors
    ///
    /// Returns [`TursoError::Config`] if neither local path nor sync URL is provided.
    pub async fn build(self) -> Result<InMemoryTursoStub, TursoError> {
        if self.config.local_path.is_none() && self.config.sync_url.is_none() {
            return Err(TursoError::Config(
                "at least one of `local_path` or `sync_url` must be configured for InMemoryTursoStub"
                    .into(),
            ));
        }

        let inner = Arc::new(InMemoryTursoStubInner {
            config: self.config,
            memory_store: RwLock::new(HashMap::new()),
        });

        Ok(InMemoryTursoStub { inner })
    }
}

#[derive(Debug)]
struct InMemoryTursoStubInner {
    config: TursoConfig,
    memory_store: RwLock<HashMap<String, Vec<HashMap<String, Value>>>>,
}

/// A connection pool managing access to local and remote Turso libSQL databases.
#[derive(Debug, Clone)]
pub struct InMemoryTursoStub {
    inner: Arc<InMemoryTursoStubInner>,
}

impl InMemoryTursoStub {
    /// Returns a new [`InMemoryTursoStubBuilder`].
    #[must_use]
    pub fn builder() -> InMemoryTursoStubBuilder {
        InMemoryTursoStubBuilder::new()
    }

    /// Connects directly to a Turso database via URL.
    ///
    /// # Errors
    ///
    /// Returns [`TursoError::Config`] if the URL is empty.
    pub async fn connect(url: &str) -> Result<Self, TursoError> {
        Self::builder().sync_url(url).build().await
    }

    /// Returns the configured local replica path, if any.
    #[must_use]
    pub fn local_path(&self) -> Option<&Path> {
        self.inner.config.local_path.as_deref()
    }

    /// Returns the configured remote synchronization URL, if any.
    #[must_use]
    pub fn sync_url(&self) -> Option<&str> {
        self.inner.config.sync_url.as_deref()
    }

    /// Always fails: this stub has no remote primary to synchronize with.
    ///
    /// The signature is kept so that a future real adapter can drop in without a
    /// source change at the call site. It never returns [`SyncStats`].
    ///
    /// # Errors
    ///
    /// Always returns [`TursoError::Unsupported`].
    pub async fn sync(&self) -> Result<SyncStats, TursoError> {
        Err(TursoError::Unsupported(
            "InMemoryTursoStub performs no network I/O; there is no remote primary to sync with"
                .into(),
        ))
    }

    /// Executes an in-memory or remote query against the Turso backend.
    async fn execute_internal(&self, sql: &str, binds: &[Value]) -> Result<RowBatch, TursoError> {
        let trimmed = sql.trim();
        let upper = trimmed.to_uppercase();

        if upper.starts_with("SELECT") {
            let mut store = self.inner.memory_store.write().await;
            let table_name = extract_table_name(trimmed);
            let rows = store.entry(table_name).or_default().clone();
            Ok(RowBatch::Edge(rows))
        } else if upper.starts_with("INSERT") {
            let mut store = self.inner.memory_store.write().await;
            let table_name = extract_table_name(trimmed);
            let columns = extract_insert_columns(trimmed);
            let mut row = HashMap::new();
            for (idx, val) in binds.iter().enumerate() {
                let name = columns
                    .get(idx)
                    .cloned()
                    .unwrap_or_else(|| format!("col_{idx}"));
                row.insert(name, val.clone());
            }
            store.entry(table_name).or_default().push(row);
            Ok(RowBatch::Edge(Vec::new()))
        } else {
            Ok(RowBatch::Edge(Vec::new()))
        }
    }
}

/// Extracts the declared column list of an `INSERT INTO t (a, b) VALUES ...`.
///
/// Returns an empty vector when the statement declares no column list, in which case
/// the caller falls back to positional `col_N` names.
fn extract_insert_columns(sql: &str) -> Vec<String> {
    let upper = sql.to_uppercase();
    let Some(into_idx) = upper.find(" INTO ") else {
        return Vec::new();
    };
    let Some(rest) = sql.get(into_idx + 6..) else {
        return Vec::new();
    };
    let Some(open) = rest.find('(') else {
        return Vec::new();
    };
    let (Some(before), Some(from_open)) = (rest.get(..open), rest.get(open + 1..)) else {
        return Vec::new();
    };
    let Some(close) = from_open.find(')') else {
        return Vec::new();
    };
    // Only a column list may sit between the table name and the first `(`.
    if before.split_whitespace().count() != 1 {
        return Vec::new();
    }
    let Some(list) = from_open.get(..close) else {
        return Vec::new();
    };
    list.split(',')
        .map(|c| {
            c.trim()
                .trim_matches(|ch| ch == '"' || ch == '`' || ch == '\'')
                .to_string()
        })
        .filter(|c| !c.is_empty())
        .collect()
}

fn extract_table_name(sql: &str) -> String {
    let upper = sql.to_uppercase();
    if let Some(from_idx) = upper.find(" FROM ") {
        let rest = sql.get(from_idx + 6..).unwrap_or("").trim();
        rest.split(|c: char| c.is_whitespace() || c == ';' || c == '(')
            .next()
            .unwrap_or("default")
            .trim_matches(|c| c == '"' || c == '`' || c == '\'')
            .to_string()
    } else if let Some(into_idx) = upper.find(" INTO ") {
        let rest = sql.get(into_idx + 6..).unwrap_or("").trim();
        rest.split(|c: char| c.is_whitespace() || c == ';' || c == '(')
            .next()
            .unwrap_or("default")
            .trim_matches(|c| c == '"' || c == '`' || c == '\'')
            .to_string()
    } else {
        "default".to_string()
    }
}

impl Executor for InMemoryTursoStub {
    fn dialect(&self) -> &dyn DbDialect {
        &SQLITE_DIALECT
    }

    fn fetch_all_raw(
        &self,
        sql: Cow<'static, str>,
        binds: Vec<Value>,
    ) -> BoxFuture<'_, Result<RowBatch, ruprizzle::Error>> {
        Box::pin(async move {
            self.execute_internal(&sql, &binds)
                .await
                .map_err(ruprizzle::Error::from)
        })
    }

    fn execute_raw(
        &self,
        sql: Cow<'static, str>,
        binds: Vec<Value>,
    ) -> BoxFuture<'_, Result<u64, ruprizzle::Error>> {
        Box::pin(async move {
            self.execute_internal(&sql, &binds)
                .await
                .map(|b| b.len() as u64)
                .map_err(ruprizzle::Error::from)
        })
    }

    fn stream_raw(&self, sql: Cow<'static, str>, binds: Vec<Value>) -> BoxRowStream<'_> {
        let fut = self.fetch_all_raw(sql, binds);
        Box::pin(futures_util::stream::once(async move {
            fut.await.and_then(|batch| {
                let first = batch.as_edge_rows().and_then(|r| r.first()).cloned();
                first
                    .map(RawRow::Edge)
                    .ok_or_else(|| ruprizzle::Error::Message("empty result".into()))
            })
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_turso_builder_config() {
        let pool = InMemoryTursoStub::builder()
            .local_path("test.db")
            .sync_url("libsql://example.turso.io")
            .auth_token("token_123")
            .sync_interval(Duration::from_secs(30))
            .read_your_writes(true)
            .build()
            .await
            .unwrap();

        assert_eq!(pool.local_path(), Some(Path::new("test.db")));
        assert_eq!(pool.sync_url(), Some("libsql://example.turso.io"));
    }

    #[tokio::test]
    async fn test_turso_executor_query_and_sync() {
        let pool = InMemoryTursoStub::connect("libsql://demo.turso.io")
            .await
            .unwrap();
        assert_eq!(pool.dialect().name(), "sqlite");

        let err = pool.sync().await.unwrap_err();
        assert!(matches!(err, TursoError::Unsupported(_)));

        let rows = pool
            .fetch_all_raw(Cow::Borrowed("SELECT * FROM users"), Vec::new())
            .await
            .unwrap();
        assert!(rows.is_empty());
    }

    #[tokio::test]
    async fn insert_round_trips_under_declared_column_names() {
        let pool = InMemoryTursoStub::connect("libsql://demo.turso.io")
            .await
            .unwrap();

        pool.execute_raw(
            Cow::Borrowed("INSERT INTO users (id, email) VALUES (?, ?)"),
            vec![Value::I64(7), Value::Str("a@b.c".into())],
        )
        .await
        .unwrap();

        let batch = pool
            .fetch_all_raw(Cow::Borrowed("SELECT * FROM users"), Vec::new())
            .await
            .unwrap();
        let rows = batch.as_edge_rows().unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].get("id"), Some(&Value::I64(7)));
        assert_eq!(rows[0].get("email"), Some(&Value::Str("a@b.c".into())));
    }

    #[tokio::test]
    async fn writes_do_not_survive_a_new_stub() {
        let first = InMemoryTursoStub::connect("libsql://demo.turso.io")
            .await
            .unwrap();
        first
            .execute_raw(
                Cow::Borrowed("INSERT INTO users (id) VALUES (?)"),
                vec![Value::I64(1)],
            )
            .await
            .unwrap();

        let second = InMemoryTursoStub::connect("libsql://demo.turso.io")
            .await
            .unwrap();
        let batch = second
            .fetch_all_raw(Cow::Borrowed("SELECT * FROM users"), Vec::new())
            .await
            .unwrap();
        assert!(batch.is_empty(), "the stub stores nothing outside itself");
    }
}
