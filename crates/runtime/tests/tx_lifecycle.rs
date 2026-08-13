//! Transaction lifecycle invariants for every backend.
//!
//! The rule under test is the same one `sqlx::Transaction` enforces for its own
//! backends: a transaction that is dropped without an explicit `commit()` or
//! `rollback()` must roll back and return its connection to the pool. The
//! hand-written native drivers each implement that themselves, so the checks
//! here are the regression tests for BUG-01, BUG-02 and BUG-03.

#[cfg(feature = "sqlite-rusqlite")]
mod rusqlite_backend {
    use ruprizzle::{Error, Executor, Pool, PoolConfig, connect_with, decode_rows};
    use tempfile::TempDir;

    /// A `rusqlite`-backed pool over a temporary file with `max_connections`
    /// connections and a single `kv(k, v)` table.
    ///
    /// The [`TempDir`] is returned alongside the pool because dropping it deletes
    /// the database file.
    async fn rusqlite_pool(max_connections: u32) -> (Pool, TempDir) {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("tx_lifecycle.sqlite");
        let file = path.to_str().expect("utf-8 path").replace('\\', "/");
        let url = format!("sqlite:///{file}?mode=rwc&driver=rusqlite");

        let mut config = PoolConfig::default();
        config.max_connections = max_connections;

        let pool = connect_with(&url, &config).await.expect("connect");
        assert!(
            pool.as_rusqlite().is_some(),
            "expected the native rusqlite backend"
        );

        pool.execute_raw(
            "CREATE TABLE kv (k TEXT PRIMARY KEY, v TEXT NOT NULL)".into(),
            Vec::new(),
        )
        .await
        .expect("create table");

        (pool, dir)
    }

    /// `SELECT COUNT(*) FROM kv` straight through the pool, so it exercises the
    /// non-transactional `acquire` path rather than a transaction's own connection.
    async fn try_kv_count(pool: &Pool) -> Result<i64, Error> {
        let batch = pool
            .fetch_all_raw("SELECT COUNT(*) FROM kv".into(), Vec::new())
            .await?;
        let rows = decode_rows::<(i64,)>(batch)?;
        rows.first()
            .map(|r| r.0)
            .ok_or_else(|| Error::Message("COUNT(*) returned no rows".into()))
    }

    async fn kv_count(pool: &Pool) -> i64 {
        try_kv_count(pool).await.expect("count")
    }

    /// BUG-01: dropping a transaction leaked its connection, so the pool shrank by
    /// one on every abandoned transaction and never recovered.
    #[tokio::test]
    async fn dropping_a_transaction_returns_its_connection() {
        let (pool, _dir) = rusqlite_pool(2).await;

        for _ in 0..2 {
            let tx = pool.begin().await.expect("begin");
            drop(tx);
        }

        // Before the fix this failed with "rusqlite connection pool exhausted".
        let tx = pool.begin().await.expect("begin after two abandoned txs");
        tx.commit().await.expect("commit");
    }

    /// A dropped transaction must behave as a rollback, not as a commit.
    #[tokio::test]
    async fn dropping_a_transaction_rolls_back_its_writes() {
        let (pool, _dir) = rusqlite_pool(2).await;

        let tx = pool.begin().await.expect("begin");
        tx.execute("INSERT INTO kv (k, v) VALUES ('a', '1')", &[])
            .await
            .expect("insert");
        drop(tx);

        assert_eq!(
            kv_count(&pool).await,
            0,
            "abandoned writes must not persist"
        );
    }

    /// The connection recovered from an abandoned transaction has to be usable, not
    /// merely present: a connection returned with `BEGIN` still open would fail the
    /// next `BEGIN` on it.
    #[tokio::test]
    async fn a_recovered_connection_is_reusable() {
        let (pool, _dir) = rusqlite_pool(1).await;

        let tx = pool.begin().await.expect("begin");
        tx.execute("INSERT INTO kv (k, v) VALUES ('a', '1')", &[])
            .await
            .expect("insert");
        drop(tx);

        let tx = pool.begin().await.expect("begin on recovered connection");
        tx.execute("INSERT INTO kv (k, v) VALUES ('b', '2')", &[])
            .await
            .expect("insert");
        tx.commit().await.expect("commit");

        assert_eq!(kv_count(&pool).await, 1);
    }

