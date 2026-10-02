//! Live smoke test against a real Turso (libSQL) database.
//!
//! Skipped unless `TURSO_DATABASE_URL` and `TURSO_AUTH_TOKEN` are set. It creates
//! a uniquely named table, round-trips rows through it and drops it:
//!
//! ```text
//! TURSO_DATABASE_URL=libsql://<db>.turso.io TURSO_AUTH_TOKEN=... \
//!     cargo test -p ruprizzle-turso --test live
//! ```

use std::borrow::Cow;

use futures_util::{FutureExt as _, StreamExt as _};
use ruprizzle::Executor;
use ruprizzle::executor::RowBatch;
use ruprizzle::value::Value;
use ruprizzle_turso::TursoPool;

/// The live pool, or `None` (the test then skips) when the credentials are unset.
fn live_pool() -> Option<TursoPool> {
    let url = std::env::var("TURSO_DATABASE_URL").ok()?;
    let token = std::env::var("TURSO_AUTH_TOKEN").ok()?;
    Some(
        TursoPool::builder()
            .url(url)
            .auth_token(token)
            .build()
            .expect("the live pool builds"),
    )
}

fn table_name() -> String {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock after epoch")
        .as_nanos();
    format!("ruprizzle_live_{nanos}")
}

#[tokio::test]
async fn round_trips_against_the_live_service() {
    let Some(pool) = live_pool() else {
        eprintln!("skipped: live credentials not set");
        return;
    };
    let t = table_name();
    let run = |sql: String, binds: Vec<Value>| pool.execute_raw(Cow::Owned(sql), binds);

    run(
        format!("CREATE TABLE {t} (id INTEGER PRIMARY KEY, name TEXT, score REAL, note TEXT)"),
        vec![],
    )
    .await
    .expect("create table");

    let outcome = std::panic::AssertUnwindSafe(async {
        let inserted = run(
            format!("INSERT INTO {t} (id, name, score, note) VALUES (?, ?, ?, ?), (?, ?, ?, ?)"),
            vec![
                Value::I64(1),
                Value::Str("alice".into()),
                Value::F64(1.5),
                Value::Null,
                Value::I64(2),
                Value::Str("bob".into()),
                Value::F64(-2.25),
                Value::Str("x".into()),
            ],
        )
        .await
        .expect("insert");
        assert_eq!(inserted, 2, "the service reports the inserted row count");

        let batch = pool
            .fetch_all_raw(
                Cow::Owned(format!(
                    "SELECT id, name, score, note FROM {t} WHERE id >= ? ORDER BY id"
                )),
                vec![Value::I64(1)],
            )
            .await
            .expect("select");
        let RowBatch::Edge(rows) = batch else {
            panic!("edge adapters produce edge rows");
        };
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].get("id"), Some(&Value::I64(1)));
        assert_eq!(rows[0].get("name"), Some(&Value::Str("alice".into())));
        assert_eq!(rows[0].get("score"), Some(&Value::F64(1.5)));
        assert_eq!(rows[0].get("note"), Some(&Value::Null));
        assert_eq!(rows[1].get("score"), Some(&Value::F64(-2.25)));

        let updated = run(
            format!("UPDATE {t} SET note = ? WHERE id = ?"),
            vec![Value::Str("y".into()), Value::I64(2)],
        )
        .await
        .expect("update");
        assert_eq!(updated, 1);

        let streamed: Vec<_> = pool
            .stream_raw(
                Cow::Owned(format!("SELECT id FROM {t} ORDER BY id")),
                vec![],
            )
            .collect()
            .await;
        assert_eq!(streamed.len(), 2);
        assert!(streamed.iter().all(Result::is_ok));

        let err = pool
            .fetch_all_raw(
                Cow::Owned(format!("SELECT no_such_column FROM {t}")),
                vec![],
            )
            .await;
        assert!(err.is_err(), "a failing statement is reported as an error");
    })
    .catch_unwind()
    .await;

    // Drop the table even when an assertion above failed, then re-raise it.
    run(format!("DROP TABLE {t}"), vec![])
        .await
        .expect("drop table");
    if let Err(panic) = outcome {
        std::panic::resume_unwind(panic);
    }
}
