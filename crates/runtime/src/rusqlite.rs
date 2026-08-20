//! Native `rusqlite` backend for the ruprizzle runtime.
//!
//! This module is compiled only when the `sqlite-rusqlite` feature is enabled.
//! It provides a synchronous, blocking-pinned SQLite connection pool that is
//! used instead of the `sqlx`-based SQLite backend when the connection URL
//! contains `driver=rusqlite`.

#![cfg(feature = "sqlite-rusqlite")]

use std::borrow::Cow;
use std::fmt;
use std::str::FromStr;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use ::rusqlite::{self, OpenFlags, types::ValueRef};
use ruprizzle_core::ir::Provider;
use ruprizzle_dialect::dialect_for;

use crate::BoxFuture;
use crate::Error;
use crate::executor::{BoxRowStream, Executor, RowBatch};
use crate::pool::PoolConfig;
use crate::value::Value;

pub use ::rusqlite::{Row as RusqliteRow, types, types::Value as RusqliteValue};

/// A single row from the `rusqlite` backend.
///
/// Columns are stored in result-set order as `rusqlite::types::Value` so that
/// decoding can be implemented without holding a borrow of the live
/// `rusqlite::Row`. Column names are stored alongside the values so aggregate
/// result structs can decode by alias.
#[derive(Debug, Clone)]
pub struct Row {
    /// Column values in result-set order.
    pub values: Vec<RusqliteValue>,
    /// Column names in result-set order.
    pub names: Vec<String>,
}

/// A pool of synchronous `rusqlite` connections.
///
/// This type is intentionally cheap to clone and holds a shared set of
/// `std::sync::Mutex<rusqlite::Connection>` handles. Queries run synchronously
/// on the calling task to avoid the dispatch cost of `spawn_blocking`.
#[derive(Clone)]
pub struct RusqlitePool {
    inner: Arc<Inner>,
}

/// Checked-out connection handle that returns the connection on drop.
///
/// This lets one-shot `execute` and `fetch_all` paths share the same pool
/// lifecycle as transactions: the connection is removed from the pool for the
/// duration of the statement and returned afterwards.
struct ConnGuard<'a> {
    pool: &'a RusqlitePool,
    conn: Option<Arc<std::sync::Mutex<rusqlite::Connection>>>,
}

impl ConnGuard<'_> {
    fn conn(&self) -> &Arc<std::sync::Mutex<rusqlite::Connection>> {
        self.conn
            .as_ref()
            .unwrap_or_else(|| unreachable!("conn is present until consumed"))
    }
}

impl<'a> Drop for ConnGuard<'a> {
    fn drop(&mut self) {
        if let Some(conn) = self.conn.take() {
            self.pool.return_conn(conn);
        }
    }
}

struct Inner {
    conns: std::sync::Mutex<Vec<Arc<std::sync::Mutex<rusqlite::Connection>>>>,
    available: std::sync::Condvar,
    next: AtomicUsize,
    size: usize,
    waiters: AtomicUsize,
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
            let opts =
                sqlx::sqlite::SqliteConnectOptions::from_str(&sqlx_url).map_err(Error::Sqlx)?;
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
                .map_err(|e| Error::ConnectionFailure {
                    reason: e.to_string(),
                })?;

                // Use a long busy timeout so concurrent writers wait through
                // WAL checkpoints and transient contention rather than returning
                // SQLITE_BUSY. This is the primary defence against the `database
                // is locked` errors seen during long-running `rusqlite` soaks.
                conn.busy_timeout(Duration::from_secs(60)).map_err(|e| {
                    Error::ConnectionFailure {
                        reason: e.to_string(),
                    }
                })?;

                // SQLite leaves foreign keys off by default. Enabling them here
                // matches the sqlx-based SQLite backend and keeps relation tests
                // honest.
                conn.execute("PRAGMA foreign_keys = ON", []).map_err(|e| {
                    Error::ConnectionFailure {
                        reason: e.to_string(),
                    }
                })?;

                apply_pragmas(&conn, filename == ":memory:").map_err(|e| {
                    Error::ConnectionFailure {
                        reason: e.to_string(),
                    }
                })?;

