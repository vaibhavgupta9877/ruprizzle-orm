//! `fetch_one` on an empty result is a typed `Error::NotFound`, so callers can
//! tell "missing" from "broken" without matching on message text.

use ruprizzle::{Error, Executor, Model, SelectQuery, connect};

#[derive(Debug, Clone, Default, sqlx::FromRow)]
struct Item {
    // Decoded by the backends; the tests only assert on the error.
    #[allow(dead_code)]
    id: i64,
}

impl Model for Item {
    const TABLE: &'static str = "items";
    const PRIMARY_KEY: &'static str = "id";
}

#[cfg(feature = "postgres-tokio-postgres")]
ruprizzle::tokio_postgres_default_row!(Item);

#[cfg(feature = "sqlite-rusqlite")]
impl ruprizzle::rusqlite::FromRusqliteRow for Item {
    fn from_rusqlite_row(row: &ruprizzle::rusqlite::RusqliteRow) -> Result<Self, Error> {
        Ok(Self {
            id: ruprizzle::rusqlite::get_i64(row, 0)?,
        })
    }
}

#[cfg(feature = "sqlite-rusqlite")]
impl ruprizzle::rusqlite::FromOwnedRow for Item {
    fn from_owned_row(row: &ruprizzle::rusqlite::Row) -> Result<Self, Error> {
        Ok(Self {
            id: row.get::<i64>(0)?,
        })
    }
}

async fn empty_pool(url: &str) -> ruprizzle::Pool {
    let pool = connect(url).await.expect("connect");
    pool.execute_raw(
        "CREATE TABLE items (id INTEGER PRIMARY KEY)"
            .to_owned()
            .into(),
        Vec::new(),
    )
    .await
    .expect("create");
    pool
}

fn assert_not_found(err: &Error) {
    assert!(
        matches!(err, Error::NotFound),
        "expected NotFound, got {err:?}"
    );
    assert_eq!(err.kind(), "not_found");
    assert_eq!(err.to_string(), "no row found for query");
}

#[tokio::test]
async fn fetch_one_on_empty_result_is_not_found() {
    let pool = empty_pool("sqlite::memory:").await;
    let err = SelectQuery::<Item>::new(&pool)
        .fetch_one()
        .await
        .unwrap_err();
    assert_not_found(&err);
}

#[cfg(feature = "sqlite-rusqlite")]
#[tokio::test]
async fn rusqlite_fetch_one_paths_are_not_found() {
    let pool = empty_pool("sqlite::memory:?driver=rusqlite").await;
    let err = SelectQuery::<Item>::new(&pool)
        .fetch_one()
        .await
        .unwrap_err();
    assert_not_found(&err);
    let err = SelectQuery::<Item>::new(&pool)
        .fetch_one_blocking()
        .unwrap_err();
    assert_not_found(&err);
}
