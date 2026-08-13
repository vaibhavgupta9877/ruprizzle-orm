//! Dual-database test harness.
//!
//! Integration tests are written **once** and run against **every** backend. That
//! is the whole point: a Postgres-only test suite lets the `SQLite` dialect rot
//! quietly until someone tries to use it, which is exactly the failure the
//! project's reality check warned about.
//!
//! ```ignore
//! use ruprizzle_testkit::{both_dbs, TestDb};
//!
//! both_dbs! {
//!     setup = "CREATE TABLE t (id INTEGER PRIMARY KEY, name TEXT NOT NULL)";
//!     async fn insert_then_count(db: TestDb) {
//!         db.execute("INSERT INTO t (id, name) VALUES (1, 'a')").await?;
//!         assert_eq!(db.fetch_i64("SELECT count(*) FROM t").await?, 1);
//!     }
//! }
//! ```
//!
//! This generates `insert_then_count::postgres`, `insert_then_count::sqlite`,
//! and `insert_then_count::mysql`.
//!
//! # Isolation
//!
//! Each Postgres test gets its own `rz_<uuid>` schema and each `SQLite` test gets
//! its own file in a temporary directory, so the suite runs concurrently without
//! tests colliding. Both are torn down when the [`TestDb`] drops.
//!
//! # When a database is missing
//!
//! Postgres needs a running server. If none is reachable the generated test
//! **skips** with a printed notice, so a contributor without Docker still gets a
//! green `cargo test`. Set `RUPRIZZLE_REQUIRE_DB=1` — as CI does — to turn a skip
//! into a failure, which is what keeps the skip from hiding real breakage.

#![forbid(unsafe_code)]
#![warn(missing_docs, clippy::pedantic)]

use std::time::Duration;

use sqlx::any::AnyPoolOptions;

/// A `sqlx` pool over the `Any` driver.
pub type AnyPool = sqlx::Pool<sqlx::Any>;
use sqlx::mysql::{MySqlPool, MySqlPoolOptions};
use sqlx::postgres::{PgPool, PgPoolOptions};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePool, SqlitePoolOptions};
use tempfile::TempDir;

/// Error type used by test bodies, so `?` works on anything.
pub type BoxError = Box<dyn std::error::Error + Send + Sync>;

/// Result type used by test bodies.
pub type Result<T = (), E = BoxError> = std::result::Result<T, E>;

/// Environment variable that promotes an unavailable database from skip to failure.
pub const REQUIRE_DB_ENV: &str = "RUPRIZZLE_REQUIRE_DB";

/// Environment variable holding the Postgres URL used by tests.
pub const PG_URL_ENV: &str = "RUPRIZZLE_TEST_PG_URL";

const DEFAULT_PG_URL: &str = "postgres://ruprizzle:ruprizzle@localhost:5432/ruprizzle_test";

/// Environment variable holding the `MySQL` / `MariaDB` URL used by tests.
pub const MYSQL_URL_ENV: &str = "RUPRIZZLE_TEST_MYSQL_URL";

const DEFAULT_MYSQL_URL: &str = "mysql://ruprizzle:ruprizzle@localhost:3306";

/// A database backend under test.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Backend {
    /// `PostgreSQL`, reached over the network. May be unavailable locally.
    Postgres,
    /// `SQLite`, in a temporary file. Always available.
    Sqlite,
    /// `MySQL` / `MariaDB`, reached over the network. May be unavailable locally.
    MySql,
}

impl Backend {
    /// Lower-case name, as used in test output and skip notices.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Backend::Postgres => "postgres",
            Backend::Sqlite => "sqlite",
            Backend::MySql => "mysql",
        }
    }
}

