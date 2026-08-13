//! The `Executor` abstraction over a pool and a transaction.
//!
//! Every query builder runs against an `&dyn Executor` rather than a concrete
//! [`Pool`], which is what lets the same query work unchanged inside or outside
//! a transaction. Both [`Pool`] and [`Tx`](crate::tx::Tx) implement it, and
//! because `&Pool` coerces to `&dyn Executor` the call sites in generated code
//! do not have to know which one they hold.
//!
//! The trait deals in raw SQL and [`Value`] binds, not in typed rows: decoding
//! is the caller's job via `sqlx::FromRow`. Keeping decoding out of the trait is
//! what keeps it object-safe, and object safety is what makes the pool/tx
//! substitution possible at all.

use std::borrow::Cow;

use ruprizzle_dialect::DbDialect;
use sqlx::any::AnyRow;
use sqlx::postgres::PgRow;
use sqlx::sqlite::SqliteRow;

use crate::BoxFuture;
use crate::error::Error;
use crate::pool::Pool;
use crate::value::Value;

/// A batch of rows returned by an executor, before `FromRow` decoding.
///
/// `Executor` is object-safe, so it cannot be generic over the row type.
/// Returning a backend-tagged batch lets the same trait object carry `AnyRow`,
/// `PgRow`, or `SqliteRow` rows, and the caller decodes with the matching
/// `FromRow` implementation.
#[non_exhaustive]
pub enum RowBatch {
    /// Rows from the generic `sqlx::Any` driver.
    Any(Vec<AnyRow>),
    /// Rows from the native Postgres driver.
    Postgres(Vec<PgRow>),
    /// Rows from the native SQLite driver.
    Sqlite(Vec<SqliteRow>),
    /// Rows from the native `rusqlite` backend.
    #[cfg(feature = "sqlite-rusqlite")]
    Rusqlite(Vec<crate::rusqlite::Row>),
    /// Rows from the native `tokio-postgres` backend.
    #[cfg(feature = "postgres-tokio-postgres")]
    PostgresNative(Vec<tokio_postgres::Row>),
}

impl RowBatch {
    /// Returns `true` if the batch contains no rows.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        match self {
            Self::Any(rows) => rows.is_empty(),
            Self::Postgres(rows) => rows.is_empty(),
            Self::Sqlite(rows) => rows.is_empty(),
            #[cfg(feature = "sqlite-rusqlite")]
            Self::Rusqlite(rows) => rows.is_empty(),
            #[cfg(feature = "postgres-tokio-postgres")]
            Self::PostgresNative(rows) => rows.is_empty(),
        }
    }

    /// Returns the number of rows in the batch.
    #[must_use]
    pub fn len(&self) -> usize {
        match self {
            Self::Any(rows) => rows.len(),
            Self::Postgres(rows) => rows.len(),
            Self::Sqlite(rows) => rows.len(),
            #[cfg(feature = "sqlite-rusqlite")]
            Self::Rusqlite(rows) => rows.len(),
            #[cfg(feature = "postgres-tokio-postgres")]
            Self::PostgresNative(rows) => rows.len(),
        }
    }

    /// Merges another batch of the same backend into this one.
    ///
    /// Used by nested `INSERT ... RETURNING` to accumulate child rows across
    /// parameter-limit chunks.
    pub fn merge(&mut self, other: Self) -> Result<(), Error> {
        match (self, other) {
            (Self::Any(a), Self::Any(b)) => a.extend(b),
            (Self::Postgres(a), Self::Postgres(b)) => a.extend(b),
            (Self::Sqlite(a), Self::Sqlite(b)) => a.extend(b),
            #[cfg(feature = "sqlite-rusqlite")]
            (Self::Rusqlite(a), Self::Rusqlite(b)) => a.extend(b),
            #[cfg(feature = "postgres-tokio-postgres")]
            (Self::PostgresNative(a), Self::PostgresNative(b)) => a.extend(b),
            _ => {
                return Err(Error::Message(
                    "cannot merge row batches from different backends".into(),
                ));
            }
        }
        Ok(())
    }
}

