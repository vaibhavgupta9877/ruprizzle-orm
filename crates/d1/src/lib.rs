//! In-memory Cloudflare D1 **test double** for the `ruprizzle` ORM.
//!
//! # This crate performs no network I/O
//!
//! [`InMemoryD1Stub`] implements [`ruprizzle::Executor`] against a process-local
//! `HashMap`. It does **not** call the Cloudflare D1 REST API, does not run inside a
//! Worker, and never transmits the configured `api_token` anywhere. Every value
//! written through it is lost when the process exits.
//!
//! It exists so that code written against the `Executor` trait can be exercised
//! without a database, and so that the shape of a future real adapter is pinned down.
//! It is **not** published to crates.io (`publish = false`) and must not be used as a
//! production backend.
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
//! use ruprizzle_d1::InMemoryD1Stub;
//!
//! # async fn doc() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
//! let pool = InMemoryD1Stub::builder()
//!     .account_id("cf_account_123")
//!     .database_id("d1_database_uuid")
//!     .build()
//!     .await?;
//!
//! assert_eq!(pool.database_id(), Some("d1_database_uuid"));
//! # Ok(())
//! # }
//! ```

#![forbid(unsafe_code)]
#![warn(clippy::pedantic)]
#![allow(clippy::unused_async)]

use std::borrow::Cow;
use std::collections::HashMap;
use std::sync::Arc;

use futures_core::future::BoxFuture;
use ruprizzle::executor::{BoxRowStream, Executor, RawRow, RowBatch};
use ruprizzle::value::Value;
use ruprizzle_dialect::{DbDialect, SqliteDialect};
use thiserror::Error;
use tokio::sync::RwLock;

/// Dialect instance for Cloudflare D1 (`SQLite` compatible).
static SQLITE_DIALECT: SqliteDialect = SqliteDialect;

/// Error returned by Cloudflare D1 operations.
#[derive(Debug, Error)]
pub enum D1Error {
    /// Configuration error.
    #[error("D1 configuration error: {0}")]
    Config(String),

    /// HTTP request or API error.
    #[error("D1 API error: {0}")]
    Api(String),

    /// Query execution error.
    #[error("D1 query error: {0}")]
    Query(String),

    /// Internal error.
    #[error("internal D1 error: {0}")]
    Internal(String),
}

impl From<D1Error> for ruprizzle::Error {
    fn from(err: D1Error) -> Self {
        ruprizzle::Error::Message(err.to_string())
    }
}

/// Configuration options for connecting to Cloudflare D1.
#[derive(Debug, Clone, Default)]
pub struct D1Config {
    /// Cloudflare Account ID.
    pub account_id: Option<String>,
    /// Cloudflare D1 Database ID (UUID).
    pub database_id: Option<String>,
    /// Cloudflare API Bearer token.
    pub api_token: Option<String>,
    /// Custom REST API endpoint (e.g. for Miniflare local development).
    pub endpoint: Option<String>,
}

/// Builder for creating configured [`InMemoryD1Stub`] instances.
#[derive(Debug, Default)]
pub struct InMemoryD1StubBuilder {
    config: D1Config,
}

impl InMemoryD1StubBuilder {
    /// Creates a new builder with default configuration.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the Cloudflare Account ID.
    #[must_use]
    pub fn account_id(mut self, id: impl Into<String>) -> Self {
        self.config.account_id = Some(id.into());
        self
    }

    /// Sets the Cloudflare D1 Database ID.
    #[must_use]
    pub fn database_id(mut self, id: impl Into<String>) -> Self {
        self.config.database_id = Some(id.into());
        self
    }

    /// Sets the Cloudflare API Bearer token.
    #[must_use]
    pub fn api_token(mut self, token: impl Into<String>) -> Self {
        self.config.api_token = Some(token.into());
        self
    }

    /// Sets a custom REST API endpoint.
    #[must_use]
    pub fn endpoint(mut self, url: impl Into<String>) -> Self {
        self.config.endpoint = Some(url.into());
        self
    }

