//! Transaction lifecycle invariants for every backend.
//!
//! The rule under test is the same one `sqlx::Transaction` enforces for its own
//! backends: a transaction that is dropped without an explicit `commit()` or
//! `rollback()` must roll back and return its connection to the pool. The
//! hand-written native drivers each implement that themselves, so the checks
//! here are the regression tests for BUG-01, BUG-02 and BUG-03.

#![cfg(feature = "sqlite-rusqlite")]

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
