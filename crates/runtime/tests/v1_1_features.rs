//! Tests for v1.1.0 features:
//! - Array operations: .has(), .has_every(), .has_some(), .is_empty(), .is_not_empty()
//! - Full-Text Search: .matches()
//! - Soft Deletes: .with_deleted(), .only_deleted(), .soft_delete()

use ruprizzle::{Column, Executor, InsertQuery, Model, Pool, SelectQuery, UpdateQuery, connect};
use ruprizzle_testkit::IsolatedSchema;
use sqlx::Row;

static FILE_COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

#[derive(Debug, Clone, PartialEq, Default)]
struct Post {
    id: i64,
    title: String,
    content: String,
    tags: Vec<String>,
    deleted_at: Option<String>,
}

impl<'r> sqlx::FromRow<'r, sqlx::any::AnyRow> for Post {
    fn from_row(row: &'r sqlx::any::AnyRow) -> Result<Self, sqlx::Error> {
        Ok(Self {
            id: row.try_get("id")?,
            title: row.try_get("title")?,
            content: row.try_get("content")?,
            tags: ruprizzle::decode::array(row, "tags")?,
            deleted_at: row.try_get("deleted_at")?,
        })
    }
}

impl<'r> sqlx::FromRow<'r, sqlx::postgres::PgRow> for Post {
    fn from_row(row: &'r sqlx::postgres::PgRow) -> Result<Self, sqlx::Error> {
        Ok(Self {
            id: row.try_get(0)?,
            title: row.try_get(1)?,
            content: row.try_get(2)?,
            tags: row.try_get(3)?,
            deleted_at: row.try_get(4)?,
        })
    }
}

impl<'r> sqlx::FromRow<'r, sqlx::sqlite::SqliteRow> for Post {
    fn from_row(row: &'r sqlx::sqlite::SqliteRow) -> Result<Self, sqlx::Error> {
        Ok(Self {
            id: row.try_get(0)?,
            title: row.try_get(1)?,
            content: row.try_get(2)?,
            tags: ruprizzle::decode::array_idx(row, 3usize)?,
            deleted_at: row.try_get(4)?,
        })
    }
}

impl<'r> sqlx::FromRow<'r, sqlx::mysql::MySqlRow> for Post {
    fn from_row(row: &'r sqlx::mysql::MySqlRow) -> Result<Self, sqlx::Error> {
        Ok(Self {
            id: row.try_get(0)?,
            title: row.try_get(1)?,
            content: row.try_get(2)?,
            tags: ruprizzle::decode::array_idx(row, 3usize)?,
            deleted_at: row.try_get(4)?,
        })
    }
}

#[cfg(feature = "postgres-tokio-postgres")]
impl ruprizzle::tokio_postgres::FromTokioPostgresRow for Post {
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
            content: row
                .try_get::<usize, String>(2)
                .map_err(ruprizzle::Error::TokioPostgres)?,
            tags: row
                .try_get::<usize, Vec<String>>(3)
                .map_err(ruprizzle::Error::TokioPostgres)?,
            deleted_at: row
                .try_get::<usize, Option<String>>(4)
                .map_err(ruprizzle::Error::TokioPostgres)?,
        })
    }
}

#[cfg(feature = "sqlite-rusqlite")]
impl ruprizzle::rusqlite::FromRusqliteRow for Post {
    fn from_rusqlite_row(row: &ruprizzle::rusqlite::RusqliteRow) -> Result<Self, ruprizzle::Error> {
        Ok(Self {
            id: ::ruprizzle::rusqlite::get::<i64>(row, 0)?,
            title: ::ruprizzle::rusqlite::get::<String>(row, 1)?,
            content: ::ruprizzle::rusqlite::get::<String>(row, 2)?,
            tags: ::ruprizzle::rusqlite::get::<Vec<String>>(row, 3)?,
            deleted_at: ::ruprizzle::rusqlite::get_text_opt(row, 4)?,
        })
    }
}

#[cfg(feature = "sqlite-rusqlite")]
impl ruprizzle::rusqlite::FromOwnedRow for Post {
    fn from_owned_row(row: &ruprizzle::rusqlite::Row) -> Result<Self, ruprizzle::Error> {
        Ok(Self {
            id: row.get::<i64>(0)?,
            title: row.get::<String>(1)?,
            content: row.get::<String>(2)?,
            tags: row.get::<Vec<String>>(3)?,
            deleted_at: row.get::<Option<String>>(4)?,
        })
    }
}

