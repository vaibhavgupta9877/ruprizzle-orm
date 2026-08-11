//! Transaction handle.

use tokio::sync::Mutex;

use ruprizzle_core::ir::Provider;
use ruprizzle_dialect::{DbDialect, dialect_for};
use sqlx::Any;

use crate::Error;
use crate::pool::Pool;
use crate::value::Value;

/// The isolation level for a transaction.
///
/// Postgres honours all three. SQLite has a single writer and is effectively
/// serializable already, so the level is accepted and ignored there rather than
/// failing — the same application code has to run on both backends.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IsolationLevel {
    /// Each statement sees rows committed before it began.
    ReadCommitted,
    /// All statements see the snapshot taken at the first read.
    RepeatableRead,
    /// Full serializability; may abort with a serialization failure.
    Serializable,
}

impl IsolationLevel {
    /// The SQL fragment used in `SET TRANSACTION ISOLATION LEVEL ...`.
    #[must_use]
    pub const fn as_sql(self) -> &'static str {
        match self {
            Self::ReadCommitted => "READ COMMITTED",
            Self::RepeatableRead => "REPEATABLE READ",
            Self::Serializable => "SERIALIZABLE",
        }
    }
}

/// A transaction in progress.
#[derive(Debug)]
pub struct Tx {
    inner: Mutex<Option<sqlx::Transaction<'static, Any>>>,
    provider: Provider,
}

impl Tx {
    /// Begins a new transaction on `pool`.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Sqlx`] if the database cannot begin a transaction.
    pub async fn begin(pool: &Pool) -> Result<Self, Error> {
        let tx = pool.begin().await.map_err(Error::Sqlx)?;
        Ok(Self {
            inner: Mutex::new(Some(tx)),
            provider: provider_of(pool),
        })
    }

    /// Begins a new transaction with an explicit isolation level.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Sqlx`] if the transaction cannot be started or the
    /// level cannot be set.
    pub async fn begin_with_isolation(pool: &Pool, level: IsolationLevel) -> Result<Self, Error> {
        let this = Self::begin(pool).await?;
        if this.provider == Provider::Postgres {
            this.execute(
                &format!("SET TRANSACTION ISOLATION LEVEL {}", level.as_sql()),
                Vec::new(),
            )
            .await?;
        }
        Ok(this)
    }

    /// The dialect for the backend this transaction runs on.
    #[must_use]
    pub fn dialect(&self) -> Box<dyn DbDialect> {
        dialect_for(self.provider)
    }

    /// Commits the transaction.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Sqlx`] if the commit fails.
    pub async fn commit(self) -> Result<(), Error> {
        let tx = self.inner.lock().await.take();
        if let Some(tx) = tx {
            tx.commit().await.map_err(Error::Sqlx)?;
            tracing::debug!(target: "ruprizzle::query", "transaction committed");
        }
        Ok(())
    }

    /// Rolls back the transaction.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Sqlx`] if the rollback fails.
    pub async fn rollback(self) -> Result<(), Error> {
        let tx = self.inner.lock().await.take();
        if let Some(tx) = tx {
            tx.rollback().await.map_err(Error::Sqlx)?;
            tracing::debug!(target: "ruprizzle::query", "transaction rolled back");
        }
        Ok(())
    }

    /// Executes a raw statement inside the transaction, returning rows affected.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Sqlx`] for database errors or [`Error::Message`] if the
    /// transaction has already been finished.
    pub async fn execute(&self, sql: &str, binds: Vec<Value>) -> Result<u64, Error> {
        let mut guard = self.inner.lock().await;
        let tx = guard
            .as_mut()
            .ok_or_else(|| Error::Message("transaction already finished".into()))?;

        let mut q = sqlx::query::<Any>(sql);
        for b in binds {
            q = q.bind(b);
        }
        q.execute(&mut **tx)
            .await
            .map(|r| r.rows_affected())
            .map_err(Error::Sqlx)
    }

    /// Fetches all rows from a raw statement inside the transaction.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Sqlx`] for database errors or [`Error::Message`] if the
    /// transaction has already been finished.
    pub async fn fetch_all<T>(&self, sql: &str, binds: Vec<Value>) -> Result<Vec<T>, Error>
    where
        T: Send + Unpin + for<'r> sqlx::FromRow<'r, sqlx::any::AnyRow>,
    {
        let mut guard = self.inner.lock().await;
        let tx = guard
            .as_mut()
            .ok_or_else(|| Error::Message("transaction already finished".into()))?;

        let mut q = sqlx::query_as::<Any, T>(sql);
        for b in binds {
            q = q.bind(b);
        }
        q.fetch_all(&mut **tx).await.map_err(Error::Sqlx)
    }

    /// Fetches one row from a raw statement inside the transaction.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Sqlx`] for database errors or [`Error::Message`] if the
    /// transaction has already been finished.
    pub async fn fetch_one<T>(&self, sql: &str, binds: Vec<Value>) -> Result<T, Error>
    where
        T: Send + Unpin + for<'r> sqlx::FromRow<'r, sqlx::any::AnyRow>,
    {
        let mut guard = self.inner.lock().await;
        let tx = guard
            .as_mut()
            .ok_or_else(|| Error::Message("transaction already finished".into()))?;

        let mut q = sqlx::query_as::<Any, T>(sql);
        for b in binds {
            q = q.bind(b);
        }
        q.fetch_one(&mut **tx).await.map_err(Error::Sqlx)
    }

