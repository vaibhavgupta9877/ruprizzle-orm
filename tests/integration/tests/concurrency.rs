//! Concurrent `apply_all` must be idempotent, not merely serialised.

use std::fs;

use ruprizzle_migrate::Migrator;
use ruprizzle_testkit::both_dbs;

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

both_dbs! {
    async fn apply_all_twice_is_idempotent(db: TestDb) {
        let dir = fixture();
        let migrator = Migrator::new(dir.path());

        let first = migrator.apply_all(db.any_pool(), false).await?;
        assert_eq!(first.applied.len(), 2);

        // The second run models a concurrent deployer that computed the same
        // pending set before the first one committed.
        let second = migrator.apply_all(db.any_pool(), false).await?;
        assert!(
            second.applied.is_empty(),
            "second run should be a no-op, applied {:?}",
            second.applied
        );
    }
}

/// Ten deployers racing on the same directory: exactly one applies each
/// migration, none error, and the schema ends up correct.
///
/// This is the only test that reaches the re-check inside the advisory lock.
/// The sequential case above cannot: once `apply_all` returns, the tracking
/// table holds the record, so the outer pending filter excludes it before the
/// lock is ever taken. Postgres only, because the advisory lock is Postgres
/// only.
#[tokio::test]
async fn ten_concurrent_deployers_all_succeed() {
    let Ok(url) = std::env::var("RUPRIZZLE_TEST_PG_URL") else {
        assert!(
            std::env::var("RUPRIZZLE_REQUIRE_DB").is_err(),
            "RUPRIZZLE_REQUIRE_DB is set but RUPRIZZLE_TEST_PG_URL is not"
        );
        eprintln!("skipping: no RUPRIZZLE_TEST_PG_URL");
        return;
    };

    let dir = fixture();
    let pool = ruprizzle::connect(&url).await.expect("connect");

    for stmt in [
        "DROP TABLE IF EXISTS conc_a",
        "DROP TABLE IF EXISTS conc_b",
        "DROP TABLE IF EXISTS _ruprizzle_migrations",
    ] {
        sqlx::query(stmt).execute(&pool).await.expect("clean slate");
    }

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
}
