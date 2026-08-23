//! Connection pool construction, replica routing, and configuration.

use std::str::FromStr;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::Duration;

use futures_core::future::BoxFuture;
use futures_core::stream::BoxStream;
use ruprizzle_core::ir::Provider;
use sqlx::any::AnyPoolOptions;
use sqlx::mysql::MySqlPoolOptions;
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
    /// Native MySQL / MariaDB pool.
    Mysql(sqlx::Pool<sqlx::MySql>),
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
            Pool::Mysql(_) => Provider::Mysql,
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
            Pool::Mysql(p) => p.size(),
            #[cfg(feature = "sqlite-rusqlite")]
            Pool::SqliteNative(p) => p.num_total() as u32,
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
            Pool::Mysql(p) => p.num_idle(),
            #[cfg(feature = "sqlite-rusqlite")]
            Pool::SqliteNative(p) => p.num_idle(),
            #[cfg(feature = "postgres-tokio-postgres")]
            Pool::PostgresNative(p) => p.num_idle(),
        }
    }

    /// Connections currently waiting for a checkout.
    ///
    /// `sqlx` pools do not expose this count; they report `0`.
    #[must_use]
    pub fn num_waiters(&self) -> usize {
        match self {
            #[cfg(feature = "postgres-tokio-postgres")]
            Pool::PostgresNative(p) => p.num_waiters(),
            #[cfg(feature = "sqlite-rusqlite")]
            Pool::SqliteNative(p) => p.num_waiters(),
            _ => 0,
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

    /// Borrows the wrapped native `sqlx::MySql` pool, if any.
    #[must_use]
    pub fn as_mysql(&self) -> Option<&sqlx::Pool<sqlx::MySql>> {
        match self {
            Pool::Mysql(p) => Some(p),
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
        tracing::info!(target: "ruprizzle::connection", event = "disconnect", "pool closing");
        match self {
            Pool::Any(p) => p.close().await,
            Pool::Postgres(p) => p.close().await,
            Pool::Sqlite(p) => p.close().await,
            Pool::Mysql(p) => p.close().await,
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
    /// Maximum child-table rows that can be loaded in a single batched `include`
    /// query before the loader falls back to a chunked `IN (...)` list.
    ///
    /// This bounds the full-table fast path so a parent table with millions of
    /// rows does not cause the loader to materialise and decode the entire child
    /// table. `None` disables fast-path include loading entirely and always uses
    /// chunked `IN`. Defaults to `100_000`.
    pub full_table_include_limit: Option<u64>,
    /// Duration above which a query emits a `WARN` event with the SQL shape and
    /// bind count. `None` disables slow-query warnings.
    ///
    /// This is a process-wide setting; the last `connect_with` call wins.
    pub slow_query_threshold: Option<Duration>,
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
            full_table_include_limit: Some(100_000),
            slow_query_threshold: None,
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
    crate::executor::set_full_table_include_limit(config.full_table_include_limit);
    crate::executor::set_slow_query_threshold(config.slow_query_threshold);
    sqlx::any::install_default_drivers();

    let scheme = url.split(':').next().unwrap_or("");

    tracing::info!(
        target: "ruprizzle::connection",
        event = "connect",
        scheme,
        "connecting to database"
    );
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
                .after_connect(|_conn, _meta| {
                    Box::pin(async move {
                        tracing::info!(
                            target: "ruprizzle::connection",
                            event = "connect",
                            backend = "postgres",
                            "sqlx connection opened"
                        );
                        Ok(())
                    })
                })
                .connect(url)
                .await
                .map_err(crate::Error::Sqlx)?;
            Ok(Pool::Postgres(pool))
        }
        "mysql" | "mariadb" => {
            let pool = MySqlPoolOptions::new()
                .max_connections(config.max_connections)
                .min_connections(config.min_connections)
                .acquire_timeout(config.acquire_timeout)
                .idle_timeout(config.idle_timeout)
                .max_lifetime(config.max_lifetime)
                .test_before_acquire(config.test_before_acquire)
                .after_connect(|_conn, _meta| {
                    Box::pin(async move {
                        tracing::info!(
                            target: "ruprizzle::connection",
                            event = "connect",
                            backend = "mysql",
                            "sqlx connection opened"
                        );
                        Ok(())
                    })
                })
                .connect(url)
                .await
                .map_err(crate::Error::Sqlx)?;
            Ok(Pool::Mysql(pool))
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
                .busy_timeout(Duration::from_secs(30))
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
                .after_connect(|_conn, _meta| {
                    Box::pin(async move {
                        tracing::info!(
                            target: "ruprizzle::connection",
                            event = "connect",
                            backend = "sqlite",
                            "sqlx connection opened"
                        );
                        Ok(())
                    })
                })
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
                .after_connect(|_conn, _meta| {
                    Box::pin(async move {
                        tracing::info!(
                            target: "ruprizzle::connection",
                            event = "connect",
                            backend = "any",
                            "sqlx connection opened"
                        );
                        Ok(())
                    })
                })
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
    /// Connections waiting to be checked out.
    pub waiters: usize,
}

/// Samples the current pool saturation.
#[must_use]
pub fn stats(pool: &Pool) -> PoolStats {
    let size = pool.size();
    let idle = pool.num_idle();
    let waiters = pool.num_waiters();
    PoolStats {
        size,
        idle,
        in_use: (size as usize).saturating_sub(idle),
        waiters,
    }
}

/// Emits pool saturation as `metrics` gauges.
///
/// This can be called by users with a `metrics` recorder installed to update
/// the current pool snapshot. When the `metrics` feature is disabled it is a
/// no-op and simply returns the sampled stats.
#[must_use]
pub fn report_metrics(pool: &Pool) -> PoolStats {
    let s = stats(pool);
    #[cfg(feature = "metrics")]
    {
        use crate::metrics::{POOL_IDLE, POOL_IN_USE, POOL_SIZE, POOL_WAITERS, gauge};
        gauge(POOL_SIZE, s.size as f64);
        gauge(POOL_IDLE, s.idle as f64);
        gauge(POOL_IN_USE, s.in_use as f64);
        gauge(POOL_WAITERS, s.waiters as f64);
    }
    s
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

/// Load balancing strategy across read replica pools.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LoadBalancing {
    /// Cycles requests across replicas sequentially.
    #[default]
    RoundRobin,
    /// Directs requests to the replica with the fewest active connections.
    LeastConnections,
    /// Selects a random healthy replica.
    Random,
}

/// A monitored read replica connection pool with health and active connection tracking.
#[derive(Debug, Clone)]
pub struct ReplicaPool {
    /// The underlying database connection pool.
    pub pool: Pool,
    /// Liveness / health status indicator.
    pub healthy: Arc<AtomicBool>,
    /// Active query / connection count currently in flight on this replica.
    pub active_conns: Arc<AtomicUsize>,
}

impl ReplicaPool {
    /// Wraps a pool as a healthy replica.
    #[must_use]
    pub fn new(pool: Pool) -> Self {
        Self {
            pool,
            healthy: Arc::new(AtomicBool::new(true)),
            active_conns: Arc::new(AtomicUsize::new(0)),
        }
    }

    /// Returns `true` if this replica is marked healthy.
    #[must_use]
    pub fn is_healthy(&self) -> bool {
        self.healthy.load(Ordering::Relaxed)
    }

    /// Marks this replica healthy or unhealthy.
    pub fn set_healthy(&self, healthy: bool) {
        self.healthy.store(healthy, Ordering::Relaxed);
    }

    /// Number of active in-flight operations on this replica.
    #[must_use]
    pub fn active_connections(&self) -> usize {
        self.active_conns.load(Ordering::Relaxed)
    }
}

/// Primary / Read-Replica connection router.
///
/// Automatically distributes read queries (`SELECT`) across healthy replicas
/// using configurable load balancing algorithms, while routing writes
/// (`INSERT`, `UPDATE`, `DELETE`) and transactions exclusively to the primary.
#[derive(Debug, Clone)]
pub struct RoutedPool {
    pub(crate) primary: Pool,
    pub(crate) replicas: Vec<ReplicaPool>,
    pub(crate) load_balancing: LoadBalancing,
    pub(crate) rr_index: Arc<AtomicUsize>,
    pub(crate) fallback_to_primary: bool,
}

impl RoutedPool {
    /// Returns a builder for configuring a `RoutedPool`.
    #[must_use]
    pub fn builder(primary: Pool) -> RoutedPoolBuilder {
        RoutedPoolBuilder::new(primary)
    }

    /// Reference to the primary database connection pool.
    #[must_use]
    pub fn primary(&self) -> &Pool {
        &self.primary
    }

    /// List of configured read replica pools.
    #[must_use]
    pub fn replicas(&self) -> &[ReplicaPool] {
        &self.replicas
    }

    /// Load balancing strategy in use.
    #[must_use]
    pub fn load_balancing(&self) -> LoadBalancing {
        self.load_balancing
    }

    /// Begins a new transaction on the primary database pool.
    pub async fn begin(&self) -> Result<crate::tx::Tx, crate::Error> {
        self.primary.begin().await
    }

    /// Selects an appropriate replica (or primary fallback) for a read query.
    #[must_use]
    pub fn select_replica(&self) -> &Pool {
        let healthy: Vec<&ReplicaPool> = self.replicas.iter().filter(|r| r.is_healthy()).collect();

        let count = healthy.len();
        if count == 0 {
            return &self.primary;
        }

        match self.load_balancing {
            LoadBalancing::RoundRobin => {
                let idx = self.rr_index.fetch_add(1, Ordering::Relaxed);
                let target = idx.checked_rem(count).unwrap_or(0);
                healthy.get(target).map_or(&self.primary, |r| &r.pool)
            }
            LoadBalancing::LeastConnections => {
                let best = healthy.iter().min_by_key(|r| r.active_connections());
                best.map_or(&self.primary, |r| &r.pool)
            }
            LoadBalancing::Random => {
                let seed = self
                    .rr_index
                    .fetch_add(1, Ordering::Relaxed)
                    .wrapping_mul(2654435761);
                let target = seed.checked_rem(count).unwrap_or(0);
                healthy.get(target).map_or(&self.primary, |r| &r.pool)
            }
        }
    }

    /// Runs health check pings across all replicas and updates their healthy flags.
    pub async fn check_health(&self) {
        for replica in &self.replicas {
            let ok = ping(&replica.pool).await.is_ok();
            replica.set_healthy(ok);
        }
    }

    /// Whether this router falls back to the primary pool when all replicas are unavailable.
    #[must_use]
    pub fn falls_back_to_primary(&self) -> bool {
        self.fallback_to_primary
    }
}

/// Builder for constructing a [`RoutedPool`].
#[derive(Debug)]
pub struct RoutedPoolBuilder {
    primary: Pool,
    replicas: Vec<ReplicaPool>,
    load_balancing: LoadBalancing,
    fallback_to_primary: bool,
}

impl RoutedPoolBuilder {
    /// Creates a new builder with the designated primary pool.
    #[must_use]
    pub fn new(primary: Pool) -> Self {
        Self {
            primary,
            replicas: Vec::new(),
            load_balancing: LoadBalancing::RoundRobin,
            fallback_to_primary: true,
        }
    }

    /// Adds a read replica pool.
    #[must_use]
    pub fn add_replica(mut self, replica: Pool) -> Self {
        self.replicas.push(ReplicaPool::new(replica));
        self
    }

    /// Sets the replica load balancing strategy.
    #[must_use]
    pub fn load_balancing(mut self, lb: LoadBalancing) -> Self {
        self.load_balancing = lb;
        self
    }

    /// Whether to fall back to the primary pool when all replicas are unhealthy or none configured.
    #[must_use]
    pub fn fallback_to_primary(mut self, fallback: bool) -> Self {
        self.fallback_to_primary = fallback;
        self
    }

    /// Builds the `RoutedPool`.
    #[must_use]
    pub fn build(self) -> RoutedPool {
        RoutedPool {
            primary: self.primary,
            replicas: self.replicas,
            load_balancing: self.load_balancing,
            rr_index: Arc::new(AtomicUsize::new(0)),
            fallback_to_primary: self.fallback_to_primary,
        }
    }
}
