//! Regression tests for BUG-05: empty insert rows must error, not panic.

use ruprizzle::{
    Column, Encodable, Executor, InsertManyQuery, InsertQuery, Model, NestedSetter, Pool, Related,
    connect,
};
use sqlx::FromRow;

#[derive(Debug, Clone, Default, FromRow)]
#[allow(dead_code)]
struct Task {
    id: i64,
    name: String,
    #[sqlx(skip)]
    children: Related<Vec<Task>>,
}

impl Model for Task {
    const TABLE: &'static str = "tasks";
}

const NAME: Column<Task, String> = Column::new("tasks", "name");

struct SetChildren;
impl NestedSetter<Task> for SetChildren {
    fn set(&self, parent: &mut Task, batch: ruprizzle::executor::RowBatch) {
        parent.children = Related::Loaded(
            ruprizzle::executor::decode_rows::<Task>(batch).unwrap_or_default(),
        );
    }
}

async fn fresh_pool() -> Pool {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("test.sqlite");
    let file = path.to_str().unwrap().replace('\\', "/");
    let url = format!("sqlite:///{}?mode=rwc", file);
    let pool = connect(&url).await.unwrap();

    pool.execute_raw(
        "CREATE TABLE tasks (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL,
            parent_id INTEGER
        )"
        .to_string()
        .into(),
        Vec::new(),
    )
    .await
    .unwrap();

    pool
}

#[tokio::test]
async fn insert_many_empty_row_errors() {
    let pool = fresh_pool().await;
    let err = InsertManyQuery::<Task>::new(&pool)
        .row([])
        .exec()
        .await
        .unwrap_err();

    let msg = format!("{err}");
    assert!(
        msg.contains("no columns"),
        "expected a clear error for an empty insert row, got: {msg}"
    );
}

#[tokio::test]
async fn insert_query_with_related_empty_child_row_errors() {
    let pool = fresh_pool().await;
    let err = InsertQuery::<Task>::new(&pool)
        .set(NAME, "parent")
        .with_related(
            |t| t.id.to_value(),
            "parent_id",
            InsertManyQuery::<Task>::new(&pool).row([]),
            SetChildren,
        )
        .exec()
        .await
        .unwrap_err();

    let msg = format!("{err}");
    assert!(
        msg.contains("no columns"),
        "expected a clear error for an empty child insert row, got: {msg}"
    );
}
