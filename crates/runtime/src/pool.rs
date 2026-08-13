//! Connection pool construction and configuration.

use std::str::FromStr;
use std::time::Duration;

use futures_core::future::BoxFuture;
use futures_core::stream::BoxStream;
use ruprizzle_core::ir::Provider;
use sqlx::any::AnyPoolOptions;
use sqlx::sqlite::SqlitePoolOptions;

/// An ORM pool that may wrap a native `sqlx` pool or the generic `Any` driver.
///
/// `connect`/`connect_with` return native `Pool::Postgres` or `Pool::Sqlite`
/// pools for their respective URL schemes, and fall back to `Pool::Any` for
/// other schemes.
#[derive(Clone, Debug)]
pub enum Pool {
    /// Generic `sqlx::Any` pool, chosen by URL scheme.
    Any(sqlx::Pool<sqlx::Any>),
    /// Native Postgres pool using `sqlx`.
    Postgres(sqlx::Pool<sqlx::Postgres>),
    /// Native SQLite pool.
    Sqlite(sqlx::Pool<sqlx::Sqlite>),
    /// Native `rusqlite`-backed SQLite pool.
    #[cfg(feature = "sqlite-rusqlite")]
    SqliteNative(crate::rusqlite::RusqlitePool),
    /// Native `tokio-postgres`-backed Postgres pool.
    #[cfg(feature = "postgres-tokio-postgres")]
    PostgresNative(crate::tokio_postgres::TokioPostgresPool),
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
            #[cfg(feature = "postgres-tokio-postgres")]
            Pool::PostgresNative(_) => Provider::Postgres,
            Pool::Sqlite(_) => Provider::Sqlite,
            #[cfg(feature = "sqlite-rusqlite")]
            Pool::SqliteNative(_) => Provider::Sqlite,
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
            #[cfg(feature = "sqlite-rusqlite")]
            Pool::SqliteNative(_) => 0,
            #[cfg(feature = "postgres-tokio-postgres")]
            Pool::PostgresNative(p) => p.size(),
        }
    }

    /// Connections immediately available for checkout.
    #[must_use]
    pub fn num_idle(&self) -> usize {
        match self {
            Pool::Any(p) => p.num_idle(),
            Pool::Postgres(p) => p.num_idle(),
            Pool::Sqlite(p) => p.num_idle(),
            #[cfg(feature = "sqlite-rusqlite")]
            Pool::SqliteNative(_) => 0,
            #[cfg(feature = "postgres-tokio-postgres")]
            Pool::PostgresNative(p) => p.num_idle(),
        }
    }

    /// Returns the pool's connection options, if this is the `Any` backend.
    ///
    /// This is exposed for tests that verify `PoolConfig` propagation.
    #[must_use]
    pub fn options(&self) -> Option<&sqlx::pool::PoolOptions<sqlx::Any>> {
        match self {
            Pool::Any(any) => Some(any.options()),
            _ => None,
        }
    }

    /// Returns the connection options for a native `sqlx::Postgres` pool, if any.
    #[must_use]
    pub fn postgres_options(&self) -> Option<&sqlx::pool::PoolOptions<sqlx::Postgres>> {
        match self {
            Pool::Postgres(p) => Some(p.options()),
            _ => None,
        }
    }

    /// Returns the connection options for a native `sqlx::Sqlite` pool, if any.
    #[must_use]
    pub fn sqlite_options(&self) -> Option<&sqlx::pool::PoolOptions<sqlx::Sqlite>> {
        match self {
            Pool::Sqlite(p) => Some(p.options()),
            _ => None,
        }
    }

    /// Acquires a connection from the `Any` pool.
    ///
    /// # Errors
    ///
    /// Returns [`crate::Error::NotImplemented`] for native driver-specific pools.
    /// Use the typed `as_*` accessors to reach those drivers.
    pub async fn acquire(&self) -> Result<sqlx::pool::PoolConnection<sqlx::Any>, crate::Error> {
        match self {
            Pool::Any(any) => any.acquire().await.map_err(crate::Error::Sqlx),
            _ => Err(crate::Error::NotImplemented),
        }
    }

    /// Borrows the wrapped `sqlx::Any` pool, if this is the `Any` backend.
    ///
    /// This is a compatibility helper for tests and benchmarks that still want
    /// to use raw `sqlx` against the `Any` backend.
    #[must_use]
    pub fn as_any(&self) -> Option<&sqlx::Pool<sqlx::Any>> {
        match self {
            Pool::Any(any) => Some(any),
            _ => None,
        }
    }

    /// Borrows the wrapped `sqlx::Postgres` pool, if any.
    #[must_use]
    pub fn as_postgres(&self) -> Option<&sqlx::Pool<sqlx::Postgres>> {
        match self {
            Pool::Postgres(p) => Some(p),
            _ => None,
        }
    }

    /// Borrows the wrapped `sqlx::Sqlite` pool, if any.
    #[must_use]
    pub fn as_sqlite(&self) -> Option<&sqlx::Pool<sqlx::Sqlite>> {
        match self {
            Pool::Sqlite(p) => Some(p),
            _ => None,
        }
    }

    /// Borrows the wrapped native `rusqlite` pool, if any.
    #[cfg(feature = "sqlite-rusqlite")]
    #[must_use]
    pub fn as_rusqlite(&self) -> Option<&crate::rusqlite::RusqlitePool> {
        match self {
            Pool::SqliteNative(p) => Some(p),
            _ => None,
        }
    }

    /// Borrows the wrapped native `tokio-postgres` pool, if any.
    #[cfg(feature = "postgres-tokio-postgres")]
    #[must_use]
    pub fn as_tokio_postgres(&self) -> Option<&crate::tokio_postgres::TokioPostgresPool> {
        match self {
            Pool::PostgresNative(p) => Some(p),
            _ => None,
        }
    }

    /// Closes the pool and waits for all connections to finish.
    pub async fn close(&self) {
        match self {
            Pool::Any(p) => p.close().await,
            Pool::Postgres(p) => p.close().await,
            Pool::Sqlite(p) => p.close().await,
            #[cfg(feature = "sqlite-rusqlite")]
            Pool::SqliteNative(_) => (),
            #[cfg(feature = "postgres-tokio-postgres")]
            Pool::PostgresNative(p) => p.close().await,
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
            _ => Box::pin(futures_util::stream::iter(std::iter::once(Err(
                sqlx::Error::Protocol(not_any_message().into()),
            )))),
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
            _ => Box::pin(futures_util::future::ready(Err(sqlx::Error::Protocol(
                not_any_message().into(),
            )))),
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
            _ => Box::pin(futures_util::future::ready(Err(sqlx::Error::Protocol(
                not_any_message().into(),
            )))),
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
            _ => Box::pin(futures_util::future::ready(Err(sqlx::Error::Protocol(
                not_any_message().into(),
            )))),
        }
    }
}