impl std::fmt::Debug for RowBatch {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Any(rows) => f.debug_tuple("Any").field(&rows.len()).finish(),
            Self::Postgres(rows) => f.debug_tuple("Postgres").field(&rows.len()).finish(),
            Self::Sqlite(rows) => f.debug_tuple("Sqlite").field(&rows.len()).finish(),
            #[cfg(feature = "sqlite-rusqlite")]
            Self::Rusqlite(rows) => f.debug_tuple("Rusqlite").field(&rows.len()).finish(),
            #[cfg(feature = "postgres-tokio-postgres")]
            Self::PostgresNative(rows) => {
                f.debug_tuple("PostgresNative").field(&rows.len()).finish()
            }
        }
    }
}

/// Something that can run SQL: a connection pool or an open transaction.
pub trait Executor: Send + Sync {
    /// The dialect for the backend behind this executor.
    ///
    /// Query compilation needs this to pick identifier quoting and placeholder
    /// syntax, so it must be answerable without touching the database.
    fn dialect(&self) -> Box<dyn DbDialect>;

    /// Runs a query and returns the raw rows.
    ///
    /// `sql` is a [`Cow`] so callers with an owned `String` can pass it without
    /// an extra clone, while the executor can still borrow the text to hand to
    /// `sqlx`. The returned future owns the `Cow` and the binds.
    fn fetch_all_raw(
        &self,
        sql: Cow<'static, str>,
        binds: Vec<Value>,
    ) -> BoxFuture<'_, Result<RowBatch, Error>>;

    /// Runs a statement and returns the number of affected rows.
    fn execute_raw(
        &self,
        sql: Cow<'static, str>,
        binds: Vec<Value>,
    ) -> BoxFuture<'_, Result<u64, Error>>;

    /// Runs a query and yields decoded rows from a buffered result set.
    ///
    /// This deliberately fetches all rows first and then streams them. A true
    /// cursor is slower on `sqlx-sqlite` (see `docs/BenchmarkResults.md`).
    fn stream_raw(&self, sql: Cow<'static, str>, binds: Vec<Value>) -> BoxRowStream<'_>;

    /// Optional hook called right before a query is executed.
    ///
    /// The default is a no-op; wrappers such as [`CountingExecutor`](crate::CountingExecutor) override it
    /// to record the statement when the caller takes a backend-specific fast
    /// path.
    fn on_query(&self) {}

    /// Returns the underlying [`rusqlite::RusqlitePool`] if this executor is
    /// backed by the native `rusqlite` backend.
    ///
    /// Used by query builders to take a direct, single-pass decode path on
    /// SQLite. Returns `None` by default.
    #[cfg(feature = "sqlite-rusqlite")]
    fn as_rusqlite(&self) -> Option<&crate::rusqlite::RusqlitePool> {
        None
    }
}

/// A single raw row from any backend.
///
/// Streaming keeps the executor object-safe by returning an untyped row that the
/// caller decodes with the matching `FromRow` implementation.
#[non_exhaustive]
pub enum RawRow {
    /// A row from the generic `sqlx::Any` driver.
    Any(AnyRow),
    /// A row from the native Postgres driver.
    Postgres(PgRow),
    /// A row from the native SQLite driver.
    Sqlite(SqliteRow),
    /// A row from the native `rusqlite` backend.
    #[cfg(feature = "sqlite-rusqlite")]
    Rusqlite(crate::rusqlite::Row),
    /// A row from the native `tokio-postgres` backend.
    #[cfg(feature = "postgres-tokio-postgres")]
    PostgresNative(tokio_postgres::Row),
}

/// A boxed stream of raw rows.
pub type BoxRowStream<'a> =
    std::pin::Pin<Box<dyn futures_core::Stream<Item = Result<RawRow, Error>> + Send + 'a>>;

/// Resolves a pending fetch, then yields its rows one at a time.
///
/// Both executors currently buffer: a `Tx` must, because it owns one connection
/// behind a mutex and an open cursor would block every other statement on the
/// transaction. The `Pool` shares this path so the two cannot drift; swapping
/// it for a true incremental cursor is a `Pool`-only change behind this type.
pub(crate) struct DeferredRowStream<'a> {
    fut: crate::BoxFuture<'a, Result<RowBatch, Error>>,
    done: bool,
    buffered: std::vec::IntoIter<RawRow>,
}