                conns.push(Arc::new(std::sync::Mutex::new(conn)));
            }

            Ok(Inner {
                conns: std::sync::Mutex::new(conns),
                available: std::sync::Condvar::new(),
                next: AtomicUsize::new(0),
                size: capacity,
                waiters: AtomicUsize::new(0),
            })
        })
        .await
        .map_err(|e| Error::ConnectionFailure {
            reason: e.to_string(),
        })?;

        let pool = Self {
            inner: Arc::new(inner?),
        };

        tracing::info!(
            target: "ruprizzle::connection",
            event = "connect",
            backend = "rusqlite",
            "rusqlite pool opened"
        );

        Ok(pool)
    }

    /// Check out a connection from the pool, blocking until one is available.
    ///
    /// Callers must return the connection via [`ConnGuard`]'s `Drop` impl or
    /// by calling [`Self::return_conn`]. This replaces the previous round-robin
    /// "share a connection" model, which allowed `PoolExhausted` under heavy
    /// concurrent load because transactions removed connections while one-shot
    /// queries still expected to find one.
    fn checkout(&self) -> Result<ConnGuard<'_>, Error> {
        self.inner.waiters.fetch_add(1, Ordering::Relaxed);
        struct WaiterGuard<'a>(&'a Inner);
        impl Drop for WaiterGuard<'_> {
            fn drop(&mut self) {
                self.0.waiters.fetch_sub(1, Ordering::Relaxed);
            }
        }
        let _guard = WaiterGuard(&self.inner);
        let mut conns = self
            .inner
            .conns
            .lock()
            .map_err(|_| Error::Message("rusqlite connection pool mutex poisoned".into()))?;
        while conns.is_empty() {
            conns =
                self.inner.available.wait(conns).map_err(|_| {
                    Error::Message("rusqlite connection pool mutex poisoned".into())
                })?;
        }
        let conn = conns.pop().ok_or(Error::PoolExhausted {
            backend: "rusqlite",
        })?;
        self.inner.next.fetch_add(1, Ordering::Relaxed);
        Ok(ConnGuard {
            pool: self,
            conn: Some(conn),
        })
    }

    /// Number of connections in the pool.
    #[must_use]
    pub fn num_total(&self) -> usize {
        self.inner.size
    }

    /// Number of idle connections available for checkout.
    #[must_use]
    pub fn num_idle(&self) -> usize {
        self.inner.conns.try_lock().map_or(0, |conns| conns.len())
    }

    /// Number of threads currently waiting for a connection.
    #[must_use]
    pub fn num_waiters(&self) -> usize {
        self.inner.waiters.load(Ordering::Relaxed)
    }

    /// Check out a connection and take ownership of it (for transactions).
    fn checkout_owned(&self) -> Result<Arc<std::sync::Mutex<rusqlite::Connection>>, Error> {
        self.inner.waiters.fetch_add(1, Ordering::Relaxed);
        struct WaiterGuard<'a>(&'a Inner);
        impl Drop for WaiterGuard<'_> {
            fn drop(&mut self) {
                self.0.waiters.fetch_sub(1, Ordering::Relaxed);
            }
        }
        let _guard = WaiterGuard(&self.inner);
        let mut conns = self
            .inner
            .conns
            .lock()
            .map_err(|_| Error::Message("rusqlite connection pool mutex poisoned".into()))?;
        while conns.is_empty() {
            conns =
                self.inner.available.wait(conns).map_err(|_| {
                    Error::Message("rusqlite connection pool mutex poisoned".into())
                })?;
        }
        conns.pop().ok_or(Error::PoolExhausted {
            backend: "rusqlite",
        })
    }

    /// Return an owned connection to the pool.
    fn return_conn(&self, conn: Arc<std::sync::Mutex<rusqlite::Connection>>) {
        if let Ok(mut conns) = self.inner.conns.lock() {
            conns.push(conn);
        }
        self.inner.available.notify_one();
    }

    /// Take a connection from the pool and start a transaction on it.
    ///
    /// The connection is removed from the pool until the transaction is
    /// committed, rolled back, or dropped, guaranteeing that all transaction
    /// statements run on the same physical SQLite connection.
    pub(crate) async fn begin_transaction(&self) -> Result<RusqliteTransaction, Error> {
        let pool = self.clone();
        tokio::task::spawn_blocking(move || -> Result<RusqliteTransaction, Error> {
            let conn = pool.checkout_owned()?;

            // The connection is out of the pool from here on, and there is no
            // `RusqliteTransaction` yet to drop it back in, so `?` would leak it
            // exactly the way an abandoned transaction used to (BUG-01).
            let started = (|| -> Result<(), Error> {
                let guard = conn
                    .lock()
                    .map_err(|_| Error::Message("rusqlite connection mutex poisoned".into()))?;
                guard
                    .execute("BEGIN", [])
                    .map_err(|e| Error::Message(e.to_string()))?;
                // Each transaction can be the first operation on a connection
                // after another connection changed the schema; refresh so that
                // the transaction sees the current schema.
                force_schema_reload(&guard);
                Ok(())
            })();

            if let Err(error) = started {
                pool.return_conn(conn);
                return Err(error);
            }

            Ok(RusqliteTransaction {
                pool,
                conn: Some(conn),
            })
        })
        .await
        .map_err(|e| Error::Message(e.to_string()))?
    }

    /// Asynchronously fetch and decode rows directly into `Vec<T>`.
    ///
    /// This is the fast path used by `SelectQuery` when running against the
    /// native `rusqlite` backend. The synchronous `rusqlite` work is offloaded
    /// to a blocking thread so the tokio runtime is not pinned during WAL
    /// contention or pool waits.
    pub(crate) async fn fetch_all_sync_decoded<T>(
        &self,
        sql: Cow<'static, str>,
        binds: Vec<Value>,
    ) -> Result<Vec<T>, Error>
    where
        T: FromRusqliteRow + Send + 'static,
    {
        let pool = self.clone();
        tokio::task::spawn_blocking(move || {
            let conn = pool.checkout()?;

            let guard = conn
                .conn()
                .lock()
                .map_err(|_| Error::Message("rusqlite connection mutex poisoned".into()))?;
            if is_ddl(sql.as_ref()) {
                force_schema_reload(&guard);
            }
            let mut stmt = guard
                .prepare_cached(sql.as_ref())
                .map_err(|e| Error::Message(e.to_string()))?;

            let mut out = Vec::new();

            {
                let mut rows = stmt
                    .query(rusqlite::params_from_iter(&binds))
                    .map_err(|e| Error::Message(e.to_string()))?;
                while let Some(row) = rows.next().map_err(|e| Error::Message(e.to_string()))? {
                    out.push(T::from_rusqlite_row(row)?);
                }
            }

            if is_ddl(sql.as_ref()) {
                stmt.discard();
                guard.flush_prepared_statement_cache();
            }

            Ok(out)
        })
        .await
        .map_err(|e| Error::Message(e.to_string()))?
    }
}

