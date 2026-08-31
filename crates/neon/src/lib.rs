//! In-memory Neon serverless `PostgreSQL` **test double** for the `ruprizzle` ORM.
//!
//! # This crate performs no network I/O
//!
//! [`InMemoryNeonStub`] implements [`ruprizzle::Executor`] against a process-local
//! `HashMap`. It does **not** open a WebSocket or HTTP session to a Neon endpoint and
//! never transmits the configured `auth_token` anywhere. Every value written through
//! it is lost when the process exits.
//!
//! It exists so that code written against the `Executor` trait can be exercised
//! without a database, and so that the shape of a future real adapter is pinned down.
//! It is **not** published to crates.io (`publish = false`) and must not be used as a
//! production backend. Neon speaks ordinary `PostgreSQL` over TLS, so for real Neon
//! access today pass the Neon connection string straight to [`ruprizzle::connect`].
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
//! use ruprizzle_neon::InMemoryNeonStub;
//!
//! # async fn doc() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
//! let pool = InMemoryNeonStub::connect("postgres://user:pw@ep-x.neon.tech/neondb").await?;
//! assert!(pool.connection_string().is_some());
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
use ruprizzle_dialect::{DbDialect, PostgresDialect};
use thiserror::Error;
use tokio::sync::RwLock;

/// Dialect instance for Neon Serverless (`PostgreSQL` compatible).
static POSTGRES_DIALECT: PostgresDialect = PostgresDialect;

/// Error returned by Neon serverless operations.
#[derive(Debug, Error)]
pub enum NeonError {
    /// Configuration error.
    #[error("Neon configuration error: {0}")]
    Config(String),

    /// Connection or transport error.
    #[error("Neon transport error: {0}")]
    Transport(String),

    /// Query execution error.
    #[error("Neon query error: {0}")]
    Query(String),

    /// Internal error.
    #[error("internal Neon error: {0}")]
    Internal(String),
}

impl From<NeonError> for ruprizzle::Error {
    fn from(err: NeonError) -> Self {
        ruprizzle::Error::Message(err.to_string())
    }
}

/// Configuration options for connecting to Neon serverless Postgres.
#[derive(Debug, Clone, Default)]
pub struct NeonConfig {
    /// Full `PostgreSQL` connection string (`postgres://...`).
    pub connection_string: Option<String>,
    /// Neon endpoint host (`ep-xxx.neon.tech`).
    pub endpoint: Option<String>,
    /// Neon authentication token or password.
    pub auth_token: Option<String>,
    /// Database name.
    pub database: Option<String>,
    /// Whether to use WebSocket transport instead of HTTP.
    pub use_websocket: bool,
    /// Maximum concurrent multiplexed streams.
    pub max_connections: usize,
}

/// Builder for creating configured [`InMemoryNeonStub`] instances.
#[derive(Debug, Default)]
pub struct InMemoryNeonStubBuilder {
    config: NeonConfig,
}

impl InMemoryNeonStubBuilder {
    /// Creates a new builder with default configuration.
    #[must_use]
    pub fn new() -> Self {
        Self {
            config: NeonConfig {
                max_connections: 10,
                ..Default::default()
            },
        }
    }

    /// Sets the full `PostgreSQL` connection string.
    #[must_use]
    pub fn connection_string(mut self, url: impl Into<String>) -> Self {
        self.config.connection_string = Some(url.into());
        self
    }

    /// Sets the Neon endpoint host name.
    #[must_use]
    pub fn endpoint(mut self, host: impl Into<String>) -> Self {
        self.config.endpoint = Some(host.into());
        self
    }

    /// Sets the authentication token.
    #[must_use]
    pub fn auth_token(mut self, token: impl Into<String>) -> Self {
        self.config.auth_token = Some(token.into());
        self
    }

    /// Sets the target database name.
    #[must_use]
    pub fn database(mut self, db: impl Into<String>) -> Self {
        self.config.database = Some(db.into());
        self
    }

    /// Enables WebSocket transport.
    #[must_use]
    pub fn use_websocket(mut self, enabled: bool) -> Self {
        self.config.use_websocket = enabled;
        self
    }

    /// Sets the maximum connection concurrency.
    #[must_use]
    pub fn max_connections(mut self, max: usize) -> Self {
        self.config.max_connections = max;
        self
    }

