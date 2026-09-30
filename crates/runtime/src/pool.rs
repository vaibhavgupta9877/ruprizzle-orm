//! Connection pool construction, replica routing, and configuration.

use std::hash::{BuildHasher, RandomState};
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
    /// Selects a random healthy replica for each call.
    ///
    /// The draw uses the standard library's randomly keyed `SipHash`: it is
    /// uniform and unpredictable enough to spread load, but it is not a
    /// cryptographic RNG.
    Random,
}

/// A monitored read replica connection pool with health and active connection tracking.
#[derive(Debug, Clone)]
pub struct ReplicaPool {
    /// The underlying database connection pool.
    pub pool: Pool,
    /// Liveness / health status indicator.
    pub healthy: Arc<AtomicBool>,
    /// Number of routed reads currently in flight on this replica.
    ///
    /// Every read the [`RoutedPool`] executor sends here holds one count until
    /// its result is returned or, for a stream, until the stream is dropped.
    /// `LoadBalancing::LeastConnections` picks the replica with the lowest count.
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
/// Used as an [`Executor`](crate::Executor), it sends a statement to a healthy
/// replica only when the statement is a plain read: it starts with `SELECT`,
/// `WITH`, `VALUES`, `TABLE` or `SHOW`, and contains none of `INSERT`,
/// `UPDATE`, `DELETE`, `MERGE`, `INTO`, `RETURNING`, `LOCK`, `SHARE`,
/// `NEXTVAL` or `SETVAL` as a word. Everything else, including
/// `SELECT … FOR UPDATE` and a data-modifying `WITH`, runs on the primary. The
/// check is by keyword, so a string literal that contains one of those words
/// also sends the read to the primary; that costs load, never correctness.
/// `execute_raw` and transactions always use the primary.
///
/// Health is not checked in the background. A replica stays healthy until
/// [`check_health`](Self::check_health) or [`ReplicaPool::set_healthy`] says
/// otherwise, so run `check_health` on a timer from your own supervisor.
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

    /// Selects the pool a read query should use.
    ///
    /// This is a healthy replica chosen by the load-balancing strategy. When no
    /// replica is healthy it is the primary, unless `fallback_to_primary(false)`
    /// was set: then it is the first configured replica, even though it is
    /// marked unhealthy, so the read fails or succeeds on a replica and never
    /// reaches the primary. With no replicas configured it is the primary.
    ///
    /// A pool returned here is not counted in [`ReplicaPool::active_conns`];
    /// only reads made through the `RoutedPool` executor are. That executor also
    /// refuses a read outright when fallback is off and no replica is healthy.
    #[must_use]
    pub fn select_replica(&self) -> &Pool {
        match self.route_read() {
            ReadRoute::Replica(r) => &r.pool,
            ReadRoute::Primary => &self.primary,
            ReadRoute::NoHealthyReplica => self.replicas.first().map_or(&self.primary, |r| &r.pool),
        }
    }

