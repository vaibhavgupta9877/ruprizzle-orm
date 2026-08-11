//! Concurrency and contention tests against a local SQLite file.
//!
//! SQLite is single-writer, but the pool and transaction paths must still behave
//! correctly when many async tasks arrive at once.

use ruprizzle::{Column, Executor, InsertManyQuery, InsertQuery, Model, SelectQuery, Value};
use ruprizzle_deep_tests::fresh_pool;

#[derive(Debug, Clone, sqlx::FromRow)]
#[allow(dead_code)]
struct Task {
    id: i64,
    label: String,
}

impl Model for Task {
    const TABLE: &'static str = "tasks";
}

const ID: Column<Task, i64> = Column::new("tasks", "id");
const LABEL: Column<Task, String> = Column::new("tasks", "label");

#[tokio::test]
async fn many_concurrent_inserts_succeed() {
    let (pool, _tmp) = fresh_pool().await;

    pool.execute_raw(
        "CREATE TABLE tasks (id INTEGER PRIMARY KEY, label TEXT NOT NULL)".to_string().into(),
        Vec::new(),
    )
    .await
    .unwrap();

    let mut handles = Vec::new();
    for t in 0..8 {
        let p = pool.clone();
        handles.push(tokio::spawn(async move {
            let mut q = InsertManyQuery::<Task>::new(&p);
            for i in 0..4 {
                q = q.row([
                    ("id", Value::I64(t as i64 * 100 + i)),
                    ("label", Value::Str(format!("task-{t}-{i}").into())),
                ]);
            }
            q.exec().await.unwrap();
        }));
    }

    for h in handles {
        h.await.unwrap();
    }

    assert_eq!(SelectQuery::<Task>::new(&pool).count().await.unwrap(), 32);
}

#[tokio::test]
async fn transactions_see_their_own_writes_before_commit() {
    let (pool, _tmp) = fresh_pool().await;

    pool.execute_raw(
        "CREATE TABLE tasks (id INTEGER PRIMARY KEY, label TEXT NOT NULL)".to_string().into(),
        Vec::new(),
    )
    .await
    .unwrap();

    InsertQuery::<Task>::new(&pool)
        .set(ID, 1)
        .set(LABEL, "visible")
        .exec()
        .await
        .unwrap();

    let tx = ruprizzle::Tx::begin(&pool).await.unwrap();

    tx.execute(
        "INSERT INTO tasks (id, label) VALUES (?, ?)",
        &vec![Value::I64(2), Value::Str("in-tx".into())],
    )
    .await
    .unwrap();

    // The transaction should see its own uncommitted write.
    let count: (i64,) = tx
        .fetch_one("SELECT count(*) FROM tasks", &[])
        .await
        .unwrap();
    assert_eq!(count.0, 2);

    // The pool should not see the uncommitted write.
    assert_eq!(SelectQuery::<Task>::new(&pool).count().await.unwrap(), 1);

    tx.rollback().await.unwrap();

    // After rollback the write is gone from the pool too.
    assert_eq!(SelectQuery::<Task>::new(&pool).count().await.unwrap(), 1);
}
