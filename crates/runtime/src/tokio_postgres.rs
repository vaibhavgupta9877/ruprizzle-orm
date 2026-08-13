//! Native `tokio-postgres` backend for the ruprizzle runtime.
//!
//! This module is compiled only when the `postgres-tokio-postgres` feature is
//! enabled. It uses `deadpool-postgres` for pooling and `tokio-postgres` for
//! the wire protocol, giving PostgreSQL a direct binary-parameter path that
//! avoids the `sqlx` parameter-type and row-decoding overhead.

#![cfg(feature = "postgres-tokio-postgres")]

use std::borrow::Cow;
use std::fmt;
use std::str::FromStr;

use bytes::BytesMut;
use deadpool_postgres::{Manager, ManagerConfig, Object, Pool, RecyclingMethod, Runtime};
use ruprizzle_core::ir::Provider;
use ruprizzle_dialect::dialect_for;
use tokio_postgres::NoTls;
use tokio_postgres::types::{IsNull, ToSql, Type};

pub use tokio_postgres::Row;

use crate::BoxFuture;
use crate::Error;
use crate::executor::{BoxRowStream, Executor, RowBatch};
use crate::pool::PoolConfig;
use crate::value::Value;

/// A `tokio-postgres`-backed PostgreSQL connection pool.
#[derive(Clone)]
pub struct TokioPostgresPool {
    inner: Pool,
}

impl fmt::Debug for TokioPostgresPool {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TokioPostgresPool")
            .field("status", &self.inner.status())
            .finish_non_exhaustive()
    }
}

impl TokioPostgresPool {
    /// Open a new `tokio-postgres` pool from a PostgreSQL URL and configuration.
    ///
    /// # Errors
    ///
    /// Returns an error if the URL cannot be parsed or the pool cannot be built.
    pub async fn connect(url: &str, config: &PoolConfig) -> Result<Self, Error> {
        let (uses_native, stripped) = strip_driver_param(url);
        if !uses_native {
            // The caller should have already routed based on `driver=tokio-postgres`.
            return Err(Error::Message(
                "expected `driver=tokio-postgres` in URL".into(),
            ));
        }

        let pg_config =
            tokio_postgres::Config::from_str(&stripped).map_err(|e| Error::ConnectionFailure {
                reason: e.to_string(),
            })?;

        // Apply the ruprizzle pool settings to the manager.
        //
        // `Clean` is the only method that scrubs session state, and it is not
        // the default: it measured around 2× the total per-query latency
        // against a local database, and the abandoned-transaction case it would
        // cover is already handled by `TokioPostgresTransaction`'s `Drop`. It is
        // available through `PoolConfig::reset_on_recycle` as defence in depth.
        let mgr_config = ManagerConfig {
            recycling_method: if config.reset_on_recycle {
                RecyclingMethod::Clean
            } else if config.test_before_acquire {
                RecyclingMethod::Verified
            } else {
                RecyclingMethod::Fast
            },
        };

        let mgr = Manager::from_config(pg_config, NoTls, mgr_config);

        let pool = Pool::builder(mgr)
            .max_size(config.max_connections as usize)
            .runtime(Runtime::Tokio1)
            .wait_timeout(Some(config.acquire_timeout))
            .create_timeout(Some(config.acquire_timeout))
            .recycle_timeout(Some(config.acquire_timeout))
            .build()
            .map_err(|e| Error::ConnectionFailure {
                reason: e.to_string(),
            })?;

        // Fill the pool to the requested minimum so the first query is not
        // penalised by cold-start connections.
        for _ in 0..config.min_connections {
            let _ = pool.get().await.map_err(tokio_postgres_pool_error)?;
        }

        Ok(Self { inner: pool })
    }

    /// Total connections currently held by the pool.
    #[must_use]
    pub fn size(&self) -> u32 {
        self.inner.status().size as u32
    }

    /// Connections immediately available for checkout.
    #[must_use]
    pub fn num_idle(&self) -> usize {
        self.inner.status().available
    }

    /// Fetch all rows from `sql` with `binds`.
    pub(crate) async fn fetch_all(&self, sql: &str, binds: &[Value]) -> Result<RowBatch, Error> {
        let client = self.inner.get().await.map_err(tokio_postgres_pool_error)?;
        let params = bind_params(binds)?;
        let rows = client.query(sql, &params).await.map_err(Error::from)?;
        Ok(RowBatch::PostgresNative(rows))
    }