    /// Builds and connects the [`InMemoryNeonStub`].
    ///
    /// # Errors
    ///
    /// Returns [`NeonError::Config`] if neither connection string nor endpoint is provided.
    pub async fn build(self) -> Result<InMemoryNeonStub, NeonError> {
        if self.config.connection_string.is_none() && self.config.endpoint.is_none() {
            return Err(NeonError::Config(
                "either `connection_string` or `endpoint` must be configured for InMemoryNeonStub"
                    .into(),
            ));
        }

        let inner = Arc::new(InMemoryNeonStubInner {
            config: self.config,
            memory_store: RwLock::new(HashMap::new()),
        });

        Ok(InMemoryNeonStub { inner })
    }
}

#[derive(Debug)]
struct InMemoryNeonStubInner {
    config: NeonConfig,
    memory_store: RwLock<HashMap<String, Vec<HashMap<String, Value>>>>,
}

/// A connection pool managing access to Neon Serverless `PostgreSQL`.
#[derive(Debug, Clone)]
pub struct InMemoryNeonStub {
    inner: Arc<InMemoryNeonStubInner>,
}

impl InMemoryNeonStub {
    /// Returns a new [`InMemoryNeonStubBuilder`].
    #[must_use]
    pub fn builder() -> InMemoryNeonStubBuilder {
        InMemoryNeonStubBuilder::new()
    }

    /// Connects directly using a Postgres connection string.
    ///
    /// # Errors
    ///
    /// Returns [`NeonError::Config`] if the URL is empty.
    pub async fn connect(url: &str) -> Result<Self, NeonError> {
        Self::builder().connection_string(url).build().await
    }

    /// Returns the configured endpoint host, if any.
    #[must_use]
    pub fn endpoint(&self) -> Option<&str> {
        self.inner.config.endpoint.as_deref()
    }

    /// Returns the configured connection string, if any.
    #[must_use]
    pub fn connection_string(&self) -> Option<&str> {
        self.inner.config.connection_string.as_deref()
    }

    /// Returns whether WebSocket transport is enabled.
    #[must_use]
    pub fn is_websocket_enabled(&self) -> bool {
        self.inner.config.use_websocket
    }

    /// Executes a SQL query against the Neon backend.
    async fn execute_internal(&self, sql: &str, binds: &[Value]) -> Result<RowBatch, NeonError> {
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

impl Executor for InMemoryNeonStub {
    fn dialect(&self) -> &dyn DbDialect {
        &POSTGRES_DIALECT
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
    async fn test_neon_builder_config() {
        let pool = InMemoryNeonStub::builder()
            .endpoint("ep-dry-lake-123456.us-east-2.aws.neon.tech")
            .auth_token("npg_token_secret")
            .database("main")
            .use_websocket(true)
            .max_connections(20)
            .build()
            .await
            .unwrap();

        assert_eq!(
            pool.endpoint(),
            Some("ep-dry-lake-123456.us-east-2.aws.neon.tech")
        );
        assert!(pool.is_websocket_enabled());
    }

    #[tokio::test]
    async fn test_neon_executor_query() {
        let pool = InMemoryNeonStub::connect("postgres://user:pass@ep-test.neon.tech/neondb")
            .await
            .unwrap();

        assert_eq!(pool.dialect().name(), "postgres");

        let rows = pool
            .fetch_all_raw(Cow::Borrowed("SELECT * FROM users"), Vec::new())
            .await
            .unwrap();
        assert!(rows.is_empty());
    }

    #[tokio::test]
    async fn insert_round_trips_under_declared_column_names() {
        let pool = InMemoryNeonStub::connect("postgres://u:p@ep-test.neon.tech/neondb")
            .await
            .unwrap();

        pool.execute_raw(
            Cow::Borrowed("INSERT INTO users (id, email) VALUES ($1, $2)"),
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
        let first = InMemoryNeonStub::connect("postgres://u:p@ep-test.neon.tech/neondb")
            .await
            .unwrap();
        first
            .execute_raw(
                Cow::Borrowed("INSERT INTO users (id) VALUES ($1)"),
                vec![Value::I64(1)],
            )
            .await
            .unwrap();

        let second = InMemoryNeonStub::connect("postgres://u:p@ep-test.neon.tech/neondb")
            .await
            .unwrap();
        let batch = second
            .fetch_all_raw(Cow::Borrowed("SELECT * FROM users"), Vec::new())
            .await
            .unwrap();
        assert!(batch.is_empty(), "the stub stores nothing outside itself");
    }
}
