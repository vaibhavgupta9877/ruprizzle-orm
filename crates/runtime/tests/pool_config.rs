//! Pool configuration preserves sqlx defaults and accepts overrides.

use std::time::Duration;

use ruprizzle::{PoolConfig, connect_with};

#[test]
fn defaults_match_sqlx() {
    let config = PoolConfig::default();
    assert_eq!(config.max_connections, 10);
    assert_eq!(config.min_connections, 0);
    assert_eq!(config.acquire_timeout, Duration::from_secs(30));
    assert_eq!(config.idle_timeout, Some(Duration::from_secs(600)));
    assert_eq!(config.max_lifetime, Some(Duration::from_secs(1800)));
    assert!(config.test_before_acquire);
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
    let options = pool.options();
    assert_eq!(options.get_max_connections(), 3);
    assert_eq!(options.get_min_connections(), 1);
    assert_eq!(options.get_acquire_timeout(), Duration::from_secs(1));
    assert_eq!(options.get_idle_timeout(), Some(Duration::from_secs(10)));
    assert_eq!(options.get_max_lifetime(), Some(Duration::from_secs(20)));
    assert!(!options.get_test_before_acquire());
}