impl<'a> DeferredRowStream<'a> {
    pub(crate) fn new(fut: crate::BoxFuture<'a, Result<RowBatch, Error>>) -> Self {
        Self {
            fut,
            done: false,
            buffered: Vec::new().into_iter(),
        }
    }
}

impl futures_core::Stream for DeferredRowStream<'_> {
    type Item = Result<RawRow, Error>;

    fn poll_next(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        use std::task::Poll;

        let this = self.get_mut();

        if !this.done {
            match this.fut.as_mut().poll(cx) {
                Poll::Pending => return Poll::Pending,
                Poll::Ready(Err(e)) => {
                    this.done = true;
                    return Poll::Ready(Some(Err(e)));
                }
                Poll::Ready(Ok(batch)) => {
                    this.done = true;
                    this.buffered = match batch {
                        RowBatch::Any(rows) => {
                            rows.into_iter().map(RawRow::Any).collect::<Vec<_>>()
                        }
                        RowBatch::Postgres(rows) => {
                            rows.into_iter().map(RawRow::Postgres).collect::<Vec<_>>()
                        }
                        RowBatch::Sqlite(rows) => {
                            rows.into_iter().map(RawRow::Sqlite).collect::<Vec<_>>()
                        }
                        #[cfg(feature = "sqlite-rusqlite")]
                        RowBatch::Rusqlite(rows) => {
                            rows.into_iter().map(RawRow::Rusqlite).collect::<Vec<_>>()
                        }
                        #[cfg(feature = "postgres-tokio-postgres")]
                        RowBatch::PostgresNative(rows) => rows
                            .into_iter()
                            .map(RawRow::PostgresNative)
                            .collect::<Vec<_>>(),
                    }
                    .into_iter();
                }
            }
        }

        Poll::Ready(this.buffered.next().map(Ok))
    }
}

impl Executor for Pool {
    fn dialect(&self) -> Box<dyn DbDialect> {
        crate::compile::dialect_for_pool(self)
    }

    #[cfg(feature = "sqlite-rusqlite")]
    fn as_rusqlite(&self) -> Option<&crate::rusqlite::RusqlitePool> {
        match self {
            Pool::SqliteNative(p) => Some(p),
            _ => None,
        }
    }

    fn fetch_all_raw(
        &self,
        sql: Cow<'static, str>,
        binds: Vec<Value>,
    ) -> BoxFuture<'_, Result<RowBatch, Error>> {
        Box::pin(async move {
            let bind_count = binds.len();
            if tracing::enabled!(target: "ruprizzle::query", tracing::Level::DEBUG) {
                let started = std::time::Instant::now();
                let result = dispatch_raw_query(self, sql.clone(), binds.clone()).await;
                let elapsed_ms = started.elapsed().as_millis() as u64;
                match &result {
                    Ok(batch) => tracing::debug!(
                        target: "ruprizzle::query",
                        sql = %sql,
                        binds = bind_count,
                        rows = batch.len(),
                        elapsed_ms,
                        "query"
                    ),
                    Err(error) => tracing::warn!(
                        target: "ruprizzle::query",
                        sql = %sql,
                        binds = bind_count,
                        elapsed_ms,
                        error = error.kind(),
                        "query failed"
                    ),
                }
                result
            } else {
                dispatch_raw_query(self, sql, binds).await
            }
        })
    }

    fn execute_raw(
        &self,
        sql: Cow<'static, str>,
        binds: Vec<Value>,
    ) -> BoxFuture<'_, Result<u64, Error>> {
        Box::pin(async move {
            let bind_count = binds.len();
            if tracing::enabled!(target: "ruprizzle::query", tracing::Level::DEBUG) {
                let started = std::time::Instant::now();
                let result = dispatch_raw_execute(self, sql.clone(), binds.clone()).await;
                let elapsed_ms = started.elapsed().as_millis() as u64;
                match &result {
                    Ok(rows_affected) => tracing::debug!(
                        target: "ruprizzle::query",
                        sql = %sql,
                        binds = bind_count,
                        rows_affected,
                        elapsed_ms,
                        "execute"
                    ),
                    Err(error) => tracing::warn!(
                        target: "ruprizzle::query",
                        sql = %sql,
                        binds = bind_count,
                        elapsed_ms,
                        error = error.kind(),
                        "execute failed"
                    ),
                }
                result
            } else {
                dispatch_raw_execute(self, sql, binds).await
            }
        })
    }

    fn stream_raw(&self, sql: Cow<'static, str>, binds: Vec<Value>) -> BoxRowStream<'_> {
        Box::pin(DeferredRowStream::new(Box::pin(async move {
            self.fetch_all_raw(sql, binds).await
        })))
    }
}

