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
    conns: tokio::sync::Mutex<Vec<Arc<tokio::sync::Mutex<rusqlite::Connection>>>>,
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
            // `driver=rusqlite` is a ruprizzle routing parameter that sqlx does
            // not understand, so strip it before parsing the SQLite URL.
            let sqlx_url = strip_driver_param(&url);
            let opts = sqlx::sqlite::SqliteConnectOptions::from_str(&sqlx_url)
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
                        OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_CREATE,
                    )
                }
                .map_err(|e| Error::ConnectionFailure { reason: e.to_string() })?;

                // Use a short busy timeout so concurrent writers wait instead
                // of immediately returning SQLITE_BUSY.
                conn.busy_timeout(Duration::from_secs(5))
                    .map_err(|e| Error::ConnectionFailure { reason: e.to_string() })?;

                // SQLite leaves foreign keys off by default. Enabling them here
                // matches the sqlx-based SQLite backend and keeps relation tests
                // honest.
                conn.execute("PRAGMA foreign_keys = ON", [])
                    .map_err(|e| Error::ConnectionFailure { reason: e.to_string() })?;

                apply_pragmas(&conn, filename == ":memory:")
                    .map_err(|e| Error::ConnectionFailure { reason: e.to_string() })?;

                conns.push(Arc::new(tokio::sync::Mutex::new(conn)));
            }

            Ok(Inner {
                conns: tokio::sync::Mutex::new(conns),
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
        let conns = self.inner.conns.blocking_lock();
        let idx = self.inner.next.fetch_add(1, Ordering::Relaxed) % conns.len();
        conns[idx].clone()
    }

    /// Take a connection from the pool and start a transaction on it.
    ///
    /// The connection is not returned to the pool until the transaction is
    /// committed or rolled back, guaranteeing that all transaction statements
    /// run on the same physical SQLite connection.
    pub(crate) async fn begin_transaction(&self) -> Result<RusqliteTransaction, Error> {
        let pool = self.clone();
        tokio::task::spawn_blocking(move || -> Result<RusqliteTransaction, Error> {
            let conn = {
                let mut conns = pool.inner.conns.blocking_lock();
                conns
                    .pop()
                    .ok_or_else(|| Error::Message("rusqlite connection pool exhausted".into()))?
            };

            {
                let guard = conn.blocking_lock();
                guard
                    .execute("BEGIN", [])
                    .map_err(|e| Error::Message(e.to_string()))?;
                // Each transaction can be the first operation on a connection
                // after another connection changed the schema; refresh so that
                // the transaction sees the current schema.
                force_schema_reload(&guard);
            }

            Ok(RusqliteTransaction { pool, conn })
        })
        .await
        .map_err(|e| Error::Message(e.to_string()))?
    }

    /// Return an owned connection to the pool.
    fn return_conn(&self, conn: Arc<tokio::sync::Mutex<rusqlite::Connection>>) {
        self.inner.conns.blocking_lock().push(conn);
    }
}

/// A `rusqlite` transaction that owns its connection.
///
/// The connection is removed from the [`RusqlitePool`] for the lifetime of the
/// transaction and is only returned on [`Self::commit`] or [`Self::rollback`].
#[derive(Debug, Clone)]
pub(crate) struct RusqliteTransaction {
    pool: RusqlitePool,
    conn: Arc<tokio::sync::Mutex<rusqlite::Connection>>,
}

impl RusqliteTransaction {
    /// Execute `sql` with `binds`, returning the number of affected rows.
    pub(crate) fn execute_sync(
        &self,
        sql: &str,
        binds: &[Value],
    ) -> Result<u64, Error> {
        let binds = binds
            .iter()
            .map(value_to_rusqlite)
            .collect::<Result<Vec<_>, _>>()?;

        let guard = self.conn.blocking_lock();
        if is_ddl(sql) {
            force_schema_reload(&guard);
        }
        let mut stmt = guard
            .prepare_cached(sql)
            .map_err(|e| Error::Message(e.to_string()))?;

        let rows = stmt
            .execute(rusqlite::params_from_iter(binds.iter()))
            .map_err(|e| Error::Message(e.to_string()))?;

        if is_ddl(sql) {
            stmt.discard();
            guard.flush_prepared_statement_cache();
        }

        Ok(rows as u64)
    }