    /// Builds and initializes the [`InMemoryD1Stub`].
    ///
    /// # Errors
    ///
    /// Returns [`D1Error::Config`] if required credentials are missing.
    pub async fn build(self) -> Result<InMemoryD1Stub, D1Error> {
        if self.config.database_id.is_none() && self.config.endpoint.is_none() {
            return Err(D1Error::Config(
                "`database_id` or `endpoint` must be configured for InMemoryD1Stub".into(),
            ));
        }

        let inner = Arc::new(InMemoryD1StubInner {
            config: self.config,
            memory_store: RwLock::new(HashMap::new()),
        });

        Ok(InMemoryD1Stub { inner })
    }
}

#[derive(Debug)]
struct InMemoryD1StubInner {
    config: D1Config,
    memory_store: RwLock<HashMap<String, Vec<HashMap<String, Value>>>>,
}

/// A connection pool managing access to Cloudflare D1.
#[derive(Debug, Clone)]
pub struct InMemoryD1Stub {
    inner: Arc<InMemoryD1StubInner>,
}

impl InMemoryD1Stub {
    /// Returns a new [`InMemoryD1StubBuilder`].
    #[must_use]
    pub fn builder() -> InMemoryD1StubBuilder {
        InMemoryD1StubBuilder::new()
    }

    /// Returns the configured database ID, if any.
    #[must_use]
    pub fn database_id(&self) -> Option<&str> {
        self.inner.config.database_id.as_deref()
    }

    /// Returns the configured account ID, if any.
    #[must_use]
    pub fn account_id(&self) -> Option<&str> {
        self.inner.config.account_id.as_deref()
    }

    /// Executes a SQL query against the D1 backend.
    async fn execute_internal(&self, sql: &str, binds: &[Value]) -> Result<RowBatch, D1Error> {
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
    let Some(close) = rest[open..].find(')') else {
        return Vec::new();
    };
    // Only a column list may sit between the table name and the first `(`.
    if rest[..open].split_whitespace().count() != 1 {
        return Vec::new();
    }
    rest[open + 1..open + close]
        .split(',')
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

impl Executor for InMemoryD1Stub {
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
    async fn test_d1_builder_config() {
        let pool = InMemoryD1Stub::builder()
            .account_id("account_xyz")
            .database_id("d1_uuid_456")
            .api_token("secret_bearer")
            .build()
            .await
            .unwrap();

        assert_eq!(pool.account_id(), Some("account_xyz"));
        assert_eq!(pool.database_id(), Some("d1_uuid_456"));
    }

    #[tokio::test]
    async fn test_d1_executor_query() {
        let pool = InMemoryD1Stub::builder()
            .database_id("d1_test_db")
            .build()
            .await
            .unwrap();

        assert_eq!(pool.dialect().name(), "sqlite");

        let rows = pool
            .fetch_all_raw(Cow::Borrowed("SELECT * FROM users"), Vec::new())
            .await
            .unwrap();
        assert!(rows.is_empty());
    }

    #[tokio::test]
    async fn insert_round_trips_under_declared_column_names() {
        let pool = InMemoryD1Stub::builder()
            .database_id("d1_test_db")
            .build()
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
        let first = InMemoryD1Stub::builder()
            .database_id("d1_test_db")
            .build()
            .await
            .unwrap();
        first
            .execute_raw(
                Cow::Borrowed("INSERT INTO users (id) VALUES (?)"),
                vec![Value::I64(1)],
            )
            .await
            .unwrap();

        let second = InMemoryD1Stub::builder()
            .database_id("d1_test_db")
            .build()
            .await
            .unwrap();
        let batch = second
            .fetch_all_raw(Cow::Borrowed("SELECT * FROM users"), Vec::new())
            .await
            .unwrap();
        assert!(batch.is_empty(), "the stub stores nothing outside itself");
    }
}