/// A `rusqlite` transaction that owns its connection.
///
/// The connection is removed from the [`RusqlitePool`] for the lifetime of the
/// transaction and is returned by [`Self::commit`], [`Self::rollback`], or —
/// for a transaction abandoned without either — the [`Drop`] impl below.
///
/// `conn` is an `Option` purely so `Drop` can distinguish a transaction that
/// was finished explicitly (`None`) from one that was abandoned (`Some`);
/// `commit`/`rollback` consume `self`, so without it `Drop` would run on a
/// finished transaction and return the same connection twice.
///
/// **This type must never be `Clone`.** A transaction owns its connection
/// exclusively, and two handles to one connection would each return it to the
/// pool, after which `begin_transaction` could hand the same physical
/// connection — with one `BEGIN` on it — to two callers who both believe they
/// hold it alone. It carried a `Clone` derive until BUG-06; nothing needed it.
#[derive(Debug)]
pub(crate) struct RusqliteTransaction {
    pool: RusqlitePool,
    conn: Option<Arc<std::sync::Mutex<rusqlite::Connection>>>,
}

impl RusqliteTransaction {
    /// The connection this transaction owns, or an error if it has finished.
    fn conn(&self) -> Result<&Arc<std::sync::Mutex<rusqlite::Connection>>, Error> {
        self.conn
            .as_ref()
            .ok_or_else(|| Error::Message("transaction already finished".into()))
    }

    /// Execute `sql` with `binds`, returning the number of affected rows.
    pub(crate) fn execute_sync(&self, sql: &str, binds: &[Value]) -> Result<u64, Error> {
        let guard = self
            .conn()?
            .lock()
            .map_err(|_| Error::Message("rusqlite connection mutex poisoned".into()))?;
        if is_ddl(sql) {
            force_schema_reload(&guard);
        }
        let mut stmt = guard
            .prepare_cached(sql)
            .map_err(|e| Error::Message(e.to_string()))?;

        let rows = stmt
            .execute(rusqlite::params_from_iter(binds))
            .map_err(|e| Error::Message(e.to_string()))?;

        if is_ddl(sql) {
            stmt.discard();
            guard.flush_prepared_statement_cache();
        }

        Ok(rows as u64)
    }

