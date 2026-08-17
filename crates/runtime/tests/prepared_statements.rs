//! Prepared statement tests (W2-07 Step 1).

use ruprizzle::{Column, Encodable, Executor, InsertQuery, Model, Pool, SelectQuery, connect};

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
    const TABLE: &'static str = "prep_tasks";
}

const NAME: Column<Task, String> = Column::new("prep_tasks", "name");

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
        "CREATE TABLE prep_tasks (id INTEGER PRIMARY KEY AUTOINCREMENT, name TEXT NOT NULL)"
            .to_string()
            .into(),
        Vec::new(),
    )
    .await
    .unwrap();

    pool
}

#[tokio::test]
async fn prepared_select_can_rebind_and_reexecute() {
    let pool = fresh_pool().await;

    InsertQuery::<Task>::new(&pool)
        .set(NAME, "one")
        .exec()
        .await
        .unwrap();
    InsertQuery::<Task>::new(&pool)
        .set(NAME, "two")
        .exec()
        .await
        .unwrap();

    let mut prep = SelectQuery::<Task>::new(&pool)
        .filter(NAME.eq("placeholder"))
        .prepare()
        .unwrap();

    prep.bind(0, "one".to_string());
    let rows = prep.fetch_all().await.unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].name, "one");

    prep.bind(0, "two".to_string());
    let rows = prep.fetch_all().await.unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].name, "two");
}

#[tokio::test]
async fn prepared_select_bind_many() {
    let pool = fresh_pool().await;

    InsertQuery::<Task>::new(&pool)
        .set(NAME, "x")
        .exec()
        .await
        .unwrap();

    let mut prep = SelectQuery::<Task>::new(&pool)
        .filter(NAME.eq("placeholder"))
        .prepare()
        .unwrap();

    prep.bind_many(vec!["x".to_string().to_value()]);
    let rows = prep.fetch_all().await.unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].name, "x");
}
