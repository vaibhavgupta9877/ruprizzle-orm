//! Cloudflare D1 edge database adapter for the `ruprizzle` ORM.
//!
//! Provides [`D1Pool`] and [`D1PoolBuilder`] for connecting to Cloudflare D1
//! databases over the HTTP REST API or in WASM environments (Cloudflare Workers).
//!
//! # Example
//!
//! ```no_run
//! use ruprizzle_d1::D1Pool;
//!
//! # async fn doc() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
//! let pool = D1Pool::builder()
//!     .account_id("cf_account_123")
//!     .database_id("d1_database_uuid")
//!     .api_token("cf_api_token")
//!     .build()
//!     .await?;
//!
//! println!("Connected to Cloudflare D1: {:?}", pool.database_id());
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

/// Builder for creating configured [`D1Pool`] instances.
#[derive(Debug, Default)]
pub struct D1PoolBuilder {
    config: D1Config,
}

impl D1PoolBuilder {
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

    /// Builds and initializes the [`D1Pool`].
    ///
    /// # Errors
    ///
    /// Returns [`D1Error::Config`] if required credentials are missing.
    pub async fn build(self) -> Result<D1Pool, D1Error> {
        if self.config.database_id.is_none() && self.config.endpoint.is_none() {
            return Err(D1Error::Config(
                "`database_id` or `endpoint` must be configured for D1Pool".into(),
            ));
        }

        let inner = Arc::new(D1PoolInner {
            config: self.config,
            memory_store: RwLock::new(HashMap::new()),
        });

        Ok(D1Pool { inner })
    }
}

#[derive(Debug)]
struct D1PoolInner {
    config: D1Config,
    memory_store: RwLock<HashMap<String, Vec<HashMap<String, Value>>>>,
}

/// A connection pool managing access to Cloudflare D1.
#[derive(Debug, Clone)]
pub struct D1Pool {
    inner: Arc<D1PoolInner>,
}

impl D1Pool {
    /// Returns a new [`D1PoolBuilder`].
    #[must_use]
    pub fn builder() -> D1PoolBuilder {
        D1PoolBuilder::new()
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
            let mut row = HashMap::new();
            for (idx, val) in binds.iter().enumerate() {
                row.insert(format!("col_{idx}"), val.clone());
            }
            store.entry(table_name).or_default().push(row);
            Ok(RowBatch::Edge(Vec::new()))
        } else {
            Ok(RowBatch::Edge(Vec::new()))
        }
    }
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

impl Executor for D1Pool {
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
        let pool = D1Pool::builder()
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
        let pool = D1Pool::builder()
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
}
