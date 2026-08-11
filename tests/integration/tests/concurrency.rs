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
