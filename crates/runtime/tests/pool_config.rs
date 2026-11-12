//! Pool configuration preserves sqlx defaults and accepts overrides.

use std::time::Duration;

use ruprizzle::{PoolConfig, connect_with};

#[test]
fn defaults_match_ruprizzle() {
    let config = PoolConfig::default();
    assert_eq!(config.max_connections, 10);
    assert_eq!(config.min_connections, 0);
    assert_eq!(config.acquire_timeout, Duration::from_secs(30));
    assert_eq!(config.idle_timeout, Some(Duration::from_secs(600)));
    assert_eq!(config.max_lifetime, Some(Duration::from_secs(1800)));
    assert!(!config.test_before_acquire);
    assert!(!config.reset_on_recycle);
    assert_eq!(config.row_buffer_size, 1024);
}

#[tokio::test]
async fn configured_pool_connects() {
    let mut config = PoolConfig::default();
    config.max_connections = 3;
    config.min_connections = 1;
    config.acquire_timeout = Duration::from_secs(1);
    config.idle_timeout = Some(Duration::from_secs(10));
    config.max_lifetime = Some(Duration::from_secs(20));
    config.test_before_acquire = false;

    let pool = connect_with("sqlite::memory:", &config)
        .await
        .expect("connect");
    let options = pool.sqlite_options().expect("sqlite pool options");
    assert_eq!(options.get_max_connections(), 3);
    assert_eq!(options.get_min_connections(), 1);
    assert_eq!(options.get_acquire_timeout(), Duration::from_secs(1));
    assert_eq!(options.get_idle_timeout(), Some(Duration::from_secs(10)));
    assert_eq!(options.get_max_lifetime(), Some(Duration::from_secs(20)));
    assert!(!options.get_test_before_acquire());
}

#[tokio::test]
async fn stats_and_ping_report_a_live_pool() {
    let pool = ruprizzle::connect("sqlite::memory:")
        .await
        .expect("connect");
    ruprizzle::ping(&pool).await.expect("ping");

    let tx = pool.begin().await.expect("begin");
    let busy = ruprizzle::stats(&pool);
    assert!(busy.in_use >= 1);
    tx.rollback().await.expect("rollback");

    let idle = ruprizzle::stats(&pool);
    assert!(idle.idle >= 1);
    assert_eq!(idle.in_use + idle.idle, idle.size as usize);
}

#[tokio::test]
async fn ping_reports_an_unreachable_database() {
    ruprizzle::sqlx::any::install_default_drivers();
    let any = ruprizzle::sqlx::any::AnyPoolOptions::new()
        .acquire_timeout(Duration::from_millis(200))
        .connect_lazy("postgres://127.0.0.1:1/nope")
        .expect("lazy pool");
    let pool = ruprizzle::Pool::Any(any);
    assert!(ruprizzle::ping(&pool).await.is_err());
}

/// `PoolStats` must be internally consistent for every pool variant that can be
/// built without an external database (FIX-07, step 4).
fn assert_consistent(stats: ruprizzle::PoolStats) {
    assert_eq!(
        stats.in_use + stats.idle,
        stats.size as usize,
        "in_use + idle must equal size: {stats:?}"
    );
}

#[tokio::test]
async fn stats_cover_the_sqlx_sqlite_pool() {
    let mut config = PoolConfig::default();
    config.max_connections = 3;
    let pool = connect_with("sqlite::memory:", &config)
        .await
        .expect("connect");
    ruprizzle::ping(&pool).await.expect("ping");
    let s = ruprizzle::stats(&pool);
    assert!(s.size > 0, "sqlx pool should report its size: {s:?}");
    assert_consistent(s);
}

#[tokio::test]
async fn stats_cover_the_any_pool() {
    ruprizzle::sqlx::any::install_default_drivers();
    let any = ruprizzle::sqlx::any::AnyPoolOptions::new()
        .max_connections(2)
        .connect_lazy("sqlite::memory:")
        .expect("lazy pool");
    let pool = ruprizzle::Pool::Any(any);
    assert_consistent(ruprizzle::stats(&pool));
}

#[cfg(feature = "sqlite-rusqlite")]
mod rusqlite_stats {
    use super::*;

    async fn rusqlite_pool(max_connections: u32) -> (ruprizzle::Pool, tempfile::TempDir) {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("pool_stats.sqlite");
        let file = path.to_str().expect("utf-8 path").replace('\\', "/");
        let url = format!("sqlite:///{file}?mode=rwc&driver=rusqlite");
        let mut config = PoolConfig::default();
        config.max_connections = max_connections;
        config.acquire_timeout = Duration::from_secs(2);
        let pool = connect_with(&url, &config).await.expect("connect");
        (pool, dir)
    }

    #[tokio::test]
    async fn stats_report_a_live_rusqlite_pool() {
        let (pool, _dir) = rusqlite_pool(2).await;
        let s = ruprizzle::stats(&pool);
        assert_eq!(s.size, 2, "size should be the configured capacity: {s:?}");
        assert_eq!(s.idle, 2, "{s:?}");
        assert_eq!(s.in_use, 0, "{s:?}");
        assert_consistent(s);
    }

    #[tokio::test]
    async fn checked_out_transactions_count_as_in_use() {
        let (pool, _dir) = rusqlite_pool(2).await;
        let tx = pool.begin().await.expect("begin");
        let s = ruprizzle::stats(&pool);
        assert_eq!(s.in_use, 1, "{s:?}");
        assert_eq!(s.idle, 1, "{s:?}");
        assert_consistent(s);
        tx.rollback().await.expect("rollback");
    }

    /// FIX-01 regression expressed as a metric: abandoned transactions must
    /// return their connections, so `idle` climbs back to capacity.
    #[tokio::test]
    async fn abandoned_transactions_return_to_full_idle() {
        let (pool, _dir) = rusqlite_pool(2).await;
        for _ in 0..50 {
            let tx = pool.begin().await.expect("begin");
            drop(tx);
        }
        let s = ruprizzle::stats(&pool);
        assert_eq!(s.idle, 2, "idle must return to capacity: {s:?}");
        assert_eq!(s.in_use, 0, "{s:?}");
    }
}
