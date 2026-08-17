//! End-to-end array column round-trips on SQLite.

use ruprizzle::{Column, Executor, InsertQuery, Model, Pool, SelectQuery, connect};
use sqlx::Row;

static FILE_COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

#[derive(Debug, Clone, PartialEq, Default)]
struct Article {
    id: i64,
    title: String,
    tags: Vec<String>,
}

impl<'r> sqlx::FromRow<'r, sqlx::any::AnyRow> for Article {
    fn from_row(row: &'r sqlx::any::AnyRow) -> Result<Self, sqlx::Error> {
        Ok(Self {
            id: row.try_get("id")?,
            title: row.try_get("title")?,
            tags: ruprizzle::decode::array(row, "tags")?,
        })
    }
}

impl<'r> sqlx::FromRow<'r, sqlx::postgres::PgRow> for Article {
    fn from_row(row: &'r sqlx::postgres::PgRow) -> Result<Self, sqlx::Error> {
        Ok(Self {
            id: row.try_get(0)?,
            title: row.try_get(1)?,
            tags: row.try_get(2)?,
        })
    }
}

impl<'r> sqlx::FromRow<'r, sqlx::sqlite::SqliteRow> for Article {
    fn from_row(row: &'r sqlx::sqlite::SqliteRow) -> Result<Self, sqlx::Error> {
        Ok(Self {
            id: row.try_get(0)?,
            title: row.try_get(1)?,
            tags: ruprizzle::decode::array_idx(row, 2usize)?,
        })
    }
}

impl<'r> sqlx::FromRow<'r, sqlx::mysql::MySqlRow> for Article {
    fn from_row(row: &'r sqlx::mysql::MySqlRow) -> Result<Self, sqlx::Error> {
        Ok(Self {
            id: row.try_get(0)?,
            title: row.try_get(1)?,
            tags: ruprizzle::decode::array_idx(row, 2usize)?,
        })
    }
}

#[cfg(feature = "postgres-tokio-postgres")]
impl ruprizzle::tokio_postgres::FromTokioPostgresRow for Article {
    fn from_tokio_postgres_row(
        row: &ruprizzle::tokio_postgres::Row,
    ) -> Result<Self, ruprizzle::Error> {
        Ok(Self {
            id: row
                .try_get::<usize, i64>(0)
                .map_err(ruprizzle::Error::TokioPostgres)?,
            title: row
                .try_get::<usize, String>(1)
                .map_err(ruprizzle::Error::TokioPostgres)?,
            tags: row
                .try_get::<usize, Vec<String>>(2)
                .map_err(ruprizzle::Error::TokioPostgres)?,
        })
    }
}

#[cfg(feature = "sqlite-rusqlite")]
impl ruprizzle::rusqlite::FromRusqliteRow for Article {
    fn from_rusqlite_row(row: &ruprizzle::rusqlite::RusqliteRow) -> Result<Self, ruprizzle::Error> {
        Ok(Self {
            id: ::ruprizzle::rusqlite::get::<i64>(row, 0)?,
            title: ::ruprizzle::rusqlite::get::<String>(row, 1)?,
            tags: ::ruprizzle::rusqlite::get::<Vec<String>>(row, 2)?,
        })
    }
}

#[cfg(feature = "sqlite-rusqlite")]
impl ruprizzle::rusqlite::FromOwnedRow for Article {
    fn from_owned_row(row: &ruprizzle::rusqlite::Row) -> Result<Self, ruprizzle::Error> {
        Ok(Self {
            id: row.get::<i64>(0)?,
            title: row.get::<String>(1)?,
            tags: row.get::<Vec<String>>(2)?,
        })
    }
}

impl Model for Article {
    const TABLE: &'static str = "articles";
    const PRIMARY_KEY: &'static str = "id";
    const COLUMNS: &'static [&'static str] = &["id", "title", "tags"];
}

const TITLE: Column<Article, String> = Column::new("articles", "title");
const TAGS: Column<Article, Vec<String>> = Column::new("articles", "tags");