    /// Fetch all rows from `sql` with `binds`.
    pub(crate) fn fetch_all_sync(&self, sql: &str, binds: &[Value]) -> Result<RowBatch, Error> {
        let guard = self
            .conn()?
            .lock()
            .map_err(|_| Error::Message("rusqlite connection mutex poisoned".into()))?;
        if is_ddl(sql) {
            force_schema_reload(&guard);
        }
        let mut stmt = guard
            .prepare_cached(sql)
            .map_err(|e| Error::Message(e.to_string()))?;

        let column_count = stmt.column_count();
        let column_names: Vec<String> = stmt
            .column_names()
            .into_iter()
            .map(|s| s.to_string())
            .collect();

        let rows = stmt
            .query_map(rusqlite::params_from_iter(binds), |row| {
                let mut values = Vec::with_capacity(column_count);
                for i in 0..column_count {
                    values.push(row.get::<_, RusqliteValue>(i)?);
                }
                Ok(Row {
                    values,
                    names: column_names.clone(),
                })
            })
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
    pub(crate) fn commit(mut self) -> Result<(), Error> {
        self.finish("COMMIT")
    }

    /// Roll the transaction back and return the connection to the pool.
    pub(crate) fn rollback(mut self) -> Result<(), Error> {
        self.finish("ROLLBACK")
    }

    /// Run `stmt` (`COMMIT` or `ROLLBACK`) and hand the connection back.
    ///
    /// The connection is taken out of the `Option` first, so it is returned to
    /// the pool exactly once and on every path — including a failed `COMMIT`.
    /// Leaking it on the failure path is half of what BUG-01 was.
    fn finish(&mut self, stmt: &'static str) -> Result<(), Error> {
        let Some(conn) = self.conn.take() else {
            return Err(Error::Message("transaction already finished".into()));
        };

        let result = end_transaction(&conn, stmt);
        self.pool.return_conn(conn);
        result
    }
}

/// Roll back and return the connection when a transaction is abandoned.
///
/// `sqlx::Transaction` does this for the `sqlx` backends; without it, dropping
/// a transaction — which `?` does on every early return — removed a connection
/// from the pool permanently (BUG-01).
///
/// This must never panic: a `Drop` that panics during an existing unwind aborts
/// the process. Every failure path here logs and continues, and the connection
/// goes back to the pool regardless.
impl Drop for RusqliteTransaction {
    fn drop(&mut self) {
        let Some(conn) = self.conn.take() else {
            return;
        };

        tracing::warn!(
            target: "ruprizzle::query",
            "transaction dropped without commit or rollback; rolling back"
        );

        if let Err(error) = end_transaction(&conn, "ROLLBACK") {
            tracing::warn!(
                target: "ruprizzle::query",
                error = %error,
                "failed to roll back an abandoned transaction"
            );
        }

        self.pool.return_conn(conn);
    }
}

/// Ends the transaction on `conn` with `stmt`, leaving no transaction open on
/// the connection whatever the outcome.
///
/// This is the last thing to touch a connection before it goes back into the
/// pool, so it must not leave one mid-transaction: a failed `COMMIT` does not
/// end the transaction in SQLite, and handing that connection to the next
/// caller would put their statements inside this transaction.
///
/// Never panics — it is called from `Drop`.
fn end_transaction(
    conn: &std::sync::Mutex<rusqlite::Connection>,
    stmt: &'static str,
) -> Result<(), Error> {
    // A poisoned mutex means another task panicked while holding it. The guard
    // is still recoverable, and ending the transaction matters more here than
    // honouring the poison flag.
    let guard = match conn.lock() {
        Ok(guard) => guard,
        Err(poisoned) => {
            tracing::warn!(
                target: "ruprizzle::query",
                "ending a transaction on a poisoned connection"
            );
            poisoned.into_inner()
        }
    };

    let result = guard
        .execute(stmt, [])
        .map(|_| ())
        .map_err(|e| Error::Message(e.to_string()));

    if result.is_err() && stmt != "ROLLBACK" {
        let _ = guard.execute("ROLLBACK", []);
    }

    // DDL can invalidate cached prepared statements. Flush before the
    // connection is reused so the next statement recompiles against the
    // current schema.
    guard.flush_prepared_statement_cache();

    result
}

impl fmt::Debug for RusqlitePool {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let len = self.inner.conns.try_lock().map_or(0, |conns| conns.len());
        f.debug_struct("RusqlitePool")
            .field("connections", &len)
            .finish_non_exhaustive()
    }
}

impl fmt::Debug for Inner {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let len = self.conns.try_lock().ok().map_or(0, |conns| conns.len());
        f.debug_struct("Inner")
            .field("connections", &len)
            .finish_non_exhaustive()
    }
}

impl Executor for RusqlitePool {
    fn dialect(&self) -> &dyn ruprizzle_dialect::DbDialect {
        dialect_for(Provider::Sqlite)
    }

    fn as_rusqlite(&self) -> Option<&Self> {
        Some(self)
    }

