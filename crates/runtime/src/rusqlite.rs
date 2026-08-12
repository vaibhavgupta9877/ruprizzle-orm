//! Native `rusqlite` backend for the ruprizzle runtime.
//!
//! This module is compiled only when the `sqlite-rusqlite` feature is enabled.
//! It provides a synchronous, blocking-pinned SQLite connection pool that is
//! used instead of the `sqlx`-based SQLite backend when the connection URL
//! contains `driver=rusqlite`.

#![cfg(feature = "sqlite-rusqlite")]

use std::borrow::Cow;
use std::fmt;
use std::str::FromStr as _;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use ruprizzle_core::ir::Provider;
use ruprizzle_dialect::dialect_for;
use ::rusqlite::{self, OpenFlags, types::Value as RusqliteValue};

use crate::BoxFuture;
use crate::Error;
use crate::executor::{BoxRowStream, Executor, RowBatch};
use crate::pool::PoolConfig;
use crate::value::Value;

/// A single row from the `rusqlite` backend.
///
/// Columns are stored in result-set order as `rusqlite::types::Value` so that
/// decoding can be implemented without a `rusqlite::Row` borrow.
#[derive(Debug, Clone)]
pub struct Row(pub Vec<RusqliteValue>);

/// A pool of synchronous `rusqlite` connections.
///
/// This type is intentionally cheap to clone and holds a shared set of
/// `tokio::sync::Mutex<rusqlite::Connection>` handles. Operations are run on
/// the blocking pool so they do not starve the async runtime.
#[derive(Clone)]
pub struct RusqlitePool {
    inner: Arc<Inner>,
}

struct Inner {
    conns: Vec<Arc<tokio::sync::Mutex<rusqlite::Connection>>>,
    next: AtomicUsize,
}

impl RusqlitePool {
    /// Open a new `rusqlite` pool from a SQLite URL and configuration.
    ///
    /// # Errors
    ///
    /// Returns an error if the URL is not a valid SQLite URL or a connection
    /// cannot be opened.
    pub async fn connect(url: &str, config: &PoolConfig) -> Result<Self, Error> {
        let url = url.to_owned();
        let config = config.clone();

        let inner = tokio::task::spawn_blocking(move || -> Result<Inner, Error> {
            let opts = sqlx::sqlite::SqliteConnectOptions::from_str(&url)
                .map_err(Error::Sqlx)?;
            let filename = opts.get_filename().to_string_lossy().into_owned();
            let capacity = config.max_connections.max(1) as usize;
            let mut conns = Vec::with_capacity(capacity);

            for _ in 0..capacity {
                let conn = if filename == ":memory:" {
                    rusqlite::Connection::open_in_memory()
                } else {
                    rusqlite::Connection::open_with_flags(
                        &filename,
                        OpenFlags::SQLITE_OPEN_READ_WRITE
                            | OpenFlags::SQLITE_OPEN_CREATE
                            | OpenFlags::SQLITE_OPEN_NO_MUTEX,
                    )
                }
                .map_err(|e| Error::ConnectionFailure { reason: e.to_string() })?;

                // Use a short busy timeout so concurrent writers wait instead
                // of immediately returning SQLITE_BUSY.
                conn.busy_timeout(Duration::from_secs(5))
                    .map_err(|e| Error::ConnectionFailure { reason: e.to_string() })?;

                conns.push(Arc::new(tokio::sync::Mutex::new(conn)));
            }

            Ok(Inner {
                conns,
                next: AtomicUsize::new(0),
            })
        })
        .await
        .map_err(|e| Error::ConnectionFailure { reason: e.to_string() })?;

        Ok(Self {
            inner: Arc::new(inner?),
        })
    }

    /// Pick a connection using round-robin load distribution.
    fn acquire(&self) -> Arc<tokio::sync::Mutex<rusqlite::Connection>> {
        let idx = self
            .inner
            .next
            .fetch_add(1, Ordering::Relaxed)
            % self.inner.conns.len();
        self.inner.conns[idx].clone()
    }
}

impl fmt::Debug for RusqlitePool {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RusqlitePool")
            .field("connections", &self.inner.conns.len())
            .finish_non_exhaustive()
    }
}

