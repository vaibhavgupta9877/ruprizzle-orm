//! P0-04 acceptance: the dual-database harness works.
//!
//! These tests do not exercise the ORM — none of it exists yet. They exercise
//! the *harness*, which every later phase depends on: isolation, teardown,
//! foreign-key enforcement, and the skip policy.

use ruprizzle_integration_tests::SMOKE_DDL;
use ruprizzle_testkit::both_dbs;

both_dbs! {
    setup = SMOKE_DDL;
    /// The base case: setup SQL ran, and writes are visible.
    async fn insert_then_select(db: TestDb) {
        db.execute("INSERT INTO widget (id, name, price) VALUES (1, 'bolt', 250)").await?;
        db.execute("INSERT INTO widget (id, name, price) VALUES (2, 'nut', 100)").await?;

        assert_eq!(db.fetch_i64("SELECT count(*) FROM widget").await?, 2);
        assert_eq!(
            db.fetch_string("SELECT name FROM widget WHERE id = 1").await?,
            "bolt"
        );
    }
}

both_dbs! {
    setup = SMOKE_DDL;
    /// Each test gets its own database.
    ///
    /// If isolation were broken, the rows written by `insert_then_select` above
    /// would be visible here and this count would not be zero. Concurrent test
    /// execution makes that failure intermittent rather than absent, which is
    /// why it is asserted explicitly.
    async fn starts_empty(db: TestDb) {
        assert_eq!(db.fetch_i64("SELECT count(*) FROM widget").await?, 0);
    }
}

both_dbs! {
    setup = SMOKE_DDL;
    /// Foreign keys are enforced on both backends.
    ///
    /// SQLite leaves `PRAGMA foreign_keys` off by default, so without the
    /// harness setting it every referential-integrity test from P2 onward would
    /// pass vacuously on SQLite while genuinely testing Postgres.
    async fn foreign_keys_are_enforced(db: TestDb) {
        db.execute("INSERT INTO widget (id, name, price) VALUES (1, 'bolt', 250)").await?;

        let orphan = db
            .execute("INSERT INTO widget_part (id, widget_id, label) VALUES (1, 999, 'x')")
            .await;
        assert!(
            orphan.is_err(),
            "inserting a row referencing a missing parent must fail on {}",
            db.backend()
        );

        db.execute("INSERT INTO widget_part (id, widget_id, label) VALUES (1, 1, 'head')").await?;
        db.execute("DELETE FROM widget WHERE id = 1").await?;
        assert_eq!(
            db.fetch_i64("SELECT count(*) FROM widget_part").await?,
            0,
            "ON DELETE CASCADE must remove children on {}",
            db.backend()
        );
    }
}

both_dbs! {
    /// A case with no setup SQL still gets a usable connection.
    async fn works_without_setup(db: TestDb) {
        assert_eq!(db.fetch_i64("SELECT 1").await?, 1);
    }
}

both_dbs! {
    setup = SMOKE_DDL;
    /// The backend is reported accurately, and the matching pool is the one present.
    ///
    /// Tests that need driver-specific behaviour branch on this, so a wrong
    /// answer here would silently send a test down the wrong path.
    async fn backend_and_pool_agree(db: TestDb) {
        match db.backend() {
            ruprizzle_testkit::Backend::Postgres => {
                assert!(db.pg_pool().is_some());
                assert!(db.sqlite_pool().is_none());
            }
            ruprizzle_testkit::Backend::Sqlite => {
                assert!(db.sqlite_pool().is_some());
                assert!(db.pg_pool().is_none());
            }
            ruprizzle_testkit::Backend::MySql => {
                assert!(db.sqlite_pool().is_none());
                assert!(db.pg_pool().is_none());
                assert!(db.pool().as_mysql().is_some());
            }
        }
    }
}