    /// Fetches one row from a raw statement inside the transaction.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Sqlx`] for database errors or [`Error::Message`] if the
    /// transaction has already been finished.
    pub async fn fetch_optional<T>(&self, sql: &str, binds: Vec<Value>) -> Result<Option<T>, Error>
    where
        T: Send + Unpin + for<'r> sqlx::FromRow<'r, sqlx::any::AnyRow>,
    {
        let mut guard = self.inner.lock().await;
        let tx = guard
            .as_mut()
            .ok_or_else(|| Error::Message("transaction already finished".into()))?;

        let mut q = sqlx::query_as::<Any, T>(sql);
        for b in binds {
            q = q.bind(b);
        }
        q.fetch_optional(&mut **tx).await.map_err(Error::Sqlx)
    }

    /// Fetches raw rows inside the transaction.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Sqlx`] for database errors or [`Error::Message`] if the
    /// transaction has already been finished.
    pub async fn fetch_all_rows(
        &self,
        sql: &str,
        binds: Vec<Value>,
    ) -> Result<Vec<sqlx::any::AnyRow>, Error> {
        let mut guard = self.inner.lock().await;
        let tx = guard
            .as_mut()
            .ok_or_else(|| Error::Message("transaction already finished".into()))?;

        let mut q = sqlx::query::<Any>(sql);
        for b in binds {
            q = q.bind(b);
        }
        q.fetch_all(&mut **tx).await.map_err(Error::Sqlx)
    }
}

/// Reads the provider from a pool's URL scheme.
fn provider_of(pool: &Pool) -> Provider {
    let opts = pool.connect_options();
    Provider::parse(opts.database_url.scheme()).unwrap_or(Provider::Postgres)
}

impl crate::executor::Executor for Tx {
    fn dialect(&self) -> Box<dyn DbDialect> {
        Self::dialect(self)
    }

    fn fetch_all_raw(
        &self,
        sql: String,
        binds: Vec<Value>,
    ) -> crate::BoxFuture<'_, Result<Vec<sqlx::any::AnyRow>, Error>> {
        Box::pin(async move {
            let bind_count = binds.len();
            let started = std::time::Instant::now();
            let result = self.fetch_all_rows(&sql, binds).await;
            let elapsed_ms = started.elapsed().as_millis() as u64;
            match &result {
                Ok(rows) => tracing::debug!(
                    target: "ruprizzle::query",
                    sql = %sql,
                    binds = bind_count,
                    rows = rows.len(),
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

    fn execute_raw(
        &self,
        sql: String,
        binds: Vec<Value>,
    ) -> crate::BoxFuture<'_, Result<u64, Error>> {
        Box::pin(async move {
            let bind_count = binds.len();
            let started = std::time::Instant::now();
            let result = self.execute(&sql, binds).await;
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

    /// Buffers rather than streaming incrementally.
    ///
    /// A transaction owns a single connection behind a mutex. Holding a cursor
    /// open across awaits would keep that mutex locked and deadlock every other
    /// statement issued on the same transaction, so the rows are fetched up
    /// front. Streaming a very large result set is therefore something to do on
    /// the pool, not inside a transaction.
    fn stream_raw(&self, sql: String, binds: Vec<Value>) -> crate::executor::BoxRowStream<'_> {
        Box::pin(DeferredRowStream::new(Box::pin(async move {
            crate::executor::Executor::fetch_all_raw(self, sql, binds).await
        })))
    }
}

/// Resolves a pending fetch, then yields its rows one at a time.
///
/// Both executors currently buffer: a `Tx` must, because it owns one connection
/// behind a mutex and an open cursor would block every other statement on the
/// transaction. The `Pool` shares this path so the two cannot drift; swapping
/// it for a true incremental cursor is a `Pool`-only change behind this type.
pub(crate) struct DeferredRowStream<'a> {
    fut: crate::BoxFuture<'a, Result<Vec<sqlx::any::AnyRow>, Error>>,
    done: bool,
    buffered: std::vec::IntoIter<sqlx::any::AnyRow>,
}

impl<'a> DeferredRowStream<'a> {
    pub(crate) fn new(fut: crate::BoxFuture<'a, Result<Vec<sqlx::any::AnyRow>, Error>>) -> Self {
        Self {
            fut,
            done: false,
            buffered: Vec::new().into_iter(),
        }
    }
}

impl futures_core::Stream for DeferredRowStream<'_> {
    type Item = Result<sqlx::any::AnyRow, Error>;

    fn poll_next(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        use std::task::Poll;
        // Every field is `Unpin` (a boxed future and a vec iterator), so the
        // struct is `Unpin` and this projection needs no `unsafe`.
        let this = self.get_mut();

        if !this.done {
            match this.fut.as_mut().poll(cx) {
                Poll::Pending => return Poll::Pending,
                Poll::Ready(Err(e)) => {
                    this.done = true;
                    return Poll::Ready(Some(Err(e)));
                }
                Poll::Ready(Ok(rows)) => {
                    this.done = true;
                    this.buffered = rows.into_iter();
                }
            }
        }
        Poll::Ready(this.buffered.next().map(Ok))
    }
}

/// Whether an error is a transient serialization/lock failure worth retrying.
///
/// Postgres reports serialization failures as `40001` and deadlocks as `40P01`;
/// SQLite reports contention as `SQLITE_BUSY` / `SQLITE_LOCKED`. Anything else
/// is a real error and must not be retried, because retrying a genuine
/// constraint violation just multiplies the work before failing anyway.
#[must_use]
pub fn is_retryable(err: &Error) -> bool {
    let Error::Sqlx(e) = err else { return false };
    let Some(db) = e.as_database_error() else {
        return false;
    };
    match db.code().as_deref() {
        Some("40001" | "40P01") => true,
        // SQLite surfaces these as extended result codes in the message.
        _ => {
            let m = db.message().to_ascii_lowercase();
            m.contains("database is locked") || m.contains("database table is locked")
        }
    }
}
