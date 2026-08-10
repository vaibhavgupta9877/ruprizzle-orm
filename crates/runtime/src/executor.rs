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

use crate::BoxFuture;
use crate::error::Error;
use crate::pool::Pool;
use crate::value::Value;

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
    fn fetch_all_raw(&self, sql: String, binds: Vec<Value>)
        -> BoxFuture<'_, Result<Vec<AnyRow>, Error>>;

    /// Runs a statement and returns the number of affected rows.
    fn execute_raw(&self, sql: String, binds: Vec<Value>) -> BoxFuture<'_, Result<u64, Error>>;

    /// Runs a query and yields rows as they arrive.
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
    ) -> BoxFuture<'_, Result<Vec<AnyRow>, Error>> {
        Box::pin(async move {
            let mut q = sqlx::query::<sqlx::Any>(&sql);
            for b in binds {
                q = q.bind(b);
            }
            q.fetch_all(self).await.map_err(Error::Sqlx)
        })
    }

    fn execute_raw(&self, sql: String, binds: Vec<Value>) -> BoxFuture<'_, Result<u64, Error>> {
        Box::pin(async move {
            let mut q = sqlx::query::<sqlx::Any>(&sql);
            for b in binds {
                q = q.bind(b);
            }
            q.execute(self)
                .await
                .map(|r| r.rows_affected())
                .map_err(Error::Sqlx)
        })
    }

    fn stream_raw(&self, sql: String, binds: Vec<Value>) -> BoxRowStream<'_> {
        Box::pin(crate::tx::DeferredRowStream::new(Box::pin(async move {
            self.fetch_all_raw(sql, binds).await
        })))
    }
}

/// Decodes raw rows into a `FromRow` type.
///
/// Shared by every builder so that the pool and transaction paths cannot drift
/// apart in how they decode.
pub(crate) fn decode_rows<T>(rows: Vec<AnyRow>) -> Result<Vec<T>, Error>
where
    T: for<'r> sqlx::FromRow<'r, AnyRow>,
{
    rows.iter()
        .map(|r| T::from_row(r).map_err(Error::Sqlx))
        .collect()
}