    /// Explicit finish paths must keep working unchanged.
    #[tokio::test]
    async fn commit_and_rollback_still_work() {
        let (pool, _dir) = rusqlite_pool(2).await;

        let tx = pool.begin().await.expect("begin");
        tx.execute("INSERT INTO kv (k, v) VALUES ('a', '1')", &[])
            .await
            .expect("insert");
        tx.commit().await.expect("commit");
        assert_eq!(kv_count(&pool).await, 1);

        let tx = pool.begin().await.expect("begin");
        tx.execute("INSERT INTO kv (k, v) VALUES ('b', '2')", &[])
            .await
            .expect("insert");
        tx.rollback().await.expect("rollback");
        assert_eq!(kv_count(&pool).await, 1);
    }

    /// The leak was cumulative, so the interesting assertion is that it does not
    /// accumulate at all across many abandoned transactions.
    #[tokio::test]
    async fn many_abandoned_transactions_leave_the_pool_intact() {
        let (pool, _dir) = rusqlite_pool(2).await;

        for _ in 0..100 {
            let tx = pool.begin().await.expect("begin");
            tx.execute("INSERT INTO kv (k, v) VALUES ('a', '1')", &[])
                .await
                .expect("insert");
            drop(tx);
        }

        let first = pool.begin().await.expect("begin");
        let second = pool.begin().await.expect("begin");
        first.commit().await.expect("commit");
        second.commit().await.expect("commit");

        assert_eq!(kv_count(&pool).await, 0);
    }

    /// BUG-02: with every connection checked out by a transaction, an ordinary
    /// query took `% conns.len()` on an empty vector and panicked.
    #[tokio::test]
    async fn an_exhausted_pool_errors_instead_of_panicking() {
        let (pool, _dir) = rusqlite_pool(1).await;

        let tx = pool.begin().await.expect("begin");

        let err = try_kv_count(&pool)
            .await
            .expect_err("query on an exhausted pool must return an error");
        assert!(
            matches!(err, Error::PoolExhausted { .. }),
            "unexpected error: {err}"
        );

        // And the pool recovers once the transaction finishes.
        tx.commit().await.expect("commit");
        assert_eq!(kv_count(&pool).await, 0);
    }

    /// The same typed error must come out of `begin`, which pops a connection
    /// rather than round-robining over the live ones.
    #[tokio::test]
    async fn beginning_past_capacity_errors() {
        let (pool, _dir) = rusqlite_pool(1).await;

        let held = pool.begin().await.expect("begin");
        let err = pool
            .begin()
            .await
            .expect_err("second begin past capacity must fail");
        assert!(
            matches!(err, Error::PoolExhausted { .. }),
            "unexpected error: {err}"
        );

        held.rollback().await.expect("rollback");
        pool.begin().await.expect("begin after rollback");
    }
}

#[cfg(feature = "postgres-tokio-postgres")]
mod tokio_postgres_backend {
    use ruprizzle::{Executor, Pool, PoolConfig, connect_with, decode_rows};

    const TABLE: &str = "rz_tx_lifecycle_kv";

    /// The Postgres URL for the native `tokio-postgres` driver, or `None` when
    /// no test database is configured.
    ///
    /// Panics rather than skipping under `RUPRIZZLE_REQUIRE_DB`, because a
    /// silently skipped test here would report green while testing nothing —
    /// which is exactly how BUG-03 survived to a published release.
    fn pg_url() -> Option<String> {
        match std::env::var("RUPRIZZLE_TEST_PG_URL") {
            Ok(url) => {
                let sep = if url.contains('?') { '&' } else { '?' };
                Some(format!("{url}{sep}driver=tokio-postgres"))
            }
            Err(_) => {
                assert!(
                    std::env::var("RUPRIZZLE_REQUIRE_DB").is_err(),
                    "RUPRIZZLE_REQUIRE_DB is set but RUPRIZZLE_TEST_PG_URL is not"
                );
                eprintln!("skipping tokio-postgres tx lifecycle tests: no RUPRIZZLE_TEST_PG_URL");
                None
            }
        }
    }

    /// A native `tokio-postgres` pool holding exactly `max_connections`
    /// connections, so a reused connection is necessarily the same one.
    async fn pg_pool(max_connections: u32) -> Pool {
        let url = pg_url().expect("caller checked for a URL");
        let mut config = PoolConfig::default();
        config.max_connections = max_connections;
        config.min_connections = 0;

        let pool = connect_with(&url, &config).await.expect("connect");
        assert!(
            pool.as_tokio_postgres().is_some(),
            "expected the native tokio-postgres backend"
        );
        pool
    }