async fn dispatch_raw_query(
    pool: &Pool,
    sql: Cow<'static, str>,
    binds: Vec<Value>,
) -> Result<RowBatch, Error> {
    match pool {
        Pool::Any(p) => {
            let mut q = sqlx::query::<sqlx::Any>(&sql);
            for bind in &binds {
                q = q.bind(bind);
            }
            q.fetch_all(p).await.map(RowBatch::Any).map_err(Error::from)
        }
        Pool::Postgres(p) => {
            let mut q = sqlx::query::<sqlx::Postgres>(&sql);
            for bind in &binds {
                q = q.bind(bind);
            }
            q.fetch_all(p)
                .await
                .map(RowBatch::Postgres)
                .map_err(Error::from)
        }
        Pool::Sqlite(p) => {
            let mut q = sqlx::query::<sqlx::Sqlite>(&sql);
            for bind in &binds {
                q = q.bind(bind);
            }
            q.fetch_all(p)
                .await
                .map(RowBatch::Sqlite)
                .map_err(Error::from)
        }
        #[cfg(feature = "sqlite-rusqlite")]
        Pool::SqliteNative(p) => Executor::fetch_all_raw(p, sql, binds).await,
        #[cfg(feature = "postgres-tokio-postgres")]
        Pool::PostgresNative(p) => Executor::fetch_all_raw(p, sql, binds).await,
    }
}

async fn dispatch_raw_execute(
    pool: &Pool,
    sql: Cow<'static, str>,
    binds: Vec<Value>,
) -> Result<u64, Error> {
    match pool {
        Pool::Any(p) => {
            let mut q = sqlx::query::<sqlx::Any>(&sql);
            for bind in &binds {
                q = q.bind(bind);
            }
            q.execute(p)
                .await
                .map(|r| r.rows_affected())
                .map_err(Error::from)
        }
        Pool::Postgres(p) => {
            let mut q = sqlx::query::<sqlx::Postgres>(&sql);
            for bind in &binds {
                q = q.bind(bind);
            }
            q.execute(p)
                .await
                .map(|r| r.rows_affected())
                .map_err(Error::from)
        }
        Pool::Sqlite(p) => {
            let mut q = sqlx::query::<sqlx::Sqlite>(&sql);
            for bind in &binds {
                q = q.bind(bind);
            }
            q.execute(p)
                .await
                .map(|r| r.rows_affected())
                .map_err(Error::from)
        }
        #[cfg(feature = "sqlite-rusqlite")]
        Pool::SqliteNative(p) => Executor::execute_raw(p, sql, binds).await,
        #[cfg(feature = "postgres-tokio-postgres")]
        Pool::PostgresNative(p) => Executor::execute_raw(p, sql, binds).await,
    }
}

/// Decodes a batch of raw rows into a `FromRow` type.
///
/// Shared by every builder so that the pool and transaction paths cannot drift
/// apart in how they decode.
pub fn decode_rows<T>(batch: RowBatch) -> Result<Vec<T>, Error>
where
    T: crate::model::RowDecode,
{
    match batch {
        RowBatch::Any(rows) => rows
            .iter()
            .map(|r| T::from_row(r).map_err(Error::Sqlx))
            .collect(),
        RowBatch::Postgres(rows) => rows
            .iter()
            .map(|r| T::from_row(r).map_err(Error::Sqlx))
            .collect(),
        RowBatch::Sqlite(rows) => rows
            .iter()
            .map(|r| T::from_row(r).map_err(Error::Sqlx))
            .collect(),
        #[cfg(feature = "sqlite-rusqlite")]
        RowBatch::Rusqlite(rows) => rows.iter().map(|r| T::from_owned_row(r)).collect(),
        #[cfg(feature = "postgres-tokio-postgres")]
        RowBatch::PostgresNative(rows) => {
            rows.iter().map(|r| T::from_tokio_postgres_row(r)).collect()
        }
    }
}