impl std::fmt::Display for Backend {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Why a [`TestDb`] could not be created.
#[derive(Debug, thiserror::Error)]
pub enum TestDbError {
    /// The server is not running, or is not reachable with the configured URL.
    #[error("{backend} is unavailable: {reason}")]
    Unavailable {
        /// Which backend was being set up.
        backend: Backend,
        /// Human-readable cause, shown in the skip notice.
        reason: String,
    },
    /// The database was reachable but the setup SQL failed.
    #[error("setup SQL failed on {backend}: {source}")]
    Setup {
        /// Which backend was being set up.
        backend: Backend,
        /// The underlying driver error.
        source: sqlx::Error,
    },
    /// Creating the temporary directory for `SQLite` failed.
    #[error("could not create a temporary directory: {0}")]
    Io(#[from] std::io::Error),
}

/// An isolated, disposable database.
#[derive(Debug)]
pub struct TestDb {
    backend: Backend,
    inner: Inner,
    /// A `ruprizzle` [`Pool`] wrapping the same connections.
    pool: ruprizzle::Pool,
}

#[derive(Debug)]
enum Inner {
    Postgres {
        pg_pool: PgPool,
        any_pool: AnyPool,
        schema: String,
        admin_url: String,
    },
    MySql {
        _mysql_pool: MySqlPool,
        any_pool: AnyPool,
    },
    Sqlite {
        sqlite_pool: SqlitePool,
        any_pool: AnyPool,
        // Held so the directory outlives the pool; removed on drop.
        _dir: TempDir,
    },
}

impl TestDb {
    /// Creates an isolated database and applies `setup_sql`.
    ///
    /// `setup_sql` may contain several statements separated by `;`, and may be
    /// empty.
    ///
    /// # Errors
    ///
    /// [`TestDbError::Unavailable`] if the backend cannot be reached — callers
    /// should treat that as a skip, not a failure — and [`TestDbError::Setup`] if
    /// the database is reachable but the SQL is wrong, which is a real failure.
    pub async fn connect(
        backend: Backend,
        setup_sql: &str,
    ) -> std::result::Result<Self, TestDbError> {
        sqlx::any::install_default_drivers();

        let db = match backend {
            Backend::Postgres => Self::connect_postgres().await?,
            Backend::Sqlite => Self::connect_sqlite().await?,
            Backend::MySql => Self::connect_mysql().await?,
        };

        if !setup_sql.trim().is_empty() {
            db.run_setup(setup_sql)
                .await
                .map_err(|source| TestDbError::Setup { backend, source })?;
        }

        Ok(db)
    }