impl Model for Post {
    const TABLE: &'static str = "posts";
    const PRIMARY_KEY: &'static str = "id";
    const COLUMNS: &'static [&'static str] = &["id", "title", "content", "tags", "deleted_at"];
    const DELETED_AT_COLUMN: Option<&'static str> = Some("deleted_at");
}

const TITLE: Column<Post, String> = Column::new("posts", "title");
const CONTENT: Column<Post, String> = Column::new("posts", "content");
const TAGS: Column<Post, Vec<String>> = Column::new("posts", "tags");

async fn fresh_pool() -> (Pool, bool, Option<IsolatedSchema>) {
    let mut isolated = None;
    let (url, is_pg) = if let Ok(base) = std::env::var("RUPRIZZLE_TEST_PG_URL") {
        let schema = IsolatedSchema::create(&base)
            .await
            .expect("create isolated schema");
        let url = schema.url().to_owned();
        isolated = Some(schema);
        (url, true)
    } else if std::env::var("RUPRIZZLE_REQUIRE_DB").is_ok() {
        panic!("RUPRIZZLE_REQUIRE_DB is set but RUPRIZZLE_TEST_PG_URL is not");
    } else {
        let dir = std::env::current_dir().unwrap().join("target/runtime-test");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(format!(
            "v110_{}_{}.sqlite",
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
        Executor::execute_raw(&pool, "DROP TABLE IF EXISTS posts".into(), Vec::new())
            .await
            .unwrap();
        Executor::execute_raw(
            &pool,
            "CREATE TABLE posts (id BIGSERIAL PRIMARY KEY, title TEXT NOT NULL, content TEXT NOT NULL, tags TEXT[] NOT NULL, deleted_at TEXT)"
                .into(),
            Vec::new(),
        )
        .await
        .unwrap();
    } else {
        Executor::execute_raw(&pool, "DROP TABLE IF EXISTS posts".into(), Vec::new())
            .await
            .unwrap();
        Executor::execute_raw(
            &pool,
            "CREATE TABLE posts (id INTEGER PRIMARY KEY AUTOINCREMENT, title TEXT NOT NULL, content TEXT NOT NULL, tags TEXT NOT NULL, deleted_at TEXT)"
                .into(),
            Vec::new(),
        )
        .await
        .unwrap();
    }

    (pool, is_pg, isolated)
}

async fn cleanup(pool: Pool, schema: Option<IsolatedSchema>) {
    pool.close().await;
    if let Some(schema) = schema {
        schema.drop_now().await.expect("drop isolated schema");
    }
}

#[tokio::test]
async fn test_array_operations() {
    let (pool, _is_pg, schema) = fresh_pool().await;

    // Post 1: tags = ["rust", "orm", "database"]
    InsertQuery::<Post>::new(&pool)
        .set(TITLE, "Rust ORM Guide")
        .set(CONTENT, "Comprehensive guide to databases in Rust")
        .set(
            TAGS,
            vec![
                "rust".to_string(),
                "orm".to_string(),
                "database".to_string(),
            ],
        )
        .exec()
        .await
        .unwrap();

    // Post 2: tags = ["rust", "async"]
    InsertQuery::<Post>::new(&pool)
        .set(TITLE, "Async in Rust")
        .set(CONTENT, "Concurrency and async await tutorial")
        .set(TAGS, vec!["rust".to_string(), "async".to_string()])
        .exec()
        .await
        .unwrap();

    // Post 3: tags = []
    InsertQuery::<Post>::new(&pool)
        .set(TITLE, "Untitled Draft")
        .set(CONTENT, "Just some notes")
        .set(TAGS, Vec::<String>::new())
        .exec()
        .await
        .unwrap();

    // 1. .has("database") -> Should match Post 1
    let rows: Vec<Post> = SelectQuery::<Post>::new(&pool)
        .filter(TAGS.has("database"))
        .fetch_all()
        .await
        .unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].title, "Rust ORM Guide");

    // 2. .has_every(["rust", "async"]) -> Should match Post 2
    let rows: Vec<Post> = SelectQuery::<Post>::new(&pool)
        .filter(TAGS.has_every(["rust", "async"]))
        .fetch_all()
        .await
        .unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].title, "Async in Rust");

    // 3. .has_some(["async", "database"]) -> Should match Post 1 and Post 2
    let rows: Vec<Post> = SelectQuery::<Post>::new(&pool)
        .filter(TAGS.has_some(["async", "database"]))
        .order_by(TITLE.asc())
        .fetch_all()
        .await
        .unwrap();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].title, "Async in Rust");
    assert_eq!(rows[1].title, "Rust ORM Guide");

    // 4. .is_empty() -> Should match Post 3
    let rows: Vec<Post> = SelectQuery::<Post>::new(&pool)
        .filter(TAGS.is_empty())
        .fetch_all()
        .await
        .unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].title, "Untitled Draft");

    // 5. .is_not_empty() -> Should match Post 1 and Post 2
    let count = SelectQuery::<Post>::new(&pool)
        .filter(TAGS.is_not_empty())
        .count()
        .await
        .unwrap();
    assert_eq!(count, 2);

    cleanup(pool, schema).await;
}

