//! Savepoint and nested transaction tests.
//!
//! These run through the dual-database `both_dbs!` macro, which gives every
//! case coverage on the default `sqlx::Any` driver for both SQLite and
//! PostgreSQL. The native `rusqlite` and `tokio-postgres` paths are exercised
//! separately in `tx_lifecycle.rs`, because savepoints are implemented purely
//! as SQL `SAVEPOINT / RELEASE SAVEPOINT / ROLLBACK TO SAVEPOINT` commands and
//! therefore ride the same execution path on every backend.

use ruprizzle::Error;
use ruprizzle::executor::Executor;
use ruprizzle_testkit::both_dbs;

both_dbs! {
    setup = "CREATE TABLE kv (k TEXT PRIMARY KEY, v TEXT NOT NULL)";
    async fn nested_commit_and_release(db: TestDb) {
        let tx = db.pool().begin().await?;

        tx.execute("INSERT INTO kv (k, v) VALUES ('a', '1')", &[]).await?;

        let sp1 = tx.savepoint().await?;
        tx.execute("INSERT INTO kv (k, v) VALUES ('b', '2')", &[]).await?;

        let sp2 = sp1.savepoint().await?;
        tx.execute("INSERT INTO kv (k, v) VALUES ('c', '3')", &[]).await?;

        sp2.release().await?;
        sp1.release().await?;
        tx.commit().await?;

        assert_eq!(db.fetch_i64("SELECT COUNT(*) FROM kv").await?, 3);
    }
}

both_dbs! {
    setup = "CREATE TABLE kv (k TEXT PRIMARY KEY, v TEXT NOT NULL)";
    async fn nested_rollback_leaves_outer_live(db: TestDb) {
        let tx = db.pool().begin().await?;

        tx.execute("INSERT INTO kv (k, v) VALUES ('a', '1')", &[]).await?;

        let sp1 = tx.savepoint().await?;
        tx.execute("INSERT INTO kv (k, v) VALUES ('b', '2')", &[]).await?;

        let sp2 = sp1.savepoint().await?;
        tx.execute("INSERT INTO kv (k, v) VALUES ('c', '3')", &[]).await?;

        // Roll back the inner savepoint: 'c' is gone, but 'a' and 'b' remain.
        sp2.rollback().await?;

        tx.execute("INSERT INTO kv (k, v) VALUES ('d', '4')", &[]).await?;

        sp1.release().await?;
        tx.commit().await?;

        assert_eq!(db.fetch_i64("SELECT COUNT(*) FROM kv").await?, 3);
    }
}

both_dbs! {
    setup = "CREATE TABLE kv (k TEXT PRIMARY KEY, v TEXT NOT NULL)";
    async fn dropped_savepoint_rolls_back(db: TestDb) {
        let tx = db.pool().begin().await?;

        tx.execute("INSERT INTO kv (k, v) VALUES ('a', '1')", &[]).await?;

        {
            let _sp = tx.savepoint().await?;
            tx.execute("INSERT INTO kv (k, v) VALUES ('b', '2')", &[])
                .await?;
            // _sp is dropped here without release or rollback.
        }

        // The next operation on the transaction flushes the deferred rollback
        // for the dropped savepoint before executing.
        tx.execute("INSERT INTO kv (k, v) VALUES ('c', '3')", &[]).await?;
        tx.commit().await?;

        assert_eq!(db.fetch_i64("SELECT COUNT(*) FROM kv").await?, 2);
    }
}

both_dbs! {
    setup = "CREATE TABLE kv (k TEXT PRIMARY KEY, v TEXT NOT NULL)";
    async fn depth_three(db: TestDb) {
        let tx = db.pool().begin().await?;

        let sp1 = tx.savepoint().await?;
        let sp2 = sp1.savepoint().await?;
        let sp3 = sp2.savepoint().await?;

        tx.execute("INSERT INTO kv (k, v) VALUES ('x', '1')", &[]).await?;

        sp3.release().await?;
        sp2.release().await?;
        sp1.release().await?;
        tx.commit().await?;

        assert_eq!(db.fetch_i64("SELECT COUNT(*) FROM kv").await?, 1);
    }
}

both_dbs! {
    setup = "CREATE TABLE kv (k TEXT PRIMARY KEY, v TEXT NOT NULL)";
    async fn rollback_after_constraint_violation(db: TestDb) {
        let tx = db.pool().begin().await?;

        tx.execute("INSERT INTO kv (k, v) VALUES ('a', '1')", &[]).await?;

        let sp = tx.savepoint().await?;
        let result = tx
            .execute("INSERT INTO kv (k, v) VALUES ('a', '2')", &[])
            .await;
        assert!(result.is_err(), "duplicate key insert should fail");

        // Rolling back to the savepoint undoes the failed insert and lets the
        // transaction continue (on Postgres this is the standard way to recover
        // from a statement-level error).
        sp.rollback().await?;

        tx.execute("INSERT INTO kv (k, v) VALUES ('b', '2')", &[]).await?;
        tx.commit().await?;

        assert_eq!(db.fetch_i64("SELECT COUNT(*) FROM kv").await?, 2);
        assert_eq!(db.fetch_string("SELECT v FROM kv WHERE k = 'a'").await?, "1");
    }
}

both_dbs! {
    setup = "CREATE TABLE kv (k TEXT PRIMARY KEY, v TEXT NOT NULL)";
    async fn transaction_closure_commits(db: TestDb) {
        let tx = db.pool().begin().await?;

        tx.transaction(|sp| {
            Box::pin(async move {
                sp.execute_raw(
                    "INSERT INTO kv (k, v) VALUES ('a', '1')".into(),
                    Vec::new(),
                )
                .await?;
                Ok::<(), Error>(())
            })
        })
        .await?;

        tx.commit().await?;

        assert_eq!(db.fetch_i64("SELECT COUNT(*) FROM kv").await?, 1);
    }
}

both_dbs! {
    setup = "CREATE TABLE kv (k TEXT PRIMARY KEY, v TEXT NOT NULL)";
    async fn transaction_closure_rolls_back(db: TestDb) {
        let tx = db.pool().begin().await?;

        let result: Result<(), Error> = tx
            .transaction(|sp| {
                Box::pin(async move {
                    sp.execute_raw(
                        "INSERT INTO kv (k, v) VALUES ('a', '1')".into(),
                        Vec::new(),
                    )
                    .await?;
                    Err::<(), Error>(Error::Message("deliberate".into()))
                })
            })
            .await;

        assert!(result.is_err());
        tx.commit().await?;

        assert_eq!(db.fetch_i64("SELECT COUNT(*) FROM kv").await?, 0);
    }
}
