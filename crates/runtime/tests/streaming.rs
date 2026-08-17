//! Unbuffered streaming round-trips.
//!
//! These tests exercise `SelectQuery::stream_unbuffered` on the live SQLite
//! `Any` backend, which uses the default `sqlx` unbuffered path.

use futures_util::StreamExt;
use ruprizzle::{Column, Executor, InsertQuery, Model, Pool, SelectQuery, connect};

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
    const TABLE: &'static str = "stream_tasks";
}

const ID: Column<Task, i64> = Column::new("stream_tasks", "id");
const NAME: Column<Task, String> = Column::new("stream_tasks", "name");

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
        "CREATE TABLE stream_tasks (id INTEGER PRIMARY KEY AUTOINCREMENT, name TEXT NOT NULL)"
            .to_string()
            .into(),
        Vec::new(),
    )
    .await
    .unwrap();

    pool
}

#[tokio::test]
async fn stream_unbuffered_returns_all_rows() {
    let pool = fresh_pool().await;

    let names = ["a", "b", "c", "d", "e"];
    for name in names {
        InsertQuery::<Task>::new(&pool)
            .set(NAME, name)
            .exec()
            .await
            .unwrap();
    }

    let mut stream = SelectQuery::<Task>::new(&pool)
        .order_by(ID.asc())
        .stream_unbuffered()
        .unwrap();

    let mut seen = Vec::new();
    while let Some(row) = stream.next().await {
        let row = row.unwrap();
        assert!(row.id > 0);
        seen.push(row.name);
    }

    assert_eq!(seen, names);
}

#[tokio::test]
async fn stream_unbuffered_with_filter() {
    let pool = fresh_pool().await;

    InsertQuery::<Task>::new(&pool)
        .set(NAME, "keep")
        .exec()
        .await
        .unwrap();
    InsertQuery::<Task>::new(&pool)
        .set(NAME, "drop")
        .exec()
        .await
        .unwrap();

    let mut stream = SelectQuery::<Task>::new(&pool)
        .filter(NAME.eq("keep"))
        .stream_unbuffered()
        .unwrap();

    let row = stream.next().await.unwrap().unwrap();
    assert_eq!(row.name, "keep");
    assert!(stream.next().await.is_none());
}

#[tokio::test]
async fn stream_unbuffered_large_result() {
    let pool = fresh_pool().await;

    let n = 1_000;
    for i in 0..n {
        let name = format!("task-{}", i);
        InsertQuery::<Task>::new(&pool)
            .set(NAME, name.as_str())
            .exec()
            .await
            .unwrap();
    }

    let mut stream = SelectQuery::<Task>::new(&pool)
        .order_by(ID.asc())
        .stream_unbuffered()
        .unwrap();

    let mut count = 0;
    while let Some(row) = stream.next().await {
        let row = row.unwrap();
        assert_eq!(row.name, format!("task-{}", count));
        count += 1;
    }

    assert_eq!(count, n);
}