impl fmt::Debug for Inner {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Inner")
            .field("connections", &self.conns.len())
            .finish_non_exhaustive()
    }
}

impl Executor for RusqlitePool {
    fn dialect(&self) -> Box<dyn ruprizzle_dialect::DbDialect> {
        dialect_for(Provider::Sqlite)
    }

    fn fetch_all_raw(
        &self,
        sql: Cow<'static, str>,
        binds: Vec<Value>,
    ) -> BoxFuture<'_, Result<RowBatch, Error>> {
        let pool = self.clone();
        Box::pin(async move {
            tokio::task::spawn_blocking(move || -> Result<RowBatch, Error> {
                let conn = pool.acquire();
                let binds = binds
                    .iter()
                    .map(value_to_rusqlite)
                    .collect::<Result<Vec<_>, _>>()?;

                let guard = conn.blocking_lock();
                let mut stmt = guard
                    .prepare_cached(sql.as_ref())
                    .map_err(|e| Error::Message(e.to_string()))?;

                let column_count = stmt.column_count();

                let rows = stmt
                    .query_map(
                        rusqlite::params_from_iter(binds.iter()),
                        |row| {
                            let mut values = Vec::with_capacity(column_count);
                            for i in 0..column_count {
                                values.push(row.get::<_, RusqliteValue>(i)?);
                            }
                            Ok(Row(values))
                        },
                    )
                    .map_err(|e| Error::Message(e.to_string()))?
                    .collect::<Result<Vec<_>, _>>()
                    .map_err(|e| Error::Message(e.to_string()))?;

                Ok(RowBatch::Rusqlite(rows))
            })
            .await
            .map_err(|e| Error::Message(e.to_string()))?
        })
    }

    fn execute_raw(
        &self,
        sql: Cow<'static, str>,
        binds: Vec<Value>,
    ) -> BoxFuture<'_, Result<u64, Error>> {
        let pool = self.clone();
        Box::pin(async move {
            tokio::task::spawn_blocking(move || -> Result<u64, Error> {
                let conn = pool.acquire();
                let binds = binds
                    .iter()
                    .map(value_to_rusqlite)
                    .collect::<Result<Vec<_>, _>>()?;

                let guard = conn.blocking_lock();
                let mut stmt = guard
                    .prepare_cached(sql.as_ref())
                    .map_err(|e| Error::Message(e.to_string()))?;

                let rows = stmt
                    .execute(rusqlite::params_from_iter(binds.iter()))
                    .map_err(|e| Error::Message(e.to_string()))?;

                Ok(rows as u64)
            })
            .await
            .map_err(|e| Error::Message(e.to_string()))?
        })
    }

    fn stream_raw(&self, sql: Cow<'static, str>, binds: Vec<Value>) -> BoxRowStream<'_> {
        Box::pin(crate::executor::DeferredRowStream::new(Box::pin(async move {
            self.fetch_all_raw(sql, binds).await
        })))
    }
}

/// Convert a ruprizzle `Value` into a `rusqlite::types::Value`.
fn value_to_rusqlite(value: &Value) -> Result<RusqliteValue, Error> {
    Ok(match value {
        Value::Null => RusqliteValue::Null,
        Value::Bool(b) => RusqliteValue::Integer(i64::from(*b)),
        Value::I32(i) => RusqliteValue::Integer(i64::from(*i)),
        Value::I64(i) => RusqliteValue::Integer(*i),
        Value::F64(f) => RusqliteValue::Real(*f),
        Value::Decimal(d) => RusqliteValue::Text(d.to_string()),
        Value::Str(s) => RusqliteValue::Text(s.to_string()),
        Value::Uuid(u) => RusqliteValue::Text(u.to_string()),
        Value::DateTime(dt) => RusqliteValue::Text(dt.to_rfc3339()),
        Value::Date(d) => RusqliteValue::Text(d.to_string()),
        Value::Time(t) => RusqliteValue::Text(t.to_string()),
        Value::Json(v) => RusqliteValue::Text(v.to_string()),
        Value::Bytes(b) => RusqliteValue::Blob(b.to_vec()),
        Value::Array(_) => return Err(Error::Message("array bind values are not supported yet".into())),
    })
}