    async fn connect_postgres() -> std::result::Result<Self, TestDbError> {
        let url = std::env::var(PG_URL_ENV)
            .or_else(|_| std::env::var("DATABASE_URL"))
            .unwrap_or_else(|_| DEFAULT_PG_URL.to_owned());

        let admin = PgPoolOptions::new()
            .max_connections(1)
            .acquire_timeout(Duration::from_secs(5))
            .connect(&url)
            .await
            .map_err(|e| TestDbError::Unavailable {
                backend: Backend::Postgres,
                reason: format!(
                    "{e} (url from {PG_URL_ENV}/DATABASE_URL, default {DEFAULT_PG_URL})"
                ),
            })?;

        let schema = format!("rz_{}", uuid::Uuid::new_v4().simple());
        sqlx::query(&format!(r#"CREATE SCHEMA "{schema}""#))
            .execute(&admin)
            .await
            .map_err(|source| TestDbError::Setup {
                backend: Backend::Postgres,
                source,
            })?;
        admin.close().await;

        // Every connection in the pool must land in the isolated schema, not just
        // the first one, or a test that outgrows one connection starts writing to
        // `public` halfway through.
        let search_path = schema.clone();
        let pg_pool = PgPoolOptions::new()
            .max_connections(4)
            .acquire_timeout(Duration::from_secs(5))
            .after_connect(move |conn, _meta| {
                let schema = search_path.clone();
                Box::pin(async move {
                    sqlx::query(&format!(r#"SET search_path TO "{schema}""#))
                        .execute(&mut *conn)
                        .await?;
                    Ok(())
                })
            })
            .connect(&url)
            .await
            .map_err(|e| TestDbError::Unavailable {
                backend: Backend::Postgres,
                reason: e.to_string(),
            })?;

        let search_path = schema.clone();
        let any_pool = AnyPoolOptions::new()
            .max_connections(4)
            .acquire_timeout(Duration::from_secs(5))
            .after_connect(move |conn, _meta| {
                let schema = search_path.clone();
                Box::pin(async move {
                    sqlx::query(&format!(r#"SET search_path TO "{schema}""#))
                        .execute(&mut *conn)
                        .await?;
                    Ok(())
                })
            })
            .connect(&url)
            .await
            .map_err(|e| TestDbError::Unavailable {
                backend: Backend::Postgres,
                reason: e.to_string(),
            })?;

        let pool = ruprizzle::Pool::Any(any_pool.clone());
        Ok(TestDb {
            backend: Backend::Postgres,
            inner: Inner::Postgres {
                pg_pool,
                any_pool,
                schema,
                admin_url: url,
            },
            pool,
        })
    }

    #[allow(clippy::too_many_lines)]
    async fn connect_mysql() -> std::result::Result<Self, TestDbError> {
        let url = std::env::var(MYSQL_URL_ENV)
            .or_else(|_| std::env::var("DATABASE_URL"))
            .unwrap_or_else(|_| DEFAULT_MYSQL_URL.to_owned());

        let mysql_pool = MySqlPoolOptions::new()
            .max_connections(4)
            .acquire_timeout(Duration::from_secs(5))
            .after_connect(|conn, _meta| {
                Box::pin(async move {
                    sqlx::query("SET SESSION sql_mode = 'ANSI_QUOTES'")
                        .execute(conn)
                        .await?;
                    Ok(())
                })
            })
            .connect(&url)
            .await
            .map_err(|e| TestDbError::Unavailable {
                backend: Backend::MySql,
                reason: format!(
                    "{e} (url from {MYSQL_URL_ENV}/DATABASE_URL, default {DEFAULT_MYSQL_URL})"
                ),
            })?;

        let any_pool = AnyPoolOptions::new()
            .max_connections(4)
            .acquire_timeout(Duration::from_secs(5))
            .after_connect(|conn, _meta| {
                Box::pin(async move {
                    sqlx::query("SET SESSION sql_mode = 'ANSI_QUOTES'")
                        .execute(&mut *conn)
                        .await?;
                    Ok(())
                })
            })
            .connect(&url)
            .await
            .map_err(|e| TestDbError::Unavailable {
                backend: Backend::MySql,
                reason: e.to_string(),
            })?;

        let pool = ruprizzle::Pool::Mysql(mysql_pool.clone());
        Ok(TestDb {
            backend: Backend::MySql,
            inner: Inner::MySql {
                _mysql_pool: mysql_pool,
                any_pool,
            },
            pool,
        })
    }

    #[allow(clippy::too_many_lines)]
    async fn connect_sqlite() -> std::result::Result<Self, TestDbError> {
        let dir = tempfile::tempdir()?;
        let path = dir.path().join("test.sqlite");
        let file = path.to_str().unwrap().replace('\\', "/");
        let url = format!("sqlite:///{file}");

        // When the `sqlite-rusqlite` feature is enabled and the environment
        // variable is set, route the ORM pool through the native `rusqlite`
        // backend while keeping a regular `sqlx` `Any` pool for raw helper
        // methods.
        #[cfg(feature = "sqlite-rusqlite")]
        if std::env::var("RUPRIZZLE_TEST_RUSQLITE").is_ok() {
            let driver_url = format!("{url}?mode=rwc&driver=rusqlite");
            let mut config = ruprizzle::PoolConfig::default();
            config.max_connections = 4;
            let pool = ruprizzle::connect_with(&driver_url, &config)
                .await
                .map_err(|e| TestDbError::Unavailable {
                    backend: Backend::Sqlite,
                    reason: e.to_string(),
                })?;

            let any_pool = AnyPoolOptions::new()
                .max_connections(4)
                .acquire_timeout(Duration::from_secs(5))
                .after_connect(|conn, _meta| {
                    Box::pin(async move {
                        sqlx::query("PRAGMA foreign_keys = ON")
                            .execute(&mut *conn)
                            .await?;
                        sqlx::query("PRAGMA busy_timeout = 5000")
                            .execute(&mut *conn)
                            .await?;
                        Ok(())
                    })
                })
                .connect(&url)
                .await
                .map_err(|e| TestDbError::Unavailable {
                    backend: Backend::Sqlite,
                    reason: e.to_string(),
                })?;

            let opts = SqliteConnectOptions::new()
                .filename(&path)
                .create_if_missing(true)
                .foreign_keys(true)
                .busy_timeout(Duration::from_secs(5));
            let sqlite_pool = SqlitePoolOptions::new()
                .max_connections(4)
                .acquire_timeout(Duration::from_secs(5))
                .connect_with(opts)
                .await
                .map_err(|e| TestDbError::Unavailable {
                    backend: Backend::Sqlite,
                    reason: e.to_string(),
                })?;

            return Ok(TestDb {
                backend: Backend::Sqlite,
                inner: Inner::Sqlite {
                    sqlite_pool,
                    any_pool,
                    _dir: dir,
                },
                pool,
            });
        }

        let opts = SqliteConnectOptions::new()
            .filename(&path)
            .create_if_missing(true)
            // Off by default in SQLite, which would make every foreign-key test
            // silently pass. Matches what the runtime will set in P4.
            .foreign_keys(true)
            .busy_timeout(Duration::from_secs(5));

        let sqlite_pool = SqlitePoolOptions::new()
            .max_connections(4)
            .acquire_timeout(Duration::from_secs(5))
            .connect_with(opts)
            .await
            .map_err(|e| TestDbError::Unavailable {
                backend: Backend::Sqlite,
                reason: e.to_string(),
            })?;

        let any_pool = AnyPoolOptions::new()
            .max_connections(4)
            .acquire_timeout(Duration::from_secs(5))
            .after_connect(|conn, _meta| {
                Box::pin(async move {
                    sqlx::query("PRAGMA foreign_keys = ON")
                        .execute(&mut *conn)
                        .await?;
                    sqlx::query("PRAGMA busy_timeout = 5000")
                        .execute(&mut *conn)
                        .await?;
                    Ok(())
                })
            })
            .connect(&url)
            .await
            .map_err(|e| TestDbError::Unavailable {
                backend: Backend::Sqlite,
                reason: e.to_string(),
            })?;

        let pool = ruprizzle::Pool::Any(any_pool.clone());
        Ok(TestDb {
            backend: Backend::Sqlite,
            inner: Inner::Sqlite {
                sqlite_pool,
                any_pool,
                _dir: dir,
            },
            pool,
        })
    }

    async fn run_setup(&self, sql: &str) -> std::result::Result<(), sqlx::Error> {
        sqlx::raw_sql(sql)
            .execute(self.any_pool())
            .await
            .map(|_| ())
    }

    /// Which backend this database is.
    #[must_use]
    pub const fn backend(&self) -> Backend {
        self.backend
    }

    /// A `ruprizzle` [`Pool`](ruprizzle::Pool) backed by this database.
    ///
    /// Use this when exercising the runtime's query builders and executor.
    #[must_use]
    pub fn pool(&self) -> &ruprizzle::Pool {
        &self.pool
    }

    /// The database-agnostic `Any` pool, for tests that go through raw `sqlx`.
    #[must_use]
    pub fn any_pool(&self) -> &AnyPool {
        match &self.inner {
            Inner::Postgres { any_pool, .. }
            | Inner::MySql { any_pool, .. }
            | Inner::Sqlite { any_pool, .. } => any_pool,
        }
    }

    /// The Postgres pool, for tests that need driver-specific access.
    #[must_use]
    pub fn pg_pool(&self) -> Option<&PgPool> {
        match &self.inner {
            Inner::Postgres { pg_pool, .. } => Some(pg_pool),
            Inner::MySql { .. } | Inner::Sqlite { .. } => None,
        }
    }

    /// The `SQLite` pool, for tests that need driver-specific access.
    #[must_use]
    pub fn sqlite_pool(&self) -> Option<&SqlitePool> {
        match &self.inner {
            Inner::Sqlite { sqlite_pool, .. } => Some(sqlite_pool),
            Inner::Postgres { .. } | Inner::MySql { .. } => None,
        }
    }

    /// Runs one or more statements, returning rows affected by the last one.
    ///
    /// # Errors
    ///
    /// Propagates any driver error.
    pub async fn execute(&self, sql: &str) -> std::result::Result<u64, sqlx::Error> {
        sqlx::raw_sql(sql)
            .execute(self.any_pool())
            .await
            .map(|r| r.rows_affected())
    }

    /// Runs a query expected to yield exactly one integer.
    ///
    /// # Errors
    ///
    /// Propagates any driver error, including a row/column shape mismatch.
    pub async fn fetch_i64(&self, sql: &str) -> std::result::Result<i64, sqlx::Error> {
        sqlx::query_scalar(sql).fetch_one(self.any_pool()).await
    }

    /// Runs a query expected to yield exactly one string.
    ///
    /// # Errors
    ///
    /// Propagates any driver error, including a row/column shape mismatch.
    pub async fn fetch_string(&self, sql: &str) -> std::result::Result<String, sqlx::Error> {
        sqlx::query_scalar(sql).fetch_one(self.any_pool()).await
    }
}

impl Drop for TestDb {
    fn drop(&mut self) {
        // Postgres schemas outlive the process, so they must be cleaned up
        // explicitly or a long-running CI database accumulates thousands of them.
        // SQLite needs nothing here: `TempDir` removes the file.
        if let Inner::Postgres {
            schema, admin_url, ..
        } = &self.inner
        {
            let (schema, url) = (schema.clone(), admin_url.clone());
            if let Ok(handle) = tokio::runtime::Handle::try_current() {
                handle.spawn(async move {
                    if let Ok(pool) = PgPoolOptions::new().max_connections(1).connect(&url).await {
                        let _ =
                            sqlx::query(&format!(r#"DROP SCHEMA IF EXISTS "{schema}" CASCADE"#))
                                .execute(&pool)
                                .await;
                        pool.close().await;
                    }
                });
            }
        }
    }
}

/// Runs one case of a [`both_dbs!`] test, applying the skip policy.
///
/// Not intended to be called directly; the macro generates the calls.
///
/// # Panics
///
/// Panics if the test body fails, if setup SQL fails, or if the backend is
/// unavailable while `RUPRIZZLE_REQUIRE_DB=1` is set.
pub async fn run_case<F, Fut>(backend: Backend, setup_sql: &str, body: F)
where
    F: FnOnce(TestDb) -> Fut,
    Fut: std::future::Future<Output = Result>,
{
    let db = match TestDb::connect(backend, setup_sql).await {
        Ok(db) => db,
        Err(TestDbError::Unavailable { reason, .. }) => {
            let required = std::env::var(REQUIRE_DB_ENV).is_ok_and(|v| v != "0" && !v.is_empty());
            assert!(
                !required,
                "{backend} is required ({REQUIRE_DB_ENV} is set) but unavailable: {reason}"
            );
            eprintln!("skipping {backend}: {reason}");
            eprintln!("  (set {REQUIRE_DB_ENV}=1 to make this a failure instead)");
            return;
        }
        Err(e) => panic!("{e}"),
    };

    if let Err(e) = body(db).await {
        panic!("test body failed on {backend}: {e}");
    }
}

/// Defines one test that runs against every supported backend.
///
/// See the [module documentation](self) for the shape of a case.
#[macro_export]
macro_rules! both_dbs {
    (
        setup = $setup:expr;
        $(#[$attr:meta])*
        async fn $name:ident ( $db:ident : TestDb ) $body:block
    ) => {
        $(#[$attr])*
        mod $name {
            #[allow(unused_imports)]
            use super::*;

            async fn case($db: $crate::TestDb) -> $crate::Result {
                $body
                Ok(())
            }

            #[::tokio::test]
            async fn postgres() {
                $crate::run_case($crate::Backend::Postgres, $setup, case).await;
            }

            #[::tokio::test]
            async fn sqlite() {
                $crate::run_case($crate::Backend::Sqlite, $setup, case).await;
            }

            #[::tokio::test]
            async fn mysql() {
                $crate::run_case($crate::Backend::MySql, $setup, case).await;
            }
        }
    };

    (
        $(#[$attr:meta])*
        async fn $name:ident ( $db:ident : TestDb ) $body:block
    ) => {
        $crate::both_dbs! {
            setup = "";
            $(#[$attr])*
            async fn $name($db: TestDb) $body
        }
    };
}