fn not_any_message() -> &'static str {
    "this Pool variant does not support generic sqlx::Any queries; use the typed as_* accessors"
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
    ///
    /// Defaults to `false` to avoid a per-query ping round-trip (or ~10 µs on
    /// SQLite). Set `true` when connections are killed between checkouts.
    pub test_before_acquire: bool,
    /// Whether to reset session state when a connection returns to the pool.
    ///
    /// Only meaningful for the native `tokio-postgres` backend, where it
    /// selects `deadpool`'s `Clean` recycling: temp tables, listens, advisory
    /// locks, `SET` values and any open transaction are discarded on recycle.
    ///
    /// Defaults to `false` because it costs a round trip on every checkout —
    /// measured at roughly 2× the total per-query latency on a local database
    /// — and the driver exists to avoid exactly that. Correctness does not
    /// depend on it: an abandoned transaction is rolled back before its
    /// connection is released. Turn it on as defence in depth when application
    /// code sets session state it does not clean up itself.
    pub reset_on_recycle: bool,
    /// Number of rows the SQLite driver buffers per prepared statement.
    ///
    /// This is only meaningful when `connect`/`connect_with` build a native
    /// SQLite pool. The default matches `sqlx-sqlite`'s own default.
    pub row_buffer_size: u32,
}

impl Default for PoolConfig {
    fn default() -> Self {
        Self {
            max_connections: 10,
            min_connections: 0,
            acquire_timeout: Duration::from_secs(30),
            idle_timeout: Some(Duration::from_secs(600)),
            max_lifetime: Some(Duration::from_secs(1800)),
            test_before_acquire: false,
            reset_on_recycle: false,
            row_buffer_size: 1024,
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

    let scheme = url.split(':').next().unwrap_or("");
    match scheme {
        "postgres" | "postgresql" => {
            #[cfg(feature = "postgres-tokio-postgres")]
            if url
                .split_once('?')
                .is_some_and(|(_, q)| q.contains("driver=tokio-postgres"))
            {
                let pool = crate::tokio_postgres::TokioPostgresPool::connect(url, config).await?;
                return Ok(Pool::PostgresNative(pool));
            }

            let pool = sqlx::postgres::PgPoolOptions::new()
                .max_connections(config.max_connections)
                .min_connections(config.min_connections)
                .acquire_timeout(config.acquire_timeout)
                .idle_timeout(config.idle_timeout)
                .max_lifetime(config.max_lifetime)
                .test_before_acquire(config.test_before_acquire)
                .connect(url)
                .await
                .map_err(crate::Error::Sqlx)?;
            Ok(Pool::Postgres(pool))
        }
        "sqlite" => {
            #[cfg(feature = "sqlite-rusqlite")]
            if url
                .split_once('?')
                .is_some_and(|(_, q)| q.contains("driver=rusqlite"))
            {
                let pool = crate::rusqlite::RusqlitePool::connect(url, config).await?;
                return Ok(Pool::SqliteNative(pool));
            }

            let mut connect_opts =
                sqlx::sqlite::SqliteConnectOptions::from_str(url).map_err(crate::Error::Sqlx)?;
            connect_opts = connect_opts
                .row_buffer_size(config.row_buffer_size as usize)
                .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal)
                .synchronous(sqlx::sqlite::SqliteSynchronous::Normal)
                .statement_cache_capacity(256)
                .pragma("cache_size", "-64000")
                .pragma("mmap_size", "268435456")
                .pragma("temp_store", "MEMORY");

            let pool = SqlitePoolOptions::new()
                .max_connections(config.max_connections)
                .min_connections(config.min_connections)
                .acquire_timeout(config.acquire_timeout)
                .idle_timeout(config.idle_timeout)
                .max_lifetime(config.max_lifetime)
                .test_before_acquire(config.test_before_acquire)
                .connect_with(connect_opts)
                .await
                .map_err(crate::Error::Sqlx)?;
            Ok(Pool::Sqlite(pool))
        }
        _ => {
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
    }
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
    crate::executor::Executor::execute_raw(pool, std::borrow::Cow::from("SELECT 1"), Vec::new())
        .await
        .map(|_| ())
}