    fn fetch_all_raw(
        &self,
        sql: Cow<'static, str>,
        binds: Vec<Value>,
    ) -> BoxFuture<'_, Result<RowBatch, Error>> {
        let pool = self.clone();
        Box::pin(async move {
            tokio::task::spawn_blocking(move || fetch_all(pool, sql, binds))
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
            tokio::task::spawn_blocking(move || execute(pool, sql, binds))
                .await
                .map_err(|e| Error::Message(e.to_string()))?
        })
    }

    fn stream_raw(&self, sql: Cow<'static, str>, binds: Vec<Value>) -> BoxRowStream<'_> {
        Box::pin(crate::executor::DeferredRowStream::new(Box::pin(
            async move { self.fetch_all_raw(sql, binds).await },
        )))
    }
}

fn fetch_all(
    pool: RusqlitePool,
    sql: Cow<'static, str>,
    binds: Vec<Value>,
) -> Result<RowBatch, Error> {
    let conn = pool.checkout()?;

    let guard = conn
        .conn()
        .lock()
        .map_err(|_| Error::Message("rusqlite connection mutex poisoned".into()))?;
    if is_ddl(sql.as_ref()) {
        force_schema_reload(&guard);
    }
    let mut stmt = guard
        .prepare_cached(sql.as_ref())
        .map_err(|e| Error::Message(e.to_string()))?;

    let column_count = stmt.column_count();
    let column_names: Vec<String> = stmt
        .column_names()
        .into_iter()
        .map(|s| s.to_string())
        .collect();

    let rows = stmt
        .query_map(rusqlite::params_from_iter(&binds), |row| {
            let mut values = Vec::with_capacity(column_count);
            for i in 0..column_count {
                values.push(row.get::<_, RusqliteValue>(i)?);
            }
            Ok(Row {
                values,
                names: column_names.clone(),
            })
        })
        .map_err(|e| Error::Message(e.to_string()))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| Error::Message(e.to_string()))?;

    if is_ddl(sql.as_ref()) {
        stmt.discard();
        guard.flush_prepared_statement_cache();
    }

    Ok(RowBatch::Rusqlite(rows))
}

fn execute(pool: RusqlitePool, sql: Cow<'static, str>, binds: Vec<Value>) -> Result<u64, Error> {
    let conn = pool.checkout()?;

    let guard = conn
        .conn()
        .lock()
        .map_err(|_| Error::Message("rusqlite connection mutex poisoned".into()))?;
    if is_ddl(sql.as_ref()) {
        force_schema_reload(&guard);
    }
    let mut stmt = guard
        .prepare_cached(sql.as_ref())
        .map_err(|e| Error::Message(e.to_string()))?;

    let rows = stmt
        .execute(rusqlite::params_from_iter(&binds))
        .map_err(|e| Error::Message(e.to_string()))?;

    if is_ddl(sql.as_ref()) {
        stmt.discard();
        guard.flush_prepared_statement_cache();
    }

    Ok(rows as u64)
}

impl Row {
    /// Number of columns in the row.
    #[must_use]
    pub fn len(&self) -> usize {
        self.values.len()
    }

    /// Returns `true` if the row has no columns.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    /// Decode the column at `idx` into `T`.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Message`] if the index is out of bounds or the value
    /// cannot be decoded into `T`.
    pub fn get<T: FromValue>(&self, idx: usize) -> Result<T, Error> {
        let value = self
            .values
            .get(idx)
            .ok_or_else(|| Error::Message(format!("column index {idx} out of bounds")))?;
        T::from_value(value)
    }

    /// Decode the column named `name` into `T`, or return the default if the
    /// column is not present.
    ///
    /// This is used by aggregate result structs where the selected columns are a
    /// subset of the generated struct's fields.
    pub fn get_by_name<T: FromValue>(&self, name: &str) -> Result<T, Error> {
        match self.names.iter().position(|n| n == name) {
            Some(idx) => self.get::<T>(idx),
            None => T::from_value(&RusqliteValue::Null),
        }
    }
}

/// A type that can be decoded from a live `rusqlite::Row`.
pub trait FromRusqliteRow: Sized + Send + Sync + 'static {
    /// Decode `self` from an ordered row of `rusqlite` values.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Message`] if the row cannot be decoded.
    fn from_rusqlite_row(row: &RusqliteRow) -> Result<Self, Error>;
}

/// A type that can be decoded from a stored [`Row`].
pub trait FromOwnedRow: Sized + Send + Sync + 'static {
    /// Decode `self` from a stored [`Row`].
    ///
    /// # Errors
    ///
    /// Returns [`Error::Message`] if the row cannot be decoded.
    fn from_owned_row(row: &Row) -> Result<Self, Error>;
}

