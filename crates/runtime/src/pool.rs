//! Connection pool construction and configuration.

use std::time::Duration;

use futures_core::future::BoxFuture;
use futures_core::stream::BoxStream;
use ruprizzle_core::ir::Provider;
use sqlx::any::AnyPoolOptions;

/// An ORM pool that may wrap a native `sqlx` pool or the generic `Any` driver.
///
/// For now the public `connect`/`connect_with` builders always return
/// `Pool::Any`. Native variants will become reachable once per-backend
/// `FromRow` codegen lands (P2-2).
#[derive(Clone, Debug)]
pub enum Pool {
    /// Generic `sqlx::Any` pool, chosen by URL scheme.
    Any(sqlx::Pool<sqlx::Any>),
    /// Native Postgres pool.
    Postgres(sqlx::Pool<sqlx::Postgres>),
    /// Native SQLite pool.
    Sqlite(sqlx::Pool<sqlx::Sqlite>),
}

impl Pool {
    /// Returns the dialect provider for the backend behind this pool.
    #[must_use]
    pub fn provider(&self) -> Provider {
        match self {
            Pool::Any(any) => {
                let opts = any.connect_options();
                let scheme = opts.database_url.scheme();
                Provider::parse(scheme).unwrap_or(Provider::Postgres)
            }
            Pool::Postgres(_) => Provider::Postgres,
            Pool::Sqlite(_) => Provider::Sqlite,
        }
    }

    /// Begins a new transaction on this pool.
    ///
    /// # Errors
    ///
    /// Returns [`crate::Error::Sqlx`] if the database cannot begin a transaction.
    pub async fn begin(&self) -> Result<crate::tx::Tx, crate::Error> {
        crate::tx::Tx::begin(self).await
    }

    /// Total connections currently held by the pool.
    #[must_use]
    pub fn size(&self) -> u32 {
        match self {
            Pool::Any(p) => p.size(),
            Pool::Postgres(p) => p.size(),
            Pool::Sqlite(p) => p.size(),
        }
    }

    /// Connections immediately available for checkout.
    #[must_use]
    pub fn num_idle(&self) -> usize {
        match self {
            Pool::Any(p) => p.num_idle(),
            Pool::Postgres(p) => p.num_idle(),
            Pool::Sqlite(p) => p.num_idle(),
        }
    }

    /// Returns the pool's connection options.
    ///
    /// This is exposed for tests that verify `PoolConfig` propagation. It only
    /// makes sense for the `Any` backend until native pool construction is
    /// wired up.
    #[must_use]
    pub fn options(&self) -> &sqlx::pool::PoolOptions<sqlx::Any> {
        match self {
            Pool::Any(any) => any.options(),
            _ => unimplemented!("options() only implemented for the Any backend"),
        }
    }

    /// Acquires a connection from the pool.
    pub async fn acquire(&self) -> Result<sqlx::pool::PoolConnection<sqlx::Any>, crate::Error> {
        match self {
            Pool::Any(any) => any.acquire().await.map_err(crate::Error::Sqlx),
            _ => unimplemented!("acquire() only implemented for the Any backend"),
        }
    }

    /// Borrows the wrapped `Any` pool.
    ///
    /// This is a compatibility helper for tests and benchmarks that still want
    /// to use raw `sqlx` against the `Any` backend. It will be removed once the
    /// native-backend path is complete.
    #[must_use]
    pub fn as_any(&self) -> &sqlx::Pool<sqlx::Any> {
        match self {
            Pool::Any(any) => any,
            _ => unimplemented!("as_any() is only valid for the Any backend"),
        }
    }
}