    async fn exec(pool: &Pool, sql: &str) {
        pool.execute_raw(sql.to_owned().into(), Vec::new())
            .await
            .unwrap_or_else(|e| panic!("{sql}: {e}"));
    }

    /// Row count read over a *separate* pool, so it sees only committed data.
    async fn committed_count(observer: &Pool) -> i64 {
        let batch = observer
            .fetch_all_raw(format!("SELECT COUNT(*) FROM {TABLE}").into(), Vec::new())
            .await
            .expect("count");
        decode_rows::<(i64,)>(batch)
            .expect("decode")
            .first()
            .expect("one row")
            .0
    }

    /// BUG-03: dropping a transaction released the pooled connection with its
    /// `BEGIN` still open, so the next request to receive that connection ran
    /// inside the abandoned transaction.
    ///
    /// The check is deliberately behavioural rather than a `pg_stat_activity`
    /// probe: a write issued through the pool after the drop must be visible to
    /// a *different* session. If the connection still carries an open
    /// transaction, that write stays uncommitted and invisible.
    #[tokio::test]
    async fn dropping_a_transaction_does_not_leak_an_open_transaction() {
        if pg_url().is_none() {
            return;
        }

        // One connection, so the drop and the follow-up write cannot land on
        // different backends.
        let pool = pg_pool(1).await;
        let observer = pg_pool(1).await;

        exec(&pool, &format!("DROP TABLE IF EXISTS {TABLE}")).await;
        exec(&pool, &format!("CREATE TABLE {TABLE} (v TEXT NOT NULL)")).await;

        let tx = pool.begin().await.expect("begin");
        tx.execute(
            &format!("INSERT INTO {TABLE} (v) VALUES ('abandoned')"),
            &[],
        )
        .await
        .expect("insert");
        drop(tx);

        // Autocommitted if — and only if — the connection came back clean.
        exec(&pool, &format!("INSERT INTO {TABLE} (v) VALUES ('after')")).await;

        assert_eq!(
            committed_count(&observer).await,
            1,
            "the write after an abandoned transaction must be committed and \
             the abandoned one must not be"
        );

        exec(&observer, &format!("DROP TABLE IF EXISTS {TABLE}")).await;
    }

    /// Explicit finish paths must keep working unchanged.
    #[tokio::test]
    async fn commit_and_rollback_still_work() {
        if pg_url().is_none() {
            return;
        }

        let table = format!("{TABLE}_explicit");
        let pool = pg_pool(1).await;
        exec(&pool, &format!("DROP TABLE IF EXISTS {table}")).await;
        exec(&pool, &format!("CREATE TABLE {table} (v TEXT NOT NULL)")).await;

        let tx = pool.begin().await.expect("begin");
        tx.execute(&format!("INSERT INTO {table} (v) VALUES ('kept')"), &[])
            .await
            .expect("insert");
        tx.commit().await.expect("commit");

        let tx = pool.begin().await.expect("begin");
        tx.execute(&format!("INSERT INTO {table} (v) VALUES ('dropped')"), &[])
            .await
            .expect("insert");
        tx.rollback().await.expect("rollback");

        let batch = pool
            .fetch_all_raw(format!("SELECT COUNT(*) FROM {table}").into(), Vec::new())
            .await
            .expect("count");
        let count = decode_rows::<(i64,)>(batch).expect("decode")[0].0;
        assert_eq!(count, 1);

        exec(&pool, &format!("DROP TABLE IF EXISTS {table}")).await;
    }
}

/// The same invariants on the `sqlx`-backed pools.
///
/// These are expected to pass without any work on our side, because
/// `sqlx::Transaction` implements `Drop` itself. That is exactly why they are
/// here: the invariant is a property of `ruprizzle`'s transaction API, not of
/// whichever driver happens to be underneath, and the native drivers only broke
/// it because nothing asserted it anywhere.
mod sqlx_backends {
    use ruprizzle::{Executor, Pool, PoolConfig, connect_with, decode_rows};
    use tempfile::TempDir;

    async fn create_table(pool: &Pool) {
        pool.execute_raw(
            "CREATE TABLE kv (k TEXT PRIMARY KEY, v TEXT NOT NULL)".into(),
            Vec::new(),
        )
        .await
        .expect("create table");
    }