/// A type that can be decoded from a single `rusqlite::types::Value`.
pub trait FromValue: Sized + Send + Sync + 'static {
    /// Decode `self` from a single `rusqlite` value.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Message`] if the value cannot be decoded into `T`.
    fn from_value(value: &RusqliteValue) -> Result<Self, Error>;
}

impl FromValue for i64 {
    fn from_value(value: &RusqliteValue) -> Result<Self, Error> {
        match *value {
            RusqliteValue::Integer(i) => Ok(i),
            RusqliteValue::Real(f) => Ok(f as i64),
            RusqliteValue::Text(ref s) => s
                .parse()
                .map_err(|e| Error::Message(format!("cannot decode i64 from {s:?}: {e}"))),
            _ => Err(Error::Message(format!("cannot decode i64 from {value:?}"))),
        }
    }
}

impl FromValue for i32 {
    fn from_value(value: &RusqliteValue) -> Result<Self, Error> {
        i64::from_value(value)?
            .try_into()
            .map_err(|e| Error::Message(format!("cannot decode i32 from integer: {e}")))
    }
}

impl FromValue for f64 {
    fn from_value(value: &RusqliteValue) -> Result<Self, Error> {
        match value {
            &RusqliteValue::Real(f) => Ok(f),
            &RusqliteValue::Integer(i) => Ok(i as f64),
            RusqliteValue::Text(s) => s
                .parse()
                .map_err(|e| Error::Message(format!("cannot decode f64 from {s:?}: {e}"))),
            _ => Err(Error::Message(format!("cannot decode f64 from {value:?}"))),
        }
    }
}

impl FromValue for bool {
    fn from_value(value: &RusqliteValue) -> Result<Self, Error> {
        match value {
            &RusqliteValue::Integer(0) => Ok(false),
            &RusqliteValue::Integer(1) => Ok(true),
            RusqliteValue::Text(s) if s == "0" || s.eq_ignore_ascii_case("false") => Ok(false),
            RusqliteValue::Text(s) if s == "1" || s.eq_ignore_ascii_case("true") => Ok(true),
            _ => Err(Error::Message(format!("cannot decode bool from {value:?}"))),
        }
    }
}

impl FromValue for String {
    fn from_value(value: &RusqliteValue) -> Result<Self, Error> {
        match value {
            RusqliteValue::Text(s) => Ok(s.clone()),
            &RusqliteValue::Integer(i) => Ok(i.to_string()),
            &RusqliteValue::Real(f) => Ok(f.to_string()),
            &RusqliteValue::Blob(_) => Err(Error::Message("cannot decode String from blob".into())),
            &RusqliteValue::Null => Err(Error::Message("cannot decode String from NULL".into())),
        }
    }
}

impl FromValue for Vec<u8> {
    fn from_value(value: &RusqliteValue) -> Result<Self, Error> {
        match value {
            RusqliteValue::Blob(b) => Ok(b.clone()),
            _ => Err(Error::Message(format!(
                "cannot decode Vec<u8> from {value:?}"
            ))),
        }
    }
}

impl FromValue for crate::types::Decimal {
    fn from_value(value: &RusqliteValue) -> Result<Self, Error> {
        let s = String::from_value(value)?;
        s.parse()
            .map_err(|e| Error::Message(format!("cannot decode Decimal from {s:?}: {e}")))
    }
}

impl FromValue for crate::types::Uuid {
    fn from_value(value: &RusqliteValue) -> Result<Self, Error> {
        let s = String::from_value(value)?;
        s.parse()
            .map_err(|e| Error::Message(format!("cannot decode Uuid from {s:?}: {e}")))
    }
}

impl FromValue for crate::types::chrono::DateTime<crate::types::chrono::Utc> {
    fn from_value(value: &RusqliteValue) -> Result<Self, Error> {
        let s = String::from_value(value)?;
        s.parse()
            .map_err(|e| Error::Message(format!("cannot decode DateTime from {s:?}: {e}")))
    }
}

impl FromValue for crate::types::chrono::NaiveDate {
    fn from_value(value: &RusqliteValue) -> Result<Self, Error> {
        let s = String::from_value(value)?;
        s.parse()
            .map_err(|e| Error::Message(format!("cannot decode NaiveDate from {s:?}: {e}")))
    }
}

impl FromValue for crate::types::chrono::NaiveTime {
    fn from_value(value: &RusqliteValue) -> Result<Self, Error> {
        let s = String::from_value(value)?;
        s.parse()
            .map_err(|e| Error::Message(format!("cannot decode NaiveTime from {s:?}: {e}")))
    }
}

