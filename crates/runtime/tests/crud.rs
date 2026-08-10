//! End-to-end CRUD round-trip using a live SQLite `Any` pool.

use ruprizzle::{Column, DeleteQuery, InsertQuery, Model, Pool, SelectQuery, UpdateQuery, connect};

#[derive(Debug, Clone, PartialEq, sqlx::FromRow)]
struct Task {
    id: i64,
    name: String,
}

impl Model for Task {
    const TABLE: &'static str = "tasks";
}

const ID: Column<Task, i64> = Column::new("tasks", "id");
const NAME: Column<Task, String> = Column::new("tasks", "name");

async fn fresh_pool() -> Pool {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("test.sqlite");
    let file = path.to_str().unwrap().replace('\\', "/");
    let url = format!("sqlite:///{}?mode=rwc", file);
    let pool = connect(&url).await.unwrap();

    sqlx::query("CREATE TABLE tasks (id INTEGER PRIMARY KEY AUTOINCREMENT, name TEXT NOT NULL)")
        .execute(&pool)
        .await
        .unwrap();

    pool
}

#[tokio::test]
async fn insert_and_select_round_trip() {
    let pool = fresh_pool().await;

    let inserted: Task = InsertQuery::<Task>::new(&pool)
        .set(NAME, "first")
        .exec()
        .await
        .unwrap();

    let rows: Vec<Task> = SelectQuery::<Task>::new(&pool)
        .filter(NAME.eq("first"))
        .fetch_all()
        .await
        .unwrap();

    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].id, inserted.id);
    assert_eq!(rows[0].name, "first");
}

#[tokio::test]
async fn update_and_delete_round_trip() {
    let pool = fresh_pool().await;

    InsertQuery::<Task>::new(&pool)
        .set(NAME, "old")
        .exec()
        .await
        .unwrap();

    let affected = UpdateQuery::<Task>::new(&pool)
        .filter(NAME.eq("old"))
        .set(NAME, "new")
        .exec()
        .await
        .unwrap();
    assert_eq!(affected, 1);

    let row = SelectQuery::<Task>::new(&pool)
        .filter(NAME.eq("new"))
        .fetch_optional()
        .await
        .unwrap();
    assert!(row.is_some());

    let deleted = DeleteQuery::<Task>::new(&pool)
        .filter(NAME.eq("new"))
        .exec()
        .await
        .unwrap();
    assert_eq!(deleted, 1);

    let count = SelectQuery::<Task>::new(&pool).count().await.unwrap();
    assert_eq!(count, 0);
}

#[tokio::test]
async fn projection_round_trip() {
    let pool = fresh_pool().await;

    InsertQuery::<Task>::new(&pool)
        .set(NAME, "ada")
        .exec()
        .await
        .unwrap();

    let rows: Vec<(i64,)> = SelectQuery::<Task>::new(&pool)
        .columns((ID,))
        .fetch_all()
        .await
        .unwrap();

    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].0, 1);
}