    fn sqlite_url(dir: &TempDir) -> String {
        let path = dir.path().join("tx_lifecycle.sqlite");
        let file = path.to_str().expect("utf-8 path").replace('\\', "/");
        format!("sqlite:///{file}?mode=rwc")
    }

    fn config(max_connections: u32) -> PoolConfig {
        let mut config = PoolConfig::default();
        config.max_connections = max_connections;
        config
    }

    /// `Pool::Sqlite` — the native `sqlx` SQLite backend.
    async fn sqlite_pool() -> (Pool, TempDir) {
        let dir = tempfile::tempdir().expect("tempdir");
        let pool = connect_with(&sqlite_url(&dir), &config(2))
            .await
            .expect("connect");
        assert!(
            pool.as_sqlite().is_some(),
            "expected the sqlx sqlite backend"
        );
        create_table(&pool).await;
        (pool, dir)
    }

    /// `Pool::Any` — the generic driver. `connect_with` routes `sqlite://` to
    /// the native backend, so this one is built directly.
    async fn any_pool() -> (Pool, TempDir) {
        ruprizzle::sqlx::any::install_default_drivers();
        let dir = tempfile::tempdir().expect("tempdir");
        let any = ruprizzle::sqlx::any::AnyPoolOptions::new()
            .max_connections(2)
            .connect(&sqlite_url(&dir))
            .await
            .expect("connect");
        let pool = Pool::Any(any);
        create_table(&pool).await;
        (pool, dir)
    }

    /// `Pool::Postgres` — the native `sqlx` Postgres backend, when configured.
    async fn postgres_pool() -> Option<Pool> {
        let Ok(url) = std::env::var("RUPRIZZLE_TEST_PG_URL") else {
            assert!(
                std::env::var("RUPRIZZLE_REQUIRE_DB").is_err(),
                "RUPRIZZLE_REQUIRE_DB is set but RUPRIZZLE_TEST_PG_URL is not"
            );
            return None;
        };

        let pool = connect_with(&url, &config(2)).await.expect("connect");
        assert!(
            pool.as_postgres().is_some(),
            "expected the sqlx postgres backend"
        );
        pool.execute_raw("DROP TABLE IF EXISTS kv".into(), Vec::new())
            .await
            .expect("drop table");
        create_table(&pool).await;
        Some(pool)
    }

    async fn kv_count(pool: &Pool) -> i64 {
        let batch = pool
            .fetch_all_raw("SELECT COUNT(*) FROM kv".into(), Vec::new())
            .await
            .expect("count");
        decode_rows::<(i64,)>(batch).expect("decode")[0].0
    }

    /// Abandoning a transaction rolls back, releases the connection, and leaves
    /// the pool able to serve `max_connections` transactions again.
    async fn abandoning_a_transaction_is_a_rollback(pool: &Pool) {
        let tx = pool.begin().await.expect("begin");
        tx.execute("INSERT INTO kv (k, v) VALUES ('a', '1')", &[])
            .await
            .expect("insert");
        drop(tx);

        assert_eq!(kv_count(pool).await, 0, "abandoned writes must not persist");

        // Capacity is intact: two concurrent transactions still start.
        let first = pool.begin().await.expect("begin");
        let second = pool.begin().await.expect("begin");
        first.commit().await.expect("commit");
        second.commit().await.expect("commit");
    }

    #[tokio::test]
    async fn sqlite_backend_rolls_back_an_abandoned_transaction() {
        let (pool, _dir) = sqlite_pool().await;
        abandoning_a_transaction_is_a_rollback(&pool).await;
    }

    #[tokio::test]
    async fn any_backend_rolls_back_an_abandoned_transaction() {
        let (pool, _dir) = any_pool().await;
        abandoning_a_transaction_is_a_rollback(&pool).await;
    }

    #[tokio::test]
    async fn postgres_backend_rolls_back_an_abandoned_transaction() {
        let Some(pool) = postgres_pool().await else {
            eprintln!("skipping postgres tx lifecycle test: no RUPRIZZLE_TEST_PG_URL");
            return;
        };
        abandoning_a_transaction_is_a_rollback(&pool).await;
        pool.execute_raw("DROP TABLE IF EXISTS kv".into(), Vec::new())
            .await
            .expect("drop table");
    }
}