impl FromValue for serde_json::Value {
    fn from_value(value: &RusqliteValue) -> Result<Self, Error> {
        let s = String::from_value(value)?;
        serde_json::from_str(&s)
            .map_err(|e| Error::Message(format!("cannot decode JSON from {s:?}: {e}")))
    }
}

impl<T: FromValue + crate::serde::de::DeserializeOwned> FromValue for Vec<T> {
    fn from_value(value: &RusqliteValue) -> Result<Self, Error> {
        let s = String::from_value(value)?;
        serde_json::from_str(&s)
            .map_err(|e| Error::Message(format!("cannot decode array from {s:?}: {e}")))
    }
}

impl<T: FromValue> FromValue for Option<T> {
    fn from_value(value: &RusqliteValue) -> Result<Self, Error> {
        match value {
            &RusqliteValue::Null => Ok(None),
            _ => T::from_value(value).map(Some),
        }
    }
}

/// Decode a column into `T` by copying it into an owned `rusqlite::types::Value`.
pub fn get<T: FromValue>(row: &RusqliteRow, idx: usize) -> Result<T, Error> {
    let value = row
        .get::<_, RusqliteValue>(idx)
        .map_err(|e| Error::Message(e.to_string()))?;
    T::from_value(&value)
}

/// Decode an `INTEGER` column.
pub fn get_i64(row: &RusqliteRow, idx: usize) -> Result<i64, Error> {
    row.get::<_, i64>(idx)
        .map_err(|e| Error::Message(e.to_string()))
}

/// Decode an optional `INTEGER` column.
pub fn get_i64_opt(row: &RusqliteRow, idx: usize) -> Result<Option<i64>, Error> {
    row.get::<_, Option<i64>>(idx)
        .map_err(|e| Error::Message(e.to_string()))
}

/// Decode a `REAL` or `INTEGER` column as `f64`.
pub fn get_f64(row: &RusqliteRow, idx: usize) -> Result<f64, Error> {
    row.get::<_, f64>(idx)
        .map_err(|e| Error::Message(e.to_string()))
}

/// Decode an optional `REAL` or `INTEGER` column as `f64`.
pub fn get_f64_opt(row: &RusqliteRow, idx: usize) -> Result<Option<f64>, Error> {
    row.get::<_, Option<f64>>(idx)
        .map_err(|e| Error::Message(e.to_string()))
}

/// Decode a boolean column.
pub fn get_bool(row: &RusqliteRow, idx: usize) -> Result<bool, Error> {
    match row
        .get_ref(idx)
        .map_err(|e| Error::Message(e.to_string()))?
    {
        ValueRef::Null => Err(Error::Message("cannot decode bool from NULL".into())),
        ValueRef::Integer(0) => Ok(false),
        ValueRef::Integer(1) => Ok(true),
        ValueRef::Text(s) => {
            let s = std::str::from_utf8(s).map_err(|e| Error::Message(e.to_string()))?;
            if s == "0" || s.eq_ignore_ascii_case("false") {
                Ok(false)
            } else if s == "1" || s.eq_ignore_ascii_case("true") {
                Ok(true)
            } else {
                Err(Error::Message(format!("cannot decode bool from {s:?}")))
            }
        }
        v => Err(Error::Message(format!("cannot decode bool from {v:?}"))),
    }
}

/// Decode an optional boolean column.
pub fn get_bool_opt(row: &RusqliteRow, idx: usize) -> Result<Option<bool>, Error> {
    match row
        .get_ref(idx)
        .map_err(|e| Error::Message(e.to_string()))?
    {
        ValueRef::Null => Ok(None),
        ValueRef::Integer(0) => Ok(Some(false)),
        ValueRef::Integer(1) => Ok(Some(true)),
        ValueRef::Text(s) => {
            let s = std::str::from_utf8(s).map_err(|e| Error::Message(e.to_string()))?;
            if s == "0" || s.eq_ignore_ascii_case("false") {
                Ok(Some(false))
            } else if s == "1" || s.eq_ignore_ascii_case("true") {
                Ok(Some(true))
            } else {
                Err(Error::Message(format!("cannot decode bool from {s:?}")))
            }
        }
        v => Err(Error::Message(format!("cannot decode bool from {v:?}"))),
    }
}

/// Decode a `TEXT` column into a `String`.
pub fn get_text(row: &RusqliteRow, idx: usize) -> Result<String, Error> {
    row.get::<_, String>(idx)
        .map_err(|e| Error::Message(e.to_string()))
}

/// Decode an optional `TEXT` column.
pub fn get_text_opt(row: &RusqliteRow, idx: usize) -> Result<Option<String>, Error> {
    row.get::<_, Option<String>>(idx)
        .map_err(|e| Error::Message(e.to_string()))
}