    /// Execute `sql` with `binds`, returning the number of affected rows.
    pub(crate) async fn execute(&self, sql: &str, binds: &[Value]) -> Result<u64, Error> {
        let client = self.inner.get().await.map_err(tokio_postgres_pool_error)?;
        let params = bind_params(binds)?;
        client.execute(sql, &params).await.map_err(Error::from)
    }

    /// Begin a new transaction on this pool.
    pub(crate) async fn begin(&self) -> Result<TokioPostgresTransaction, Error> {
        let client = self.inner.get().await.map_err(tokio_postgres_pool_error)?;
        client.execute("BEGIN", &[]).await.map_err(Error::from)?;
        Ok(TokioPostgresTransaction {
            client: Some(client),
            // Captured here rather than in `Drop`, which may run on a thread
            // with no runtime in context (a `block_on` teardown, say). `begin`
            // is `async`, so there is always one at this point.
            runtime: tokio::runtime::Handle::try_current().ok(),
        })
    }
}

impl Executor for TokioPostgresPool {
    fn dialect(&self) -> Box<dyn ruprizzle_dialect::DbDialect> {
        dialect_for(Provider::Postgres)
    }

    fn fetch_all_raw(
        &self,
        sql: Cow<'static, str>,
        binds: Vec<Value>,
    ) -> BoxFuture<'_, Result<RowBatch, Error>> {
        let pool = self.clone();
        Box::pin(async move { pool.fetch_all(sql.as_ref(), &binds).await })
    }

    fn execute_raw(
        &self,
        sql: Cow<'static, str>,
        binds: Vec<Value>,
    ) -> BoxFuture<'_, Result<u64, Error>> {
        let pool = self.clone();
        Box::pin(async move { pool.execute(sql.as_ref(), &binds).await })
    }

    fn stream_raw(&self, sql: Cow<'static, str>, binds: Vec<Value>) -> BoxRowStream<'_> {
        Box::pin(crate::executor::DeferredRowStream::new(Box::pin(
            async move { self.fetch_all_raw(sql, binds).await },
        )))
    }
}

/// A `tokio-postgres` transaction that owns its pooled connection.
///
/// **This type must never be `Clone`.** It owns its `Object` exclusively; two
/// handles to one pooled connection would let two callers issue statements
/// inside the same `BEGIN` while each believed it held the connection alone.
/// See BUG-06, which is that hazard on the `rusqlite` side.
pub(crate) struct TokioPostgresTransaction {
    /// `None` once the transaction has been committed or rolled back, which is
    /// how [`Drop`] tells a finished transaction from an abandoned one.
    client: Option<Object>,
    /// The runtime to spawn the rollback on when the transaction is abandoned.
    runtime: Option<tokio::runtime::Handle>,
}

impl fmt::Debug for TokioPostgresTransaction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TokioPostgresTransaction")
            .finish_non_exhaustive()
    }
}

impl TokioPostgresTransaction {
    /// The connection this transaction owns, or an error if it has finished.
    fn client(&self) -> Result<&Object, Error> {
        self.client
            .as_ref()
            .ok_or_else(|| Error::Message("transaction already finished".into()))
    }

    /// Commit the transaction and return the connection to the pool.
    pub(crate) async fn commit(mut self) -> Result<(), Error> {
        self.finish("COMMIT").await
    }

    /// Roll the transaction back and return the connection to the pool.
    pub(crate) async fn rollback(mut self) -> Result<(), Error> {
        self.finish("ROLLBACK").await
    }

    /// Run `stmt` (`COMMIT` or `ROLLBACK`) and release the connection.
    ///
    /// Taking the `Object` out first is what stops [`Drop`] from spawning a
    /// second, redundant `ROLLBACK` after an explicit finish.
    async fn finish(&mut self, stmt: &'static str) -> Result<(), Error> {
        let Some(client) = self.client.take() else {
            return Err(Error::Message("transaction already finished".into()));
        };

        let result = client.execute(stmt, &[]).await.map_err(Error::from);

        if result.is_err() && stmt != "ROLLBACK" {
            // A failed COMMIT leaves the transaction open (or aborted). The
            // connection is about to go back into rotation, so end it.
            let _ = client.execute("ROLLBACK", &[]).await;
        }

        result.map(|_| ())
    }

