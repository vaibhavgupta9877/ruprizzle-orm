//! Concurrent `apply_all` must be idempotent under genuine interleaving.

use std::fs;
use std::time::Duration;

use ruprizzle::{PoolConfig, connect_with};
use ruprizzle_migrate::Migrator;

/// Writes a two-migration directory into a temp dir and returns its path.
fn fixture() -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("tempdir");
    for (id, sql) in [
        (
            "20260101000000_first",
            "CREATE TABLE conc_a (id INTEGER PRIMARY KEY);",
        ),
        (
            "20260101000001_second",
            "CREATE TABLE conc_b (id INTEGER PRIMARY KEY);",
        ),
    ] {
        let m = dir.path().join(id);
        fs::create_dir_all(&m).expect("create migration dir");
        fs::write(m.join("up.sql"), sql).expect("write up.sql");
        fs::write(m.join("down.sql"), "").expect("write down.sql");
    }
    dir
}

/// Ten deployers racing on the same directory: exactly one applies each
/// migration, none error, and the schema ends up correct.
#[tokio::test]
async fn ten_concurrent_deployers_all_succeed() {
    let Some(url) = std::env::var("RUPRIZZLE_TEST_PG_URL").ok() else {
        if std::env::var("RUPRIZZLE_REQUIRE_DB").is_ok() {
            panic!("RUPRIZZLE_REQUIRE_DB is set but RUPRIZZLE_TEST_PG_URL is not");
        }
        eprintln!(
            "skipping ten_concurrent_deployers_all_succeed: RUPRIZZLE_TEST_PG_URL is not set"
        );
        return;
    };

    let dir = fixture();
    let mut config = PoolConfig::default();
    config.max_connections = 20;
    config.acquire_timeout = Duration::from_secs(5);
    let pool = connect_with(&url, &config).await.expect("connect");

    sqlx::query("DROP TABLE IF EXISTS conc_a, conc_b, _ruprizzle_migrations")
        .execute(&pool)
        .await
        .expect("clean slate");

    let mut handles = Vec::new();
    for _ in 0..10 {
        let path = dir.path().to_path_buf();
        let pool = pool.clone();
        handles.push(tokio::spawn(async move {
            Migrator::new(path).apply_all(&pool, false).await
        }));
    }

    let mut total_applied = 0;
    for h in handles {
        let report = h
            .await
            .expect("task panicked")
            .expect("apply must not error");
        total_applied += report.applied.len();
    }

    assert_eq!(
        total_applied, 2,
        "each migration must be applied exactly once across all deployers"
    );

    sqlx::query("SELECT 1 FROM conc_a")
        .execute(&pool)
        .await
        .expect("conc_a exists");
    sqlx::query("SELECT 1 FROM conc_b")
        .execute(&pool)
        .await
        .expect("conc_b exists");
}
