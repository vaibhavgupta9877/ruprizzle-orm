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
}

impl RowBatch {
    /// Returns `true` if the batch contains no rows.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        match self {
            Self::Any(rows) => rows.is_empty(),
            Self::Postgres(rows) => rows.is_empty(),
            Self::Sqlite(rows) => rows.is_empty(),
        }
    }

    /// Returns the number of rows in the batch.
    #[must_use]
    pub fn len(&self) -> usize {
        match self {
            Self::Any(rows) => rows.len(),
            Self::Postgres(rows) => rows.len(),
            Self::Sqlite(rows) => rows.len(),
        }
    }

    /// Consumes the batch and returns the underlying `AnyRow` rows.
    ///
    /// This is a temporary helper for `NestedSetter`, which still works with
    /// `Vec<AnyRow>`. It will be removed once child rows are decoded per-backend.
    pub(crate) fn into_any_rows(self) -> Result<Vec<AnyRow>, Error> {
        match self {
            Self::Any(rows) => Ok(rows),
            _ => Err(Error::Message(
                "native backend child rows are not yet implemented".into(),
            )),
        }
    }
}

impl std::fmt::Debug for RowBatch {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Any(rows) => f.debug_tuple("Any").field(&rows.len()).finish(),
            Self::Postgres(rows) => f.debug_tuple("Postgres").field(&rows.len()).finish(),
            Self::Sqlite(rows) => f.debug_tuple("Sqlite").field(&rows.len()).finish(),
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
    /// Takes the SQL by value: the returned future outlives the call, so
    /// borrowing the query text would force every caller into a
    /// self-referential struct. One allocation per statement is irrelevant
    /// beside a database round trip.
    fn fetch_all_raw(
        &self,
        sql: String,
        binds: Vec<Value>,
    ) -> BoxFuture<'_, Result<RowBatch, Error>>;

    /// Runs a statement and returns the number of affected rows.
    fn execute_raw(&self, sql: String, binds: Vec<Value>) -> BoxFuture<'_, Result<u64, Error>>;

    /// Runs a query and yields decoded rows from a buffered result set.
    ///
    /// This deliberately fetches all rows first and then streams them. A true
    /// cursor is slower on `sqlx-sqlite` (see `docs/BenchmarkResults.md`).
    fn stream_raw(&self, sql: String, binds: Vec<Value>) -> BoxRowStream<'_>;
}

/// A boxed stream of raw rows.
pub type BoxRowStream<'a> =
    std::pin::Pin<Box<dyn futures_core::Stream<Item = Result<AnyRow, Error>> + Send + 'a>>;

impl Executor for Pool {
    fn dialect(&self) -> Box<dyn DbDialect> {
        crate::compile::dialect_for_pool(self)
    }

    fn fetch_all_raw(
        &self,
        sql: String,
        binds: Vec<Value>,
    ) -> BoxFuture<'_, Result<RowBatch, Error>> {
        Box::pin(async move {
            let bind_count = binds.len();
            let started = std::time::Instant::now();
            let result = dispatch_raw_query(self, sql.clone(), binds).await;
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
        })
    }

    fn execute_raw(&self, sql: String, binds: Vec<Value>) -> BoxFuture<'_, Result<u64, Error>> {
        Box::pin(async move {
            let bind_count = binds.len();
            let started = std::time::Instant::now();
            let result = dispatch_raw_execute(self, sql.clone(), binds).await;
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
        })
    }

    fn stream_raw(&self, sql: String, binds: Vec<Value>) -> BoxRowStream<'_> {
        Box::pin(crate::tx::DeferredRowStream::new(Box::pin(async move {
            self.fetch_all_raw(sql, binds).await
        })))
    }
}

async fn dispatch_raw_query(
    pool: &Pool,
    sql: String,
    binds: Vec<Value>,
) -> Result<RowBatch, Error> {
    match pool {
        Pool::Any(p) => {
            let mut q = sqlx::query::<sqlx::Any>(&sql);
            for bind in binds {
                q = q.bind(bind);
            }
            q.fetch_all(p).await.map(RowBatch::Any).map_err(Error::from)
        }
        _ => unimplemented!("native backend queries need per-backend FromRow (P2-2)"),
    }
}

async fn dispatch_raw_execute(pool: &Pool, sql: String, binds: Vec<Value>) -> Result<u64, Error> {
    match pool {
        Pool::Any(p) => {
            let mut q = sqlx::query::<sqlx::Any>(&sql);
            for bind in binds {
                q = q.bind(bind);
            }
            q.execute(p)
                .await
                .map(|r| r.rows_affected())
                .map_err(Error::from)
        }
        _ => unimplemented!("native backend execute needs the Backend dispatch path (P2-2)"),
    }
}

/// Decodes a batch of raw rows into a `FromRow` type.
///
/// Shared by every builder so that the pool and transaction paths cannot drift
/// apart in how they decode.
pub(crate) fn decode_rows<T>(batch: RowBatch) -> Result<Vec<T>, Error>
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
    }
}