impl<'c> sqlx::Executor<'c> for &'c Pool {
    type Database = sqlx::Any;

    fn fetch_many<'e, 'q: 'e, E>(
        self,
        query: E,
    ) -> BoxStream<
        'e,
        Result<
            sqlx::Either<
                <Self::Database as sqlx::Database>::QueryResult,
                <Self::Database as sqlx::Database>::Row,
            >,
            sqlx::Error,
        >,
    >
    where
        'c: 'e,
        E: 'q + sqlx::Execute<'q, Self::Database>,
    {
        match self {
            Pool::Any(any) => sqlx::Executor::fetch_many(any, query),
            _ => unimplemented!("native backend queries need per-backend FromRow (P2-2)"),
        }
    }

    fn fetch_optional<'e, 'q: 'e, E>(
        self,
        query: E,
    ) -> BoxFuture<'e, Result<Option<<Self::Database as sqlx::Database>::Row>, sqlx::Error>>
    where
        'c: 'e,
        E: 'q + sqlx::Execute<'q, Self::Database>,
    {
        match self {
            Pool::Any(any) => sqlx::Executor::fetch_optional(any, query),
            _ => unimplemented!("native backend queries need per-backend FromRow (P2-2)"),
        }
    }

    fn prepare_with<'e, 'q: 'e>(
        self,
        sql: &'q str,
        parameters: &'e [<Self::Database as sqlx::Database>::TypeInfo],
    ) -> BoxFuture<'e, Result<<Self::Database as sqlx::Database>::Statement<'q>, sqlx::Error>>
    where
        'c: 'e,
    {
        match self {
            Pool::Any(any) => sqlx::Executor::prepare_with(any, sql, parameters),
            _ => unimplemented!("native backend queries need per-backend FromRow (P2-2)"),
        }
    }

    fn describe<'e, 'q: 'e>(
        self,
        sql: &'q str,
    ) -> BoxFuture<'e, Result<sqlx::Describe<Self::Database>, sqlx::Error>>
    where
        'c: 'e,
    {
        match self {
            Pool::Any(any) => sqlx::Executor::describe(any, sql),
            _ => unimplemented!("native backend queries need per-backend FromRow (P2-2)"),
        }
    }
}

/// Configuration used to build a [`Pool`].
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct PoolConfig {
    /// Maximum connections held open by the pool.
    pub max_connections: u32,
    /// Connections kept warm while idle.
    pub min_connections: u32,
    /// Maximum time spent waiting to acquire a connection.
    pub acquire_timeout: Duration,
    /// Maximum idle connection duration; `None` keeps idle connections forever.
    pub idle_timeout: Option<Duration>,
    /// Maximum connection lifetime; `None` disables recycling by age.
    pub max_lifetime: Option<Duration>,
    /// Whether to test a connection before handing it out.
    pub test_before_acquire: bool,
}

impl Default for PoolConfig {
    fn default() -> Self {
        Self {
            max_connections: 10,
            min_connections: 0,
            acquire_timeout: Duration::from_secs(30),
            idle_timeout: Some(Duration::from_secs(600)),
            max_lifetime: Some(Duration::from_secs(1800)),
            test_before_acquire: true,
        }
    }
}

/// Connects using sqlx-compatible default pool settings.
///
/// The URL scheme selects the driver (`postgres://`, `sqlite://`, etc.).
///
/// # Errors
///
/// Returns an error if the URL cannot be parsed or the connection fails.
pub async fn connect(url: &str) -> Result<Pool, crate::Error> {
    connect_with(url, &PoolConfig::default()).await
}

/// Connects using explicit pool settings.
///
/// The URL scheme selects the driver (`postgres://`, `sqlite://`, etc.).
///
/// # Errors
///
/// Returns an error if the URL cannot be parsed or the connection fails.
pub async fn connect_with(url: &str, config: &PoolConfig) -> Result<Pool, crate::Error> {
    sqlx::any::install_default_drivers();
    let pool = AnyPoolOptions::new()
        .max_connections(config.max_connections)
        .min_connections(config.min_connections)
        .acquire_timeout(config.acquire_timeout)
        .idle_timeout(config.idle_timeout)
        .max_lifetime(config.max_lifetime)
        .test_before_acquire(config.test_before_acquire)
        .connect(url)
        .await
        .map_err(crate::Error::Sqlx)?;
    Ok(Pool::Any(pool))
}

/// Point-in-time pool saturation data for metrics endpoints.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct PoolStats {
    /// Total connections currently held by the pool.
    pub size: u32,
    /// Connections immediately available for checkout.
    pub idle: usize,
    /// Connections currently checked out.
    pub in_use: usize,
}

/// Samples the current pool saturation.
#[must_use]
pub fn stats(pool: &Pool) -> PoolStats {
    let size = pool.size();
    let idle = pool.num_idle();
    PoolStats {
        size,
        idle,
        in_use: (size as usize).saturating_sub(idle),
    }
}

/// Checks database reachability for readiness probes.
///
/// # Errors
///
/// Returns an error if a connection cannot be acquired or `SELECT 1` fails.
pub async fn ping(pool: &Pool) -> Result<(), crate::Error> {
    crate::executor::Executor::execute_raw(pool, "SELECT 1".to_string(), Vec::new())
        .await
        .map(|_| ())
}