    /// Fetch all rows from `sql` with `binds`.
    pub(crate) fn fetch_all_sync(
        &self,
        sql: &str,
        binds: &[Value],
    ) -> Result<RowBatch, Error> {
        let binds = binds
            .iter()
            .map(value_to_rusqlite)
            .collect::<Result<Vec<_>, _>>()?;

        let guard = self.conn.blocking_lock();
        if is_ddl(sql) {
            force_schema_reload(&guard);
        }
        let mut stmt = guard
            .prepare_cached(sql)
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

        if is_ddl(sql) {
            stmt.discard();
            guard.flush_prepared_statement_cache();
        }

        Ok(RowBatch::Rusqlite(rows))
    }

    /// Commit the transaction and return the connection to the pool.
    pub(crate) fn commit(self) -> Result<(), Error> {
        let conn = self.conn;
        let pool = self.pool;

        let guard = conn.blocking_lock();
        guard
            .execute("COMMIT", [])
            .map_err(|e| Error::Message(e.to_string()))?;
        // DDL can invalidate cached prepared statements. Flush before returning
        // the connection so the next statement recompiles against the current
        // schema.
        guard.flush_prepared_statement_cache();
        drop(guard);

        pool.return_conn(conn);
        Ok(())
    }

    /// Roll the transaction back and return the connection to the pool.
    pub(crate) fn rollback(self) -> Result<(), Error> {
        let conn = self.conn;
        let pool = self.pool;

        let guard = conn.blocking_lock();
        guard
            .execute("ROLLBACK", [])
            .map_err(|e| Error::Message(e.to_string()))?;
        guard.flush_prepared_statement_cache();
        drop(guard);

        pool.return_conn(conn);
        Ok(())
    }
}

impl fmt::Debug for RusqlitePool {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let len = self
            .inner
            .conns
            .try_lock()
            .map_or(0, |conns| conns.len());
        f.debug_struct("RusqlitePool")
            .field("connections", &len)
            .finish_non_exhaustive()
    }
}

impl fmt::Debug for Inner {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let len = self.conns.try_lock().map_or(0, |conns| conns.len());
        f.debug_struct("Inner")
            .field("connections", &len)
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
                if is_ddl(sql.as_ref()) {
                    force_schema_reload(&guard);
                }
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

                if is_ddl(sql.as_ref()) {
                    stmt.discard();
                    guard.flush_prepared_statement_cache();
                }

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
                if is_ddl(sql.as_ref()) {
                    force_schema_reload(&guard);
                }
                let mut stmt = guard
                    .prepare_cached(sql.as_ref())
                    .map_err(|e| Error::Message(e.to_string()))?;

                let rows = stmt
                    .execute(rusqlite::params_from_iter(binds.iter()))
                    .map_err(|e| Error::Message(e.to_string()))?;

                if is_ddl(sql.as_ref()) {
                    stmt.discard();
                    guard.flush_prepared_statement_cache();
                }

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

impl Row {
    /// Number of columns in the row.
    #[must_use]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Returns `true` if the row has no columns.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Decode the column at `idx` into `T`, cloning the underlying value.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Message`] if the index is out of bounds or the value
    /// cannot be decoded into `T`.
    pub fn get<T: FromValue>(&self, idx: usize) -> Result<T, Error> {
        let value = self
            .0
            .get(idx)
            .ok_or_else(|| Error::Message(format!("column index {idx} out of bounds")))?
            .clone();
        T::from_value(value)
    }

    /// Take the column at `idx` and decode it into `T` without cloning.
    ///
    /// The slot is replaced with `Null`, so this consumes the value.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Message`] if the index is out of bounds or the value
    /// cannot be decoded into `T`.
    pub fn take<T: FromValue>(&mut self, idx: usize) -> Result<T, Error> {
        if idx >= self.0.len() {
            return Err(Error::Message(format!("column index {idx} out of bounds")));
        }
        let value = std::mem::replace(&mut self.0[idx], RusqliteValue::Null);
        T::from_value(value)
    }
}

/// A type that can be decoded from a `crate::rusqlite::Row`.
pub trait FromRusqliteRow: Sized + Send + Sync + 'static {
    /// Decode `self` from an ordered row of `rusqlite` values.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Message`] if the row cannot be decoded.
    fn from_rusqlite_row(row: &mut Row) -> Result<Self, Error>;
}

/// A type that can be decoded from a single `rusqlite::types::Value`.
pub trait FromValue: Sized + Send + Sync + 'static {
    /// Decode `self` from a single `rusqlite` value.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Message`] if the value cannot be decoded into `T`.
    fn from_value(value: RusqliteValue) -> Result<Self, Error>;
}

