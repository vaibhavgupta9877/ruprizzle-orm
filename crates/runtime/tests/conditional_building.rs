//! Conditional query building (W2-07 Step 2).
//!
//! `filter_if`, `set_if`, `order_by_if`, `limit_if`, and `offset_if` allow
//! callers to assemble a query from optional values without branching on the
//! builder's type state.

use ruprizzle::{
    Column, DeleteQuery, Executor, InsertQuery, Model, Pool, SelectQuery, UpdateQuery, connect,
};

#[derive(Debug, Clone, PartialEq, Default, sqlx::FromRow)]
struct Task {
    id: i64,
    name: String,
    priority: i64,
}

#[cfg(feature = "postgres-tokio-postgres")]
ruprizzle::tokio_postgres_default_row!(Task);

#[cfg(feature = "sqlite-rusqlite")]
impl ruprizzle::rusqlite::FromRusqliteRow for Task {
    fn from_rusqlite_row(row: &ruprizzle::rusqlite::RusqliteRow) -> Result<Self, ruprizzle::Error> {
        Ok(Self {
            id: ::ruprizzle::rusqlite::get::<i64>(row, 0)?,
            name: ::ruprizzle::rusqlite::get::<String>(row, 1)?,
            priority: ::ruprizzle::rusqlite::get::<i64>(row, 2)?,
        })
    }
}

#[cfg(feature = "sqlite-rusqlite")]
impl ruprizzle::rusqlite::FromOwnedRow for Task {
    fn from_owned_row(row: &ruprizzle::rusqlite::Row) -> Result<Self, ruprizzle::Error> {
        Ok(Self {
            id: row.get::<i64>(0)?,
            name: row.get::<String>(1)?,
            priority: row.get::<i64>(2)?,
        })
    }
}

impl Model for Task {
    const TABLE: &'static str = "cond_tasks";
}

const ID: Column<Task, i64> = Column::new("cond_tasks", "id");
const NAME: Column<Task, String> = Column::new("cond_tasks", "name");
const PRIORITY: Column<Task, i64> = Column::new("cond_tasks", "priority");

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
        "CREATE TABLE cond_tasks (id INTEGER PRIMARY KEY AUTOINCREMENT, name TEXT NOT NULL, priority INTEGER NOT NULL)"
            .to_string()
            .into(),
        Vec::new(),
    )
    .await
    .unwrap();

    pool
}

#[tokio::test]
async fn select_filter_if_applies_and_skips() {
    let pool = fresh_pool().await;

    InsertQuery::<Task>::new(&pool)
        .set(NAME, "alpha")
        .set(PRIORITY, 1i64)
        .exec()
        .await
        .unwrap();
    InsertQuery::<Task>::new(&pool)
        .set(NAME, "beta")
        .set(PRIORITY, 2i64)
        .exec()
        .await
        .unwrap();

    let maybe_name: Option<String> = Some("alpha".to_string());
    let rows: Vec<Task> = SelectQuery::<Task>::new(&pool)
        .filter_if(maybe_name.map(|n| NAME.eq(n)))
        .fetch_all()
        .await
        .unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].name, "alpha");

    let no_filter: Option<ruprizzle::Filter<Task>> = None;
    let rows: Vec<Task> = SelectQuery::<Task>::new(&pool)
        .filter_if(no_filter)
        .fetch_all()
        .await
        .unwrap();
    assert_eq!(rows.len(), 2);
}

#[tokio::test]
async fn select_order_by_if_and_limit_if() {
    let pool = fresh_pool().await;

    InsertQuery::<Task>::new(&pool)
        .set(NAME, "a")
        .set(PRIORITY, 2i64)
        .exec()
        .await
        .unwrap();
    InsertQuery::<Task>::new(&pool)
        .set(NAME, "b")
        .set(PRIORITY, 1i64)
        .exec()
        .await
        .unwrap();

    let rows: Vec<Task> = SelectQuery::<Task>::new(&pool)
        .order_by_if(Some(PRIORITY.asc()))
        .limit_if(Some(1))
        .fetch_all()
        .await
        .unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].name, "b");

    let rows: Vec<Task> = SelectQuery::<Task>::new(&pool)
        .order_by_if(None)
        .limit_if(None)
        .fetch_all()
        .await
        .unwrap();
    assert_eq!(rows.len(), 2);
}

#[tokio::test]
async fn update_set_if_and_filter_if() {
    let pool = fresh_pool().await;

    InsertQuery::<Task>::new(&pool)
        .set(NAME, "old")
        .set(PRIORITY, 1i64)
        .exec()
        .await
        .unwrap();

    let new_name: Option<String> = Some("new".to_string());
    let filter: Option<ruprizzle::Filter<Task>> = Some(NAME.eq("old"));
    let affected = UpdateQuery::<Task>::new(&pool)
        .set_if(NAME, new_name)
        .set_if(PRIORITY, None::<i64>)
        .filter_if(filter)
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
}

#[tokio::test]
async fn delete_filter_if_with_none_deletes_all_rows() {
    let pool = fresh_pool().await;

    InsertQuery::<Task>::new(&pool)
        .set(NAME, "keep")
        .set(PRIORITY, 1i64)
        .exec()
        .await
        .unwrap();
    InsertQuery::<Task>::new(&pool)
        .set(NAME, "drop")
        .set(PRIORITY, 2i64)
        .exec()
        .await
        .unwrap();

    let filter: Option<ruprizzle::Filter<Task>> = Some(NAME.eq("drop"));
    let affected = DeleteQuery::<Task>::new(&pool)
        .filter_if(filter)
        .exec()
        .await
        .unwrap();
    assert_eq!(affected, 1);

    let remaining = SelectQuery::<Task>::new(&pool).count().await.unwrap();
    assert_eq!(remaining, 1);

    // None means all rows.
    DeleteQuery::<Task>::new(&pool)
        .filter_if(None)
        .exec()
        .await
        .unwrap();
    let remaining = SelectQuery::<Task>::new(&pool).count().await.unwrap();
    assert_eq!(remaining, 0);
}

#[tokio::test]
async fn insert_set_if_skips_none() {
    let pool = fresh_pool().await;

    let row = InsertQuery::<Task>::new(&pool)
        .set(NAME, "always")
        .set_if(PRIORITY, Some(5i64))
        .set_if(ID, None::<i64>)
        .exec()
        .await
        .unwrap();

    assert_eq!(row.name, "always");
    assert_eq!(row.priority, 5);
}