    /// Execute `sql` with `binds` inside the transaction.
    pub(crate) async fn execute(&self, sql: &str, binds: &[Value]) -> Result<u64, Error> {
        let params = bind_params(binds)?;
        self.client()?
            .execute(sql, &params)
            .await
            .map_err(Error::from)
    }

    /// Fetch all rows from `sql` with `binds` inside the transaction.
    pub(crate) async fn fetch_all(&self, sql: &str, binds: &[Value]) -> Result<RowBatch, Error> {
        let params = bind_params(binds)?;
        let rows = self
            .client()?
            .query(sql, &params)
            .await
            .map_err(Error::from)?;
        Ok(RowBatch::PostgresNative(rows))
    }
}

/// Roll back before releasing the connection when a transaction is abandoned.
///
/// Without this, dropping a transaction released the `Object` straight back to
/// `deadpool` with its `BEGIN` still open, and the next request to receive that
/// connection ran inside the previous request's transaction (BUG-03). Unlike
/// the `rusqlite` leak this was silent: the connection was never lost, only
/// dirty.
///
/// `ROLLBACK` is async and `Drop` is not, so the rollback is spawned — and the
/// `Object` moves *into* the spawned task. That ordering is the whole point:
/// `deadpool` reclaims the connection when the `Object` drops, so it must not
/// drop until the rollback has resolved.
impl Drop for TokioPostgresTransaction {
    fn drop(&mut self) {
        let Some(client) = self.client.take() else {
            return;
        };

        tracing::warn!(
            target: "ruprizzle::query",
            "transaction dropped without commit or rollback; rolling back"
        );

        let Some(runtime) = self.runtime.take() else {
            // No runtime to roll back on. Detaching the connection loses it
            // from the pool, which is the lesser evil: returning it would put
            // the next request inside this transaction.
            tracing::warn!(
                target: "ruprizzle::query",
                "no tokio runtime available to roll back an abandoned transaction; \
                 discarding the connection"
            );
            let _ = Object::take(client);
            return;
        };

        runtime.spawn(async move {
            if let Err(error) = client.execute("ROLLBACK", &[]).await {
                tracing::warn!(
                    target: "ruprizzle::query",
                    error = %error,
                    "failed to roll back an abandoned transaction"
                );
            }
            // `client` drops here, returning the connection to `deadpool` only
            // now that the rollback has completed.
            drop(client);
        });
    }
}

/// Convert a vector of [`Value`]s into a slice of `&(dyn ToSql + Sync)`.
///
/// [`Value`] implements `ToSql`, so this only needs to collect the references.
fn bind_params(binds: &[Value]) -> Result<Vec<&(dyn ToSql + Sync)>, Error> {
    let mut params = Vec::with_capacity(binds.len());
    for b in binds {
        params.push(b as &(dyn ToSql + Sync));
    }
    Ok(params)
}

/// A type that can be decoded from a `tokio_postgres::Row`.
pub trait FromTokioPostgresRow: Sized + Send + Sync + 'static {
    /// Decode `self` from an ordered row of PostgreSQL values.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Message`] if the row cannot be decoded.
    fn from_tokio_postgres_row(row: &Row) -> Result<Self, Error>;
}

macro_rules! tuple_from_tokio_postgres_row {
    ($($T:ident $idx:tt),+) => {
        impl<$($T: tokio_postgres::types::FromSqlOwned + Send + Sync + 'static),+> FromTokioPostgresRow for ($($T,)+) {
            fn from_tokio_postgres_row(row: &Row) -> Result<Self, Error> {
                Ok((
                    $(
                        row.try_get::<usize, $T>($idx)?
                    ,)+
                ))
            }
        }
    };
}

tuple_from_tokio_postgres_row! { A 0 }
tuple_from_tokio_postgres_row! { A 0, B 1 }
tuple_from_tokio_postgres_row! { A 0, B 1, C 2 }
tuple_from_tokio_postgres_row! { A 0, B 1, C 2, D 3 }
tuple_from_tokio_postgres_row! { A 0, B 1, C 2, D 3, E 4 }
tuple_from_tokio_postgres_row! { A 0, B 1, C 2, D 3, E 4, F 5 }
tuple_from_tokio_postgres_row! { A 0, B 1, C 2, D 3, E 4, F 5, G 6 }
tuple_from_tokio_postgres_row! { A 0, B 1, C 2, D 3, E 4, F 5, G 6, H 7 }