async fn fresh_pool() -> (Pool, bool) {
    let (url, is_pg) = if let Ok(url) = std::env::var("RUPRIZZLE_TEST_PG_URL") {
        (url, true)
    } else if std::env::var("RUPRIZZLE_REQUIRE_DB").is_ok() {
        panic!("RUPRIZZLE_REQUIRE_DB is set but RUPRIZZLE_TEST_PG_URL is not");
    } else {
        let dir = std::env::current_dir().unwrap().join("target/runtime-test");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(format!(
            "arrays_{}_{}.sqlite",
            std::process::id(),
            FILE_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        let file = path.to_str().unwrap().replace('\\', "/");
        let driver = if std::env::var("RUPRIZZLE_TEST_RUSQLITE").is_ok() {
            "&driver=rusqlite"
        } else {
            ""
        };
        let url = format!("sqlite:///{}?mode=rwc{}", file, driver);
        (url, false)
    };

    let pool = connect(&url).await.unwrap();
    let dialect = pool.dialect().name();

    if dialect == "postgres" {
        Executor::execute_raw(&pool, "DROP TABLE IF EXISTS articles".into(), Vec::new())
            .await
            .unwrap();
        Executor::execute_raw(
            &pool,
            "CREATE TABLE articles (id BIGSERIAL PRIMARY KEY, title TEXT NOT NULL, tags TEXT[] NOT NULL)"
                .into(),
            Vec::new(),
        )
        .await
        .unwrap();
    } else {
        Executor::execute_raw(&pool, "DROP TABLE IF EXISTS articles".into(), Vec::new())
            .await
            .unwrap();
        Executor::execute_raw(
            &pool,
            "CREATE TABLE articles (id INTEGER PRIMARY KEY AUTOINCREMENT, title TEXT NOT NULL, tags TEXT NOT NULL)"
                .into(),
            Vec::new(),
        )
        .await
        .unwrap();
    }

    (pool, is_pg)
}

#[tokio::test]
async fn insert_and_select_array_round_trip() {
    let (pool, _is_pg) = fresh_pool().await;

    let inserted: Article = InsertQuery::<Article>::new(&pool)
        .set(TITLE, "first")
        .set(TAGS, vec!["rust".to_string(), "orm".to_string()])
        .exec()
        .await
        .unwrap();

    let rows: Vec<Article> = SelectQuery::<Article>::new(&pool)
        .filter(TAGS.contains(["rust"]))
        .fetch_all()
        .await
        .unwrap();

    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].id, inserted.id);
    assert_eq!(rows[0].title, "first");
    assert_eq!(rows[0].tags, vec!["rust".to_string(), "orm".to_string()]);
}

#[tokio::test]
async fn array_filters_work() {
    let (pool, _is_pg) = fresh_pool().await;

    InsertQuery::<Article>::new(&pool)
        .set(TITLE, "a")
        .set(TAGS, vec!["rust".to_string(), "orm".to_string()])
        .exec()
        .await
        .unwrap();

    InsertQuery::<Article>::new(&pool)
        .set(TITLE, "b")
        .set(TAGS, vec!["sql".to_string()])
        .exec()
        .await
        .unwrap();

    let contains: Vec<Article> = SelectQuery::<Article>::new(&pool)
        .filter(TAGS.contains(["rust"]))
        .order_by(TITLE.asc())
        .fetch_all()
        .await
        .unwrap();
    assert_eq!(contains.len(), 1);
    assert_eq!(contains[0].title, "a");

    let contained_by: Vec<Article> = SelectQuery::<Article>::new(&pool)
        .filter(TAGS.contained_by(["rust".to_string(), "orm".to_string(), "sql".to_string()]))
        .order_by(TITLE.asc())
        .fetch_all()
        .await
        .unwrap();
    assert_eq!(contained_by.len(), 2);
    assert_eq!(contained_by[0].title, "a");
    assert_eq!(contained_by[1].title, "b");

    let overlaps: Vec<Article> = SelectQuery::<Article>::new(&pool)
        .filter(TAGS.overlaps(["orm".to_string(), "sql".to_string()]))
        .order_by(TITLE.asc())
        .fetch_all()
        .await
        .unwrap();
    assert_eq!(overlaps.len(), 2);
    assert_eq!(overlaps[0].title, "a");
    assert_eq!(overlaps[1].title, "b");
}