#[tokio::test]
async fn test_full_text_search_sql_compilation() {
    let (pool, _is_pg, schema) = fresh_pool().await;

    // Test SQL compilation for full-text search match operator
    let query = SelectQuery::<Post>::new(&pool).filter(CONTENT.matches("rust concurrency"));
    let compiled = query.to_sql().unwrap();

    let dialect = pool.dialect().name();
    if dialect == "postgres" {
        assert!(compiled.sql.contains("to_tsvector"));
        assert!(compiled.sql.contains("@@ plainto_tsquery"));
    } else {
        assert!(compiled.sql.contains("MATCH"));
    }

    cleanup(pool, schema).await;
}

#[tokio::test]
async fn test_soft_deletes() {
    let (pool, _is_pg, schema) = fresh_pool().await;

    // 1. Insert 2 active posts
    let p1: Post = InsertQuery::<Post>::new(&pool)
        .set(TITLE, "Active Post 1")
        .set(CONTENT, "Content 1")
        .set(TAGS, vec!["news".to_string()])
        .exec()
        .await
        .unwrap();

    let p2: Post = InsertQuery::<Post>::new(&pool)
        .set(TITLE, "Active Post 2")
        .set(CONTENT, "Content 2")
        .set(TAGS, vec!["updates".to_string()])
        .exec()
        .await
        .unwrap();

    // Verify both are returned by standard select query (deleted_at IS NULL filter auto-injected)
    let active_count = SelectQuery::<Post>::new(&pool).count().await.unwrap();
    assert_eq!(active_count, 2);

    // 2. Soft delete p1 via UpdateQuery::soft_delete
    let updated = UpdateQuery::<Post>::new(&pool)
        .filter(TITLE.eq("Active Post 1"))
        .soft_delete()
        .unwrap()
        .exec()
        .await
        .unwrap();
    assert_eq!(updated, 1);

    // 3. Standard SelectQuery: only active posts (p2)
    let active_posts: Vec<Post> = SelectQuery::<Post>::new(&pool).fetch_all().await.unwrap();
    assert_eq!(active_posts.len(), 1);
    assert_eq!(active_posts[0].id, p2.id);
    assert_eq!(active_posts[0].title, "Active Post 2");

    // 4. SelectQuery::with_deleted: both p1 and p2
    let all_posts: Vec<Post> = SelectQuery::<Post>::new(&pool)
        .with_deleted()
        .order_by(TITLE.asc())
        .fetch_all()
        .await
        .unwrap();
    assert_eq!(all_posts.len(), 2);
    assert_eq!(all_posts[0].title, "Active Post 1");
    assert!(all_posts[0].deleted_at.is_some());
    assert_eq!(all_posts[1].title, "Active Post 2");
    assert!(all_posts[1].deleted_at.is_none());

    // 5. SelectQuery::only_deleted: only p1
    let deleted_posts: Vec<Post> = SelectQuery::<Post>::new(&pool)
        .only_deleted()
        .fetch_all()
        .await
        .unwrap();
    assert_eq!(deleted_posts.len(), 1);
    assert_eq!(deleted_posts[0].id, p1.id);
    assert_eq!(deleted_posts[0].title, "Active Post 1");

    cleanup(pool, schema).await;
}