/// Convenience macro for tests and examples: implement `FromTokioPostgresRow`
/// by returning `Default::default()`.
///
/// This is only useful when the type is never actually decoded from a
/// `tokio-postgres` row (e.g. SQLite-only tests).
#[macro_export]
#[cfg(feature = "postgres-tokio-postgres")]
macro_rules! tokio_postgres_default_row {
    ($t:ty) => {
        impl $crate::tokio_postgres::FromTokioPostgresRow for $t {
            fn from_tokio_postgres_row(
                _: &$crate::tokio_postgres::Row,
            ) -> Result<Self, $crate::Error> {
                Ok(<$t>::default())
            }
        }
    };
}

impl TokioPostgresPool {
    /// Close the pool.
    pub(crate) async fn close(&self) {
        self.inner.close();
    }
}

/// Implement `ToSql` for [`Value`] so the generic query pipeline can bind
/// values without per-call allocation.
impl ToSql for Value {
    fn to_sql(
        &self,
        ty: &Type,
        out: &mut BytesMut,
    ) -> Result<IsNull, Box<dyn std::error::Error + Sync + Send>> {
        // Always go through `to_sql_checked` so each variant's real `ToSql`
        // implementation has a chance to verify the Postgres `Type`.
        self.to_sql_checked(ty, out)
    }

    fn accepts(_ty: &Type) -> bool
    where
        Self: Sized,
    {
        // `Value` is a runtime-discriminated union. The real compatibility check
        // happens inside `to_sql_checked` by delegating to the variant's real
        // `ToSql` implementation. Returning `true` here lets the driver send
        // `NULL` values to any column type without forcing a TEXT cast.
        true
    }

    fn to_sql_checked(
        &self,
        ty: &Type,
        out: &mut BytesMut,
    ) -> Result<IsNull, Box<dyn std::error::Error + Sync + Send>> {
        match self {
            Value::Null => Ok(IsNull::Yes),
            Value::Bool(b) => <bool as ToSql>::to_sql_checked(b, ty, out),
            Value::I32(i) => <i32 as ToSql>::to_sql_checked(i, ty, out),
            Value::I64(i) => <i64 as ToSql>::to_sql_checked(i, ty, out),
            Value::F64(f) => <f64 as ToSql>::to_sql_checked(f, ty, out),
            Value::Decimal(d) => <crate::types::Decimal as ToSql>::to_sql_checked(d, ty, out),
            Value::Str(s) => {
                let s_ref: &str = s.as_ref();
                <&str as ToSql>::to_sql_checked(&s_ref, ty, out)
            }
            Value::Uuid(u) => <crate::types::Uuid as ToSql>::to_sql_checked(u, ty, out),
            Value::DateTime(dt) => {
                <chrono::DateTime<chrono::Utc> as ToSql>::to_sql_checked(dt, ty, out)
            }
            Value::Date(d) => <chrono::NaiveDate as ToSql>::to_sql_checked(d, ty, out),
            Value::Time(t) => <chrono::NaiveTime as ToSql>::to_sql_checked(t, ty, out),
            Value::Json(v) => <serde_json::Value as ToSql>::to_sql_checked(v, ty, out),
            Value::Bytes(b) => {
                let bytes: &[u8] = b.as_ref();
                <&[u8] as ToSql>::to_sql_checked(&bytes, ty, out)
            }
            Value::Array(_) => Err("array bind values are not supported yet".into()),
        }
    }
}

/// Strips the `driver=tokio-postgres` routing parameter from a PostgreSQL URL
/// so the remainder can be parsed by `tokio_postgres::Config`.
fn strip_driver_param(url: &str) -> (bool, String) {
    let Some((base, query)) = url.split_once('?') else {
        return (false, url.to_owned());
    };

    let mut parts = Vec::new();
    let mut uses_native = false;
    for part in query.split('&') {
        if part == "driver=tokio-postgres" || part.starts_with("driver=tokio-postgres&") {
            uses_native = true;
            continue;
        }
        parts.push(part);
    }

    let stripped = if parts.is_empty() {
        base.to_owned()
    } else {
        format!("{base}?{}", parts.join("&"))
    };

    (uses_native, stripped)
}

fn tokio_postgres_pool_error(e: deadpool_postgres::PoolError) -> Error {
    match e {
        deadpool_postgres::PoolError::Backend(e) => Error::from(e),
        _ => Error::ConnectionFailure {
            reason: e.to_string(),
        },
    }
}
