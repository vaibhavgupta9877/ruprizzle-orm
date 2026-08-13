//! Transaction handle.

use std::borrow::Cow;
use std::fmt;

use tokio::sync::Mutex;

use ruprizzle_core::ir::Provider;
use ruprizzle_dialect::{DbDialect, dialect_for};
use sqlx::{Any, Postgres, Sqlite};

use crate::BoxFuture;
use crate::Error;
use crate::executor::RowBatch;
use crate::model::RowDecode;
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

#[derive(Debug)]
enum TxInner {
    Any(sqlx::Transaction<'static, Any>),
    Postgres(sqlx::Transaction<'static, Postgres>),
    Sqlite(sqlx::Transaction<'static, Sqlite>),
    #[cfg(feature = "sqlite-rusqlite")]
    SqliteNative(crate::rusqlite::RusqliteTransaction),
    #[cfg(feature = "postgres-tokio-postgres")]
    PostgresNative(crate::tokio_postgres::TokioPostgresTransaction),
}

/// A transaction in progress.
pub struct Tx {
    inner: Mutex<Option<TxInner>>,
    provider: Provider,
    dialect: &'static dyn DbDialect,
}

impl fmt::Debug for Tx {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Tx")
            .field("provider", &self.provider)
            .field("dialect", &self.dialect.name())
            .finish_non_exhaustive()
    }
}

impl Tx {
    /// Begins a new transaction on `pool`.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Sqlx`] if the database cannot begin a transaction.
    pub async fn begin(pool: &Pool) -> Result<Self, Error> {
        let tx = match pool {
            Pool::Any(p) => TxInner::Any(p.begin().await.map_err(Error::Sqlx)?),
            Pool::Postgres(p) => TxInner::Postgres(p.begin().await.map_err(Error::Sqlx)?),
            Pool::Sqlite(p) => TxInner::Sqlite(p.begin().await.map_err(Error::Sqlx)?),
            #[cfg(feature = "sqlite-rusqlite")]
            Pool::SqliteNative(p) => TxInner::SqliteNative(p.begin_transaction().await?),
            #[cfg(feature = "postgres-tokio-postgres")]
            Pool::PostgresNative(p) => TxInner::PostgresNative(p.begin().await?),
        };
        let provider = pool.provider();
        Ok(Self {
            inner: Mutex::new(Some(tx)),
            provider,
            dialect: dialect_for(provider),
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
                &[],
            )
            .await?;
        }
        Ok(this)
    }

    /// The dialect for the backend this transaction runs on.
    #[must_use]
    pub fn dialect(&self) -> &dyn DbDialect {
        self.dialect
    }

    /// Commits the transaction.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Sqlx`] if the commit fails.
    pub async fn commit(self) -> Result<(), Error> {
        let tx = self.inner.lock().await.take();
        if let Some(tx) = tx {
            match tx {
                TxInner::Any(tx) => tx.commit().await.map_err(Error::Sqlx)?,
                TxInner::Postgres(tx) => tx.commit().await.map_err(Error::Sqlx)?,
                TxInner::Sqlite(tx) => tx.commit().await.map_err(Error::Sqlx)?,
                #[cfg(feature = "sqlite-rusqlite")]
                TxInner::SqliteNative(tx) => {
                    tx.commit()?;
                }
                #[cfg(feature = "postgres-tokio-postgres")]
                TxInner::PostgresNative(tx) => {
                    tx.commit().await?;
                }
            };
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
            match tx {
                TxInner::Any(tx) => tx.rollback().await.map_err(Error::Sqlx)?,
                TxInner::Postgres(tx) => tx.rollback().await.map_err(Error::Sqlx)?,
                TxInner::Sqlite(tx) => tx.rollback().await.map_err(Error::Sqlx)?,
                #[cfg(feature = "sqlite-rusqlite")]
                TxInner::SqliteNative(tx) => {
                    tx.rollback()?;
                }
                #[cfg(feature = "postgres-tokio-postgres")]
                TxInner::PostgresNative(tx) => {
                    tx.rollback().await?;
                }
            };
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
    pub async fn execute(&self, sql: &str, binds: &[Value]) -> Result<u64, Error> {
        let mut guard = self.inner.lock().await;
        let tx = guard
            .as_mut()
            .ok_or_else(|| Error::Message("transaction already finished".into()))?;

        match tx {
            TxInner::Any(tx) => {
                let mut q = sqlx::query::<Any>(sql);
                for b in binds {
                    q = q.bind(b);
                }
                q.execute(&mut **tx)
                    .await
                    .map(|r| r.rows_affected())
                    .map_err(Error::Sqlx)
            }
            TxInner::Postgres(tx) => {
                let mut q = sqlx::query::<Postgres>(sql);
                for b in binds {
                    q = q.bind(b);
                }
                q.execute(&mut **tx)
                    .await
                    .map(|r| r.rows_affected())
                    .map_err(Error::Sqlx)
            }
            TxInner::Sqlite(tx) => {
                let mut q = sqlx::query::<Sqlite>(sql);
                for b in binds {
                    q = q.bind(b);
                }
                q.execute(&mut **tx)
                    .await
                    .map(|r| r.rows_affected())
                    .map_err(Error::Sqlx)
            }
            #[cfg(feature = "sqlite-rusqlite")]
            TxInner::SqliteNative(tx) => tx.execute_sync(sql, binds),
            #[cfg(feature = "postgres-tokio-postgres")]
            TxInner::PostgresNative(tx) => tx.execute(sql, binds).await,
        }
    }

    /// Fetches all rows from a raw statement inside the transaction.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Sqlx`] for database errors or [`Error::Message`] if the
    /// transaction has already been finished.
    pub async fn fetch_all<T>(&self, sql: &str, binds: &[Value]) -> Result<Vec<T>, Error>
    where
        T: Send + Unpin + RowDecode,
    {
        let mut guard = self.inner.lock().await;
        let tx = guard
            .as_mut()
            .ok_or_else(|| Error::Message("transaction already finished".into()))?;

        match tx {
            TxInner::Any(tx) => {
                let mut q = sqlx::query_as::<Any, T>(sql);
                for b in binds {
                    q = q.bind(b);
                }
                q.fetch_all(&mut **tx).await.map_err(Error::Sqlx)
            }
            TxInner::Postgres(tx) => {
                let mut q = sqlx::query_as::<Postgres, T>(sql);
                for b in binds {
                    q = q.bind(b);
                }
                q.fetch_all(&mut **tx).await.map_err(Error::Sqlx)
            }
            TxInner::Sqlite(tx) => {
                let mut q = sqlx::query_as::<Sqlite, T>(sql);
                for b in binds {
                    q = q.bind(b);
                }
                q.fetch_all(&mut **tx).await.map_err(Error::Sqlx)
            }
            #[cfg(feature = "sqlite-rusqlite")]
            TxInner::SqliteNative(tx) => {
                let batch = tx.fetch_all_sync(sql, binds)?;
                crate::executor::decode_rows::<T>(batch)
            }
            #[cfg(feature = "postgres-tokio-postgres")]
            TxInner::PostgresNative(tx) => {
                let batch = tx.fetch_all(sql, binds).await?;
                crate::executor::decode_rows::<T>(batch)
            }
        }
    }

    /// Fetches one row from a raw statement inside the transaction.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Sqlx`] for database errors or [`Error::Message`] if the
    /// transaction has already been finished.
    pub async fn fetch_one<T>(&self, sql: &str, binds: &[Value]) -> Result<T, Error>
    where
        T: Send + Unpin + RowDecode,
    {
        let mut guard = self.inner.lock().await;
        let tx = guard
            .as_mut()
            .ok_or_else(|| Error::Message("transaction already finished".into()))?;

        match tx {
            TxInner::Any(tx) => {
                let mut q = sqlx::query_as::<Any, T>(sql);
                for b in binds {
                    q = q.bind(b);
                }
                q.fetch_one(&mut **tx).await.map_err(Error::Sqlx)
            }
            TxInner::Postgres(tx) => {
                let mut q = sqlx::query_as::<Postgres, T>(sql);
                for b in binds {
                    q = q.bind(b);
                }
                q.fetch_one(&mut **tx).await.map_err(Error::Sqlx)
            }
            TxInner::Sqlite(tx) => {
                let mut q = sqlx::query_as::<Sqlite, T>(sql);
                for b in binds {
                    q = q.bind(b);
                }
                q.fetch_one(&mut **tx).await.map_err(Error::Sqlx)
            }
            #[cfg(feature = "sqlite-rusqlite")]
            TxInner::SqliteNative(tx) => {
                let batch = tx.fetch_all_sync(sql, binds)?;
                let rows = crate::executor::decode_rows::<T>(batch)?;
                rows.into_iter()
                    .next()
                    .ok_or_else(|| Error::Message("no row found".into()))
            }
            #[cfg(feature = "postgres-tokio-postgres")]
            TxInner::PostgresNative(tx) => {
                let batch = tx.fetch_all(sql, binds).await?;
                let rows = crate::executor::decode_rows::<T>(batch)?;
                rows.into_iter()
                    .next()
                    .ok_or_else(|| Error::Message("no row found".into()))
            }
        }
    }

    /// Fetches one row from a raw statement inside the transaction.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Sqlx`] for database errors or [`Error::Message`] if the
    /// transaction has already been finished.
    pub async fn fetch_optional<T>(&self, sql: &str, binds: &[Value]) -> Result<Option<T>, Error>
    where
        T: Send + Unpin + RowDecode,
    {
        let mut guard = self.inner.lock().await;
        let tx = guard
            .as_mut()
            .ok_or_else(|| Error::Message("transaction already finished".into()))?;

        match tx {
            TxInner::Any(tx) => {
                let mut q = sqlx::query_as::<Any, T>(sql);
                for b in binds {
                    q = q.bind(b);
                }
                q.fetch_optional(&mut **tx).await.map_err(Error::Sqlx)
            }
            TxInner::Postgres(tx) => {
                let mut q = sqlx::query_as::<Postgres, T>(sql);
                for b in binds {
                    q = q.bind(b);
                }
                q.fetch_optional(&mut **tx).await.map_err(Error::Sqlx)
            }
            TxInner::Sqlite(tx) => {
                let mut q = sqlx::query_as::<Sqlite, T>(sql);
                for b in binds {
                    q = q.bind(b);
                }
                q.fetch_optional(&mut **tx).await.map_err(Error::Sqlx)
            }
            #[cfg(feature = "sqlite-rusqlite")]
            TxInner::SqliteNative(tx) => {
                let batch = tx.fetch_all_sync(sql, binds)?;
                let rows = crate::executor::decode_rows::<T>(batch)?;
                Ok(rows.into_iter().next())
            }
            #[cfg(feature = "postgres-tokio-postgres")]
            TxInner::PostgresNative(tx) => {
                let batch = tx.fetch_all(sql, binds).await?;
                let rows = crate::executor::decode_rows::<T>(batch)?;
                Ok(rows.into_iter().next())
            }
        }
    }

    /// Fetches raw rows inside the transaction.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Sqlx`] for database errors or [`Error::Message`] if the
    /// transaction has already been finished.
    pub(crate) async fn fetch_all_rows(
        &self,
        sql: &str,
        binds: &[Value],
    ) -> Result<RowBatch, Error> {
        let mut guard = self.inner.lock().await;
        let tx = guard
            .as_mut()
            .ok_or_else(|| Error::Message("transaction already finished".into()))?;

        match tx {
            TxInner::Any(tx) => {
                let mut q = sqlx::query::<Any>(sql);
                for b in binds {
                    q = q.bind(b);
                }
                q.fetch_all(&mut **tx)
                    .await
                    .map(RowBatch::Any)
                    .map_err(Error::Sqlx)
            }
            TxInner::Postgres(tx) => {
                let mut q = sqlx::query::<Postgres>(sql);
                for b in binds {
                    q = q.bind(b);
                }
                q.fetch_all(&mut **tx)
                    .await
                    .map(RowBatch::Postgres)
                    .map_err(Error::Sqlx)
            }
            TxInner::Sqlite(tx) => {
                let mut q = sqlx::query::<Sqlite>(sql);
                for b in binds {
                    q = q.bind(b);
                }
                q.fetch_all(&mut **tx)
                    .await
                    .map(RowBatch::Sqlite)
                    .map_err(Error::Sqlx)
            }
            #[cfg(feature = "sqlite-rusqlite")]
            TxInner::SqliteNative(tx) => tx.fetch_all_sync(sql, binds),
            #[cfg(feature = "postgres-tokio-postgres")]
            TxInner::PostgresNative(tx) => tx.fetch_all(sql, binds).await,
        }
    }
}

impl crate::executor::Executor for Tx {
    fn dialect(&self) -> &dyn DbDialect {
        Self::dialect(self)
    }

    #[cfg(feature = "sqlite-rusqlite")]
    fn as_rusqlite(&self) -> Option<&crate::rusqlite::RusqlitePool> {
        None
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
                let result = self.fetch_all_rows(sql.as_ref(), &binds).await;
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
                self.fetch_all_rows(sql.as_ref(), &binds).await
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
                let result = self.execute(sql.as_ref(), &binds).await;
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
                self.execute(sql.as_ref(), &binds).await
            }
        })
    }

    /// Buffers rather than streaming incrementally.
    ///
    /// A transaction owns a single connection behind a mutex. Holding a cursor
    /// open across awaits would keep that mutex locked and deadlock every other
    /// statement issued on the same transaction, so the rows are fetched up
    /// front. Streaming a very large result set is therefore something to do on
    /// the pool, not inside a transaction.
    fn stream_raw(
        &self,
        sql: Cow<'static, str>,
        binds: Vec<Value>,
    ) -> crate::executor::BoxRowStream<'_> {
        Box::pin(crate::executor::DeferredRowStream::new(Box::pin(
            async move { crate::executor::Executor::fetch_all_raw(self, sql, binds).await },
        )))
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
    let (code, message) = match err {
        Error::Sqlx(e) => {
            let Some(db) = e.as_database_error() else {
                return false;
            };
            (
                db.code().map(|c| c.to_string()),
                Some(db.message().to_string()),
            )
        }
        #[cfg(feature = "postgres-tokio-postgres")]
        Error::TokioPostgres(e) => {
            let Some(db) = e.as_db_error() else {
                return false;
            };
            (
                Some(db.code().code().to_owned()),
                Some(db.message().to_owned()),
            )
        }
        _ => return false,
    };

    match code.as_deref() {
        Some("40001" | "40P01") => true,
        _ => {
            let m = message.unwrap_or_default().to_ascii_lowercase();
            m.contains("database is locked") || m.contains("database table is locked")
        }
    }
}