/// Decode a `BLOB` column into `Vec<u8>`.
pub fn get_bytes(row: &RusqliteRow, idx: usize) -> Result<Vec<u8>, Error> {
    row.get::<_, Vec<u8>>(idx)
        .map_err(|e| Error::Message(e.to_string()))
}

/// Decode an optional `BLOB` column.
pub fn get_bytes_opt(row: &RusqliteRow, idx: usize) -> Result<Option<Vec<u8>>, Error> {
    row.get::<_, Option<Vec<u8>>>(idx)
        .map_err(|e| Error::Message(e.to_string()))
}

/// Parse a `TEXT` column into `T` using `FromStr`.
pub fn parse<T>(row: &RusqliteRow, idx: usize) -> Result<T, Error>
where
    T: FromStr,
    T::Err: fmt::Display,
{
    let s = row
        .get_ref(idx)
        .map_err(|e| Error::Message(e.to_string()))?
        .as_str()
        .map_err(|e| Error::Message(e.to_string()))?;
    s.parse::<T>()
        .map_err(|e| Error::Message(format!("cannot parse column {idx}: {e}")))
}

/// Parse an optional `TEXT` column into `Option<T>`.
pub fn parse_opt<T>(row: &RusqliteRow, idx: usize) -> Result<Option<T>, Error>
where
    T: FromStr,
    T::Err: fmt::Display,
{
    row.get_ref(idx)
        .map_err(|e| Error::Message(e.to_string()))?
        .as_str_or_null()
        .map_err(|e| Error::Message(e.to_string()))?
        .map(|s| {
            s.parse::<T>()
                .map_err(|e| Error::Message(format!("cannot parse column {idx}: {e}")))
        })
        .transpose()
}

/// Decode a JSON column.
pub fn get_json(row: &RusqliteRow, idx: usize) -> Result<serde_json::Value, Error> {
    let s = row
        .get_ref(idx)
        .map_err(|e| Error::Message(e.to_string()))?
        .as_str()
        .map_err(|e| Error::Message(e.to_string()))?;
    serde_json::from_str(s)
        .map_err(|e| Error::Message(format!("cannot decode JSON from column {idx}: {e}")))
}

/// Decode an optional JSON column.
pub fn get_json_opt(row: &RusqliteRow, idx: usize) -> Result<Option<serde_json::Value>, Error> {
    match row
        .get_ref(idx)
        .map_err(|e| Error::Message(e.to_string()))?
        .as_str_or_null()
        .map_err(|e| Error::Message(e.to_string()))?
    {
        None => Ok(None),
        Some(s) => serde_json::from_str(s)
            .map(Some)
            .map_err(|e| Error::Message(format!("cannot decode JSON from column {idx}: {e}"))),
    }
}

macro_rules! tuple_from_value {
    ($($T:ident $idx:tt),+) => {
        impl<$($T: FromValue),+> FromRusqliteRow for ($($T,)+) {
            fn from_rusqlite_row(row: &RusqliteRow) -> Result<Self, Error> {
                Ok((
                    $(
                        {
                            let value = row
                                .get::<_, RusqliteValue>($idx)
                                .map_err(|e| Error::Message(e.to_string()))?;
                            $T::from_value(&value)?
                        }
                    ,)+
                ))
            }
        }

        impl<$($T: FromValue),+> FromOwnedRow for ($($T,)+) {
            fn from_owned_row(row: &Row) -> Result<Self, Error> {
                Ok((
                    $(
                        $T::from_value(&row.values[$idx])?
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
tuple_from_value! { A 0, B 1, C 2, D 3, E 4, F 5, G 6, H 7, I 8 }
tuple_from_value! { A 0, B 1, C 2, D 3, E 4, F 5, G 6, H 7, I 8, J 9 }
tuple_from_value! { A 0, B 1, C 2, D 3, E 4, F 5, G 6, H 7, I 8, J 9, K 10 }
tuple_from_value! { A 0, B 1, C 2, D 3, E 4, F 5, G 6, H 7, I 8, J 9, K 10, L 11 }

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
        // Checkpoint less frequently than the default 1000 pages. The long soak
        // segments run their own explicit TRUNCATE checkpoint at the start of
        // each segment, so mid-segment auto-checkpoints should be large and rare.
        let _ = conn.pragma_update(None, "wal_autocheckpoint", 10_000i64);
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
    matches!(
        first.as_str(),
        "ALTER" | "CREATE" | "DROP" | "REINDEX" | "VACUUM"
    )
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
