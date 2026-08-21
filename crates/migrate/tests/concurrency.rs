//! Concurrent `apply_all` must be idempotent under genuine interleaving.

use std::borrow::Cow;
use std::fs;

use ruprizzle::Executor;
use ruprizzle_migrate::Migrator;
use ruprizzle_testkit::IsolatedSchema;

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

    // A private schema, rather than `public` with a "drop what we know about"
    // clean slate. The old form left `conc_a`/`conc_b` behind for every other
    // DB-backed test in the workspace to trip over.
    let schema = IsolatedSchema::create(&url)
        .await
        .expect("create isolated schema");
    let pool = ruprizzle::connect(schema.url()).await.expect("connect");

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

    pool.execute_raw(Cow::Owned("SELECT 1 FROM conc_a".into()), Vec::new())
        .await
        .expect("conc_a exists");
    pool.execute_raw(Cow::Owned("SELECT 1 FROM conc_b".into()), Vec::new())
        .await
        .expect("conc_b exists");

    pool.close().await;
    schema.drop_now().await.expect("drop isolated schema");
}