impl FromValue for i64 {
    fn from_value(value: RusqliteValue) -> Result<Self, Error> {
        match value {
            RusqliteValue::Integer(i) => Ok(i),
            RusqliteValue::Real(f) => Ok(f as i64),
            RusqliteValue::Text(s) => s
                .parse()
                .map_err(|e| Error::Message(format!("cannot decode i64 from {s:?}: {e}"))),
            _ => Err(Error::Message(format!("cannot decode i64 from {value:?}"))),
        }
    }
}

impl FromValue for i32 {
    fn from_value(value: RusqliteValue) -> Result<Self, Error> {
        i64::from_value(value)?.try_into().map_err(|e| {
            Error::Message(format!("cannot decode i32 from integer: {e}"))
        })
    }
}

impl FromValue for f64 {
    fn from_value(value: RusqliteValue) -> Result<Self, Error> {
        match value {
            RusqliteValue::Real(f) => Ok(f),
            RusqliteValue::Integer(i) => Ok(i as f64),
            RusqliteValue::Text(s) => s
                .parse()
                .map_err(|e| Error::Message(format!("cannot decode f64 from {s:?}: {e}"))),
            _ => Err(Error::Message(format!("cannot decode f64 from {value:?}"))),
        }
    }
}

impl FromValue for bool {
    fn from_value(value: RusqliteValue) -> Result<Self, Error> {
        match value {
            RusqliteValue::Integer(0) => Ok(false),
            RusqliteValue::Integer(1) => Ok(true),
            RusqliteValue::Text(s) if s == "0" || s.eq_ignore_ascii_case("false") => Ok(false),
            RusqliteValue::Text(s) if s == "1" || s.eq_ignore_ascii_case("true") => Ok(true),
            _ => Err(Error::Message(format!("cannot decode bool from {value:?}"))),
        }
    }
}

impl FromValue for String {
    fn from_value(value: RusqliteValue) -> Result<Self, Error> {
        match value {
            RusqliteValue::Text(s) => Ok(s),
            RusqliteValue::Integer(i) => Ok(i.to_string()),
            RusqliteValue::Real(f) => Ok(f.to_string()),
            RusqliteValue::Blob(_) => Err(Error::Message("cannot decode String from blob".into())),
            RusqliteValue::Null => Err(Error::Message("cannot decode String from NULL".into())),
        }
    }
}

impl FromValue for crate::types::Decimal {
    fn from_value(value: RusqliteValue) -> Result<Self, Error> {
        let s = String::from_value(value)?;
        s.parse()
            .map_err(|e| Error::Message(format!("cannot decode Decimal from {s:?}: {e}")))
    }
}

impl FromValue for crate::types::Uuid {
    fn from_value(value: RusqliteValue) -> Result<Self, Error> {
        let s = String::from_value(value)?;
        s.parse()
            .map_err(|e| Error::Message(format!("cannot decode Uuid from {s:?}: {e}")))
    }
}

impl FromValue for crate::types::chrono::DateTime<crate::types::chrono::Utc> {
    fn from_value(value: RusqliteValue) -> Result<Self, Error> {
        let s = String::from_value(value)?;
        s.parse()
            .map_err(|e| Error::Message(format!("cannot decode DateTime from {s:?}: {e}")))
    }
}

impl FromValue for crate::types::chrono::NaiveDate {
    fn from_value(value: RusqliteValue) -> Result<Self, Error> {
        let s = String::from_value(value)?;
        s.parse()
            .map_err(|e| Error::Message(format!("cannot decode NaiveDate from {s:?}: {e}")))
    }
}

impl FromValue for crate::types::chrono::NaiveTime {
    fn from_value(value: RusqliteValue) -> Result<Self, Error> {
        let s = String::from_value(value)?;
        s.parse()
            .map_err(|e| Error::Message(format!("cannot decode NaiveTime from {s:?}: {e}")))
    }
}

impl FromValue for serde_json::Value {
    fn from_value(value: RusqliteValue) -> Result<Self, Error> {
        let s = String::from_value(value)?;
        serde_json::from_str(&s)
            .map_err(|e| Error::Message(format!("cannot decode JSON from {s:?}: {e}")))
    }
}

impl FromValue for Vec<u8> {
    fn from_value(value: RusqliteValue) -> Result<Self, Error> {
        match value {
            RusqliteValue::Blob(b) => Ok(b),
            _ => Err(Error::Message(format!("cannot decode Vec<u8> from {value:?}"))),
        }
    }
}

