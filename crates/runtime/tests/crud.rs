//! End-to-end CRUD round-trip using a live SQLite `Any` pool.

use ruprizzle::{
    Column, DeleteQuery, Executor, InsertQuery, Model, Pool, SelectQuery, UpdateQuery, connect,
};

#[derive(Debug, Clone, PartialEq, Default, sqlx::FromRow)]
struct Task {
    id: i64,
    name: String,
}

#[cfg(feature = "postgres-tokio-postgres")]
ruprizzle::tokio_postgres_default_row!(Task);

#[cfg(feature = "sqlite-rusqlite")]
impl ruprizzle::rusqlite::FromRusqliteRow for Task {
    fn from_rusqlite_row(row: &ruprizzle::rusqlite::RusqliteRow) -> Result<Self, ruprizzle::Error> {
        Ok(Self {
            id: ::ruprizzle::rusqlite::get::<i64>(row, 0)?,
            name: ::ruprizzle::rusqlite::get::<String>(row, 1)?,
        })
    }
}

#[cfg(feature = "sqlite-rusqlite")]
impl ruprizzle::rusqlite::FromOwnedRow for Task {
    fn from_owned_row(row: &ruprizzle::rusqlite::Row) -> Result<Self, ruprizzle::Error> {
        Ok(Self {
            id: row.get::<i64>(0)?,
            name: row.get::<String>(1)?,
        })
    }
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
    let driver = if std::env::var("RUPRIZZLE_TEST_RUSQLITE").is_ok() {
        "&driver=rusqlite"
    } else {
        ""
    };
    let url = format!("sqlite:///{}?mode=rwc{}", file, driver);
    let pool = connect(&url).await.unwrap();

    pool.execute_raw(
        "CREATE TABLE tasks (id INTEGER PRIMARY KEY AUTOINCREMENT, name TEXT NOT NULL)"
            .to_string()
            .into(),
        Vec::new(),
    )
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

#[tokio::test]
async fn transaction_raw_round_trip() {
    let pool = fresh_pool().await;

    let tx = ruprizzle::Tx::begin(&pool).await.unwrap();

    let rows_affected = tx
        .execute(
            "INSERT INTO tasks (name) VALUES (?)",
            &[ruprizzle::Value::Str("in-tx".into())],
        )
        .await
        .unwrap();
    assert_eq!(rows_affected, 1);

    let rows: Vec<(i64, String)> = tx
        .fetch_all("SELECT id, name FROM tasks", &[])
        .await
        .unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].1, "in-tx");

    tx.commit().await.unwrap();

    let count = SelectQuery::<Task>::new(&pool).count().await.unwrap();
    assert_eq!(count, 1);
}
