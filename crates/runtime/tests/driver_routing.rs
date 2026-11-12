//! `driver=` routing parameter handling in `connect_with`.
//!
//! The `driver` query parameter selects a native driver. Its handling must not
//! depend on whether the matching Cargo feature is enabled:
//!
//! | feature enabled | parameter               | expected                          |
//! |-----------------|-------------------------|-----------------------------------|
//! | yes             | `driver=rusqlite`       | native `rusqlite` pool            |
//! | no              | `driver=rusqlite`       | error naming `sqlite-rusqlite`    |
//! | either          | absent                  | sqlx pool                         |
//! | either          | unknown value (typo)    | error listing the valid values    |
//!
//! Regression tests for BUG-10 and FIX-10.

use ruprizzle::{Error, PoolConfig, connect_with};

fn message(err: Error) -> String {
    match err {
        Error::Message(m) => m,
        other => other.to_string(),
    }
}

/// Connects with default settings and returns the error message, failing the
/// test if the URL is accepted.
async fn rejected(url: &str) -> String {
    match connect_with(url, &PoolConfig::default()).await {
        Ok(_) => panic!("expected `{url}` to be rejected"),
        Err(err) => message(err),
    }
}

#[tokio::test]
async fn misspelled_sqlite_driver_is_rejected_in_every_build() {
    let msg = rejected("sqlite::memory:?driver=rusqlit").await;
    assert!(
        msg.contains("rusqlit"),
        "message should name the value: {msg}"
    );
    assert!(
        msg.contains("rusqlite"),
        "message should list the valid value: {msg}"
    );
}

#[tokio::test]
async fn driver_value_must_match_exactly_not_by_prefix() {
    // `driver=rusqlite2` contains `driver=rusqlite` as a substring; it must be
    // treated as an unknown value, not routed to the native driver.
    let msg = rejected("sqlite::memory:?driver=rusqlite2").await;
    assert!(msg.contains("rusqlite2"), "{msg}");
}

#[tokio::test]
async fn misspelled_postgres_driver_is_rejected_in_every_build() {
    // Rejected before any network I/O, so the unreachable host is never dialled.
    let msg = rejected("postgres://127.0.0.1:1/nope?driver=tokio-postgress").await;
    assert!(msg.contains("tokio-postgress"), "{msg}");
    assert!(msg.contains("tokio-postgres"), "{msg}");
}

#[tokio::test]
async fn sqlite_without_driver_uses_sqlx() {
    let pool = connect_with("sqlite::memory:", &PoolConfig::default())
        .await
        .expect("connect");
    assert!(
        matches!(pool, ruprizzle::Pool::Sqlite(_)),
        "expected the sqlx sqlite pool"
    );
}

#[cfg(not(feature = "sqlite-rusqlite"))]
#[tokio::test]
async fn rusqlite_driver_without_feature_names_the_cargo_feature() {
    let msg = rejected("sqlite::memory:?driver=rusqlite").await;
    assert!(
        msg.contains("sqlite-rusqlite"),
        "message should name the Cargo feature: {msg}"
    );
    assert!(
        !msg.contains("unknown query parameter"),
        "must not surface the opaque sqlx error: {msg}"
    );
}

#[cfg(feature = "sqlite-rusqlite")]
#[tokio::test]
async fn rusqlite_driver_with_feature_routes_to_native_pool() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("driver_routing.sqlite");
    let file = path.to_str().expect("utf-8 path").replace('\\', "/");
    let url = format!("sqlite:///{file}?mode=rwc&driver=rusqlite");

    let pool = connect_with(&url, &PoolConfig::default())
        .await
        .expect("connect");
    assert!(
        pool.as_rusqlite().is_some(),
        "expected native rusqlite pool"
    );
}