impl<T: FromValue> FromValue for Option<T> {
    fn from_value(value: RusqliteValue) -> Result<Self, Error> {
        match value {
            RusqliteValue::Null => Ok(None),
            _ => T::from_value(value).map(Some),
        }
    }
}

macro_rules! tuple_from_value {
    ($($T:ident $idx:tt),+) => {
        impl<$($T: FromValue),+> FromRusqliteRow for ($($T,)+) {
            fn from_rusqlite_row(row: &mut Row) -> Result<Self, Error> {
                Ok((
                    $(
                        row.take::<$T>($idx)?
                    ,)+
                ))
            }
        }
    };
}

tuple_from_value! { A 0 }
tuple_from_value! { A 0, B 1 }
tuple_from_value! { A 0, B 1, C 2 }
tuple_from_value! { A 0, B 1, C 2, D 3 }
tuple_from_value! { A 0, B 1, C 2, D 3, E 4 }
tuple_from_value! { A 0, B 1, C 2, D 3, E 4, F 5 }
tuple_from_value! { A 0, B 1, C 2, D 3, E 4, F 5, G 6 }
tuple_from_value! { A 0, B 1, C 2, D 3, E 4, F 5, G 6, H 7 }

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

/// Apply performance and correctness PRAGMAs to a fresh `rusqlite` connection.
///
/// WAL mode, a relaxed synchronous setting and a larger page/mmap cache are
/// standard SQLite tuning for local-file workloads.  They are applied per
/// connection; WAL is persistent once set on the database file.
fn apply_pragmas(conn: &rusqlite::Connection, is_memory: bool) -> Result<(), rusqlite::Error> {
    // WAL is not meaningful for in-memory databases and may error.
    if !is_memory {
        // `journal_mode` returns a row, so we need the checking variant.
        let _ = conn.pragma_update_and_check(None, "journal_mode", "WAL", |_| Ok(()));
    }

    conn.pragma_update(None, "synchronous", "NORMAL")?;
    conn.pragma_update(None, "temp_store", "MEMORY")?;
    conn.pragma_update(None, "cache_size", -64_000i64)?;
    conn.pragma_update(None, "mmap_size", 268_435_456i64)?;

    // Allow more prepared statements to stay live across queries; the DDL-safe
    // flush logic in commit/rollback keeps the cache consistent with schema
    // changes.
    conn.set_prepared_statement_cache_capacity(256);

    Ok(())
}

/// Force `rusqlite` to reload its schema cache for the current connection.
///
/// SQLite's schema cookie is checked when a statement is prepared, but because
/// the native backend may use one connection for a DDL write and a different
/// connection for a subsequent DDL read, the read connection can have a stale
/// schema view.  Querying `sqlite_master` forces a full schema re-parse before
/// the real DDL is prepared.
fn force_schema_reload(conn: &rusqlite::Connection) {
    // The result is unimportant; an empty database simply returns no rows.
    // `query_row` will trigger `sqlite3_prepare_v2()` and `sqlite3_step()`,
    // which is enough to refresh the in-memory schema tables when the schema
    // cookie has changed.
    let _ = conn.query_row("SELECT name FROM sqlite_master LIMIT 1", [], |_| Ok(()));
}

/// Heuristic for whether a statement may change the database schema.
fn is_ddl(sql: &str) -> bool {
    let mut s = sql.trim_start();
    // Skip leading single-line SQL comments so we don't misclassify a
    // commented-out DDL statement as a schema change.
    while s.starts_with("-- ") {
        if let Some(nl) = s.find('\n') {
            s = &s[nl + 1..];
        } else {
            return false;
        }
        s = s.trim_start();
    }

    let first = s
        .split_whitespace()
        .next()
        .unwrap_or("")
        .to_ascii_uppercase();
    matches!(first.as_str(), "ALTER" | "CREATE" | "DROP" | "REINDEX" | "VACUUM")
}

/// Strips the `driver=rusqlite` routing parameter from a SQLite URL so the
/// remainder can be parsed by `sqlx::sqlite::SqliteConnectOptions`.
fn strip_driver_param(url: &str) -> String {
    if let Some((base, query)) = url.split_once('?') {
        let mut parts = Vec::new();
        for part in query.split('&') {
            if part == "driver=rusqlite" || part.starts_with("driver=rusqlite&") {
                continue;
            }
            parts.push(part);
        }
        if parts.is_empty() {
            base.to_owned()
        } else {
            format!("{base}?{}", parts.join("&"))
        }
    } else {
        url.to_owned()
    }
}
