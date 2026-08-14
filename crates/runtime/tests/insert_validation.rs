//! Regression tests for BUG-05: empty insert rows must error, not panic.

use ruprizzle::{
    Column, Encodable, Executor, InsertManyQuery, InsertQuery, Model, NestedSetter, Pool, Related,
    Value, connect,
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

#[cfg(feature = "postgres-tokio-postgres")]
ruprizzle::tokio_postgres_default_row!(Task);

#[cfg(feature = "sqlite-rusqlite")]
impl ruprizzle::rusqlite::FromRusqliteRow for Task {
    fn from_rusqlite_row(_: &ruprizzle::rusqlite::RusqliteRow) -> Result<Self, ruprizzle::Error> {
        Ok(Task::default())
    }
}

#[cfg(feature = "sqlite-rusqlite")]
impl ruprizzle::rusqlite::FromOwnedRow for Task {
    fn from_owned_row(_: &ruprizzle::rusqlite::Row) -> Result<Self, ruprizzle::Error> {
        Ok(Task::default())
    }
}

const NAME: Column<Task, String> = Column::new("tasks", "name");

struct SetChildren;
impl NestedSetter<Task> for SetChildren {
    fn set(&self, parent: &mut Task, batch: ruprizzle::executor::RowBatch) {
        parent.children =
            Related::Loaded(ruprizzle::executor::decode_rows::<Task>(batch).unwrap_or_default());
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

#[tokio::test]
async fn insert_many_heterogeneous_rows_errors() {
    let pool = fresh_pool().await;
    let err = InsertManyQuery::<Task>::new(&pool)
        .row([("name", Value::Str("first".into()))])
        .row([
            ("name", Value::Str("second".into())),
            ("parent_id", Value::I64(1)),
        ])
        .exec()
        .await
        .unwrap_err();

    let msg = format!("{err}");
    assert!(
        msg.contains("row 1"),
        "expected error to name the offending row, got: {msg}"
    );
}

#[tokio::test]
async fn insert_many_wrong_column_order_errors() {
    let pool = fresh_pool().await;
    let err = InsertManyQuery::<Task>::new(&pool)
        .row([
            ("name", Value::Str("first".into())),
            ("parent_id", Value::I64(1)),
        ])
        .row([
            ("parent_id", Value::I64(1)),
            ("name", Value::Str("second".into())),
        ])
        .exec()
        .await
        .unwrap_err();

    let msg = format!("{err}");
    assert!(
        msg.contains("row 1"),
        "expected error to name the offending row, got: {msg}"
    );
}

#[tokio::test]
async fn insert_query_with_related_heterogeneous_child_rows_errors() {
    let pool = fresh_pool().await;
    let err = InsertQuery::<Task>::new(&pool)
        .set(NAME, "parent")
        .with_related(
            |t| t.id.to_value(),
            "parent_id",
            InsertManyQuery::<Task>::new(&pool)
                .row([("name", Value::Str("first".into()))])
                .row([
                    ("name", Value::Str("second".into())),
                    ("parent_id", Value::I64(1)),
                ]),
            SetChildren,
        )
        .exec()
        .await
        .unwrap_err();

    let msg = format!("{err}");
    assert!(
        msg.contains("row 1"),
        "expected error to name the offending child row, got: {msg}"
    );
}