    /// Where a read goes: a healthy replica, the primary, or nowhere.
    pub(crate) fn route_read(&self) -> ReadRoute<'_> {
        match self.pick_healthy() {
            Some(replica) => ReadRoute::Replica(replica),
            None if self.fallback_to_primary || self.replicas.is_empty() => ReadRoute::Primary,
            None => ReadRoute::NoHealthyReplica,
        }
    }

    fn pick_healthy(&self) -> Option<&ReplicaPool> {
        let healthy: Vec<&ReplicaPool> = self.replicas.iter().filter(|r| r.is_healthy()).collect();
        let count = healthy.len();
        if count == 0 {
            return None;
        }

        match self.load_balancing {
            LoadBalancing::RoundRobin => {
                let idx = self.rr_index.fetch_add(1, Ordering::Relaxed);
                healthy.get(idx.checked_rem(count).unwrap_or(0)).copied()
            }
            LoadBalancing::LeastConnections => {
                healthy.into_iter().min_by_key(|r| r.active_connections())
            }
            LoadBalancing::Random => {
                // A fresh `RandomState` is keyed per thread from the OS RNG and
                // re-keyed on every call, so hashing a counter gives a uniform draw.
                let draw =
                    RandomState::new().hash_one(self.rr_index.fetch_add(1, Ordering::Relaxed));
                let bound = u64::try_from(count).unwrap_or(u64::MAX);
                let target = draw
                    .checked_rem(bound)
                    .and_then(|t| usize::try_from(t).ok())
                    .unwrap_or(0);
                healthy.get(target).copied()
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

/// Where [`RoutedPool::route_read`] sends a read.
pub(crate) enum ReadRoute<'a> {
    Replica(&'a ReplicaPool),
    Primary,
    /// Every replica is unhealthy and `fallback_to_primary` is off.
    NoHealthyReplica,
}

/// Holds one count in a replica's `active_conns` while a routed read runs.
pub(crate) struct InFlight(Arc<AtomicUsize>);

impl InFlight {
    pub(crate) fn start(replica: &ReplicaPool) -> Self {
        replica.active_conns.fetch_add(1, Ordering::Relaxed);
        Self(Arc::clone(&replica.active_conns))
    }
}

impl Drop for InFlight {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::Relaxed);
    }
}

/// Whether a raw statement may run on a replica.
///
/// Conservative by design: any doubt sends the statement to the primary. The
/// first keyword (after comments) must be a read, and no word anywhere in the
/// text may write, lock, or advance a sequence.
pub(crate) fn is_replica_safe(sql: &str) -> bool {
    const READ_START: &[&str] = &["SELECT", "WITH", "VALUES", "TABLE", "SHOW"];
    const NOT_ON_REPLICA: &[&str] = &[
        "INSERT",
        "UPDATE",
        "DELETE",
        "MERGE",
        "INTO",
        "RETURNING",
        "LOCK",
        "SHARE",
        "NEXTVAL",
        "SETVAL",
    ];

    let mut words = strip_leading_comments(sql)
        .split(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
        .filter(|w| !w.is_empty());
    let Some(first) = words.next() else {
        return false;
    };
    READ_START.iter().any(|k| first.eq_ignore_ascii_case(k))
        && !words.any(|w| NOT_ON_REPLICA.iter().any(|k| w.eq_ignore_ascii_case(k)))
}

fn strip_leading_comments(mut sql: &str) -> &str {
    loop {
        sql = sql.trim_start();
        if let Some(rest) = sql.strip_prefix("--") {
            sql = rest.split_once('\n').map_or("", |(_, after)| after);
        } else if let Some(rest) = sql.strip_prefix("/*") {
            sql = rest.split_once("*/").map_or("", |(_, after)| after);
        } else {
            return sql;
        }
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

    /// Whether reads go to the primary when every replica is unhealthy (default `true`).
    ///
    /// With `false`, a read made through the `RoutedPool` executor fails with an
    /// error instead, so a replica outage cannot move read load onto the
    /// primary. A router with no replicas at all always reads from the primary.
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

#[cfg(test)]
mod tests {
    use super::is_replica_safe;

    #[test]
    fn plain_reads_may_use_a_replica() {
        for sql in [
            "SELECT * FROM users",
            "  select id from users where updated_at > $1",
            "WITH recent AS (SELECT id FROM posts) SELECT * FROM recent",
            "VALUES (1), (2)",
            "TABLE users",
            "SHOW server_version",
            "-- leading comment\nSELECT 1",
            "/* hint */ SELECT 1",
        ] {
            assert!(is_replica_safe(sql), "{sql}");
        }
    }

    #[test]
    fn writes_locks_and_sequences_stay_on_the_primary() {
        for sql in [
            "INSERT INTO users (name) VALUES ($1) RETURNING id",
            "UPDATE users SET name = $1 RETURNING *",
            "DELETE FROM users WHERE id = $1 RETURNING id",
            "WITH gone AS (DELETE FROM users RETURNING id) SELECT * FROM gone",
            "SELECT * FROM users WHERE id = $1 FOR UPDATE",
            "SELECT * FROM users FOR SHARE",
            "SELECT * FROM users LOCK IN SHARE MODE",
            "SELECT * INTO backup FROM users",
            "SELECT nextval('users_id_seq')",
            "SELECT setval('users_id_seq', 1)",
            "MERGE INTO t USING s ON t.id = s.id WHEN MATCHED THEN DELETE",
            "EXPLAIN ANALYZE DELETE FROM users",
            "CALL refresh()",
            "/* SELECT */ DELETE FROM users",
            "-- SELECT\nUPDATE users SET name = 'x'",
            "",
            "   ",
        ] {
            assert!(!is_replica_safe(sql), "{sql}");
        }
    }
}
