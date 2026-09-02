//! Integration tests for Nested Relational Mutations 2.0 (v1.3.0).

use ruprizzle::prelude::*;

#[derive(Debug, Clone, PartialEq, Default, sqlx::FromRow)]
struct User {
    id: String,
    email: String,
    name: String,
}

#[cfg(feature = "postgres-tokio-postgres")]
ruprizzle::tokio_postgres_default_row!(User);

#[cfg(feature = "sqlite-rusqlite")]
impl ruprizzle::rusqlite::FromRusqliteRow for User {
    fn from_rusqlite_row(row: &ruprizzle::rusqlite::RusqliteRow) -> Result<Self, ruprizzle::Error> {
        Ok(Self {
            id: ::ruprizzle::rusqlite::get::<String>(row, 0)?,
            email: ::ruprizzle::rusqlite::get::<String>(row, 1)?,
            name: ::ruprizzle::rusqlite::get::<String>(row, 2)?,
        })
    }
}

#[cfg(feature = "sqlite-rusqlite")]
impl ruprizzle::rusqlite::FromOwnedRow for User {
    fn from_owned_row(row: &ruprizzle::rusqlite::Row) -> Result<Self, ruprizzle::Error> {
        Ok(Self {
            id: row.get::<String>(0)?,
            email: row.get::<String>(1)?,
            name: row.get::<String>(2)?,
        })
    }
}

impl Model for User {
    const TABLE: &'static str = "users";
    const PRIMARY_KEY: &'static str = "id";
    const COLUMNS: &'static [&'static str] = &["id", "email", "name"];
}

const USER_ID: Column<User, String> = Column::new("users", "id");
const USER_EMAIL: Column<User, String> = Column::new("users", "email");
const USER_NAME: Column<User, String> = Column::new("users", "name");

#[derive(Debug, Clone, PartialEq, Default, sqlx::FromRow)]
struct Post {
    id: String,
    user_id: String,
    title: String,
}

#[cfg(feature = "postgres-tokio-postgres")]
ruprizzle::tokio_postgres_default_row!(Post);

#[cfg(feature = "sqlite-rusqlite")]
impl ruprizzle::rusqlite::FromRusqliteRow for Post {
    fn from_rusqlite_row(row: &ruprizzle::rusqlite::RusqliteRow) -> Result<Self, ruprizzle::Error> {
        Ok(Self {
            id: ::ruprizzle::rusqlite::get::<String>(row, 0)?,
            user_id: ::ruprizzle::rusqlite::get::<String>(row, 1)?,
            title: ::ruprizzle::rusqlite::get::<String>(row, 2)?,
        })
    }
}

#[cfg(feature = "sqlite-rusqlite")]
impl ruprizzle::rusqlite::FromOwnedRow for Post {
    fn from_owned_row(row: &ruprizzle::rusqlite::Row) -> Result<Self, ruprizzle::Error> {
        Ok(Self {
            id: row.get::<String>(0)?,
            user_id: row.get::<String>(1)?,
            title: row.get::<String>(2)?,
        })
    }
}

impl Model for Post {
    const TABLE: &'static str = "posts";
    const PRIMARY_KEY: &'static str = "id";
    const COLUMNS: &'static [&'static str] = &["id", "user_id", "title"];
}

const POST_ID: Column<Post, String> = Column::new("posts", "id");
const POST_USER_ID: Column<Post, String> = Column::new("posts", "user_id");
const POST_TITLE: Column<Post, String> = Column::new("posts", "title");

async fn setup_db() -> (Pool, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("test.sqlite");
    let file = path.to_str().unwrap().replace('\\', "/");
    let driver = if std::env::var("RUPRIZZLE_TEST_RUSQLITE").is_ok() {
        "&driver=rusqlite"
    } else {
        ""
    };
    let url = format!("sqlite:///{}?mode=rwc{}", file, driver);
    let pool = ruprizzle::connect(&url).await.unwrap();

    pool.execute_raw(
        "CREATE TABLE users (id TEXT PRIMARY KEY, email TEXT UNIQUE, name TEXT);"
            .to_string()
            .into(),
        vec![],
    )
    .await
    .unwrap();

    pool.execute_raw(
        "CREATE TABLE posts (id TEXT PRIMARY KEY, user_id TEXT, title TEXT);"
            .to_string()
            .into(),
        vec![],
    )
    .await
    .unwrap();

    (pool, dir)
}

#[tokio::test]
async fn test_nested_rel_create_and_save() {
    let (pool, _dir) = setup_db().await;

    // Insert User with nested Post create operations
    let post_create1 = NestedCreate::<Post>::new()
        .set(POST_ID, "p1")
        .set(POST_TITLE, "First Post");

    let post_create2 = NestedCreate::<Post>::new()
        .set(POST_ID, "p2")
        .set(POST_TITLE, "Second Post");

    let nested_write = NestedRelWrite::<User, Post>::new(
        |u| u.id.clone().to_value(),
        "posts",
        "user_id",
        "id",
        vec![RelNestedOp::Create(vec![post_create1, post_create2])],
    );

    let user: User = InsertQuery::<User>::new(&pool)
        .set(USER_ID, "u1")
        .set(USER_EMAIL, "alex@example.com")
        .set(USER_NAME, "Alex")
        .with_nested_write(nested_write)
        .save()
        .await
        .expect("user insert with nested create should succeed");

    assert_eq!(user.id, "u1");
    assert_eq!(user.email, "alex@example.com");

    // Verify children in database
    let posts: Vec<Post> = SelectQuery::<Post>::new(&pool)
        .filter(POST_USER_ID.eq("u1"))
        .order_by(POST_ID.asc())
        .all()
        .await
        .unwrap();

    assert_eq!(posts.len(), 2);
    assert_eq!(posts[0].id, "p1");
    assert_eq!(posts[0].user_id, "u1");
    assert_eq!(posts[1].id, "p2");
    assert_eq!(posts[1].user_id, "u1");
}

#[tokio::test]
async fn test_nested_rel_connect_or_create() {
    let (pool, _dir) = setup_db().await;

    // Pre-insert an existing post
    pool.execute_raw(
        "INSERT INTO posts (id, user_id, title) VALUES ('p_existing', 'other_user', 'Old Title')"
            .to_string()
            .into(),
        vec![],
    )
    .await
    .unwrap();

    // Insert user and connect existing post or create fallback
    let existing_check = NestedConnectOrCreate::<Post>::new(
        POST_ID.eq("p_existing"),
        NestedCreate::<Post>::new()
            .set(POST_ID, "p_existing_fallback")
            .set(POST_TITLE, "Fallback Title"),
    );

    let new_post = NestedConnectOrCreate::<Post>::new(
        POST_ID.eq("p_brand_new"),
        NestedCreate::<Post>::new()
            .set(POST_ID, "p_brand_new")
            .set(POST_TITLE, "Brand New Title"),
    );

    let nested_write = NestedRelWrite::<User, Post>::new(
        |u| u.id.clone().to_value(),
        "posts",
        "user_id",
        "id",
        vec![RelNestedOp::ConnectOrCreate(vec![existing_check, new_post])],
    );

    let user: User = InsertQuery::<User>::new(&pool)
        .set(USER_ID, "u2")
        .set(USER_EMAIL, "bob@example.com")
        .set(USER_NAME, "Bob")
        .with_nested_write(nested_write)
        .save()
        .await
        .unwrap();

    assert_eq!(user.id, "u2");

    // Verify p_existing was updated to user_id = u2 and p_brand_new was created
    let posts: Vec<Post> = SelectQuery::<Post>::new(&pool)
        .filter(POST_USER_ID.eq("u2"))
        .order_by(POST_ID.asc())
        .all()
        .await
        .unwrap();

    assert_eq!(posts.len(), 2);
    assert_eq!(posts[0].id, "p_brand_new");
    assert_eq!(posts[0].user_id, "u2");
    assert_eq!(posts[1].id, "p_existing");
    assert_eq!(posts[1].user_id, "u2");
}

#[tokio::test]
async fn test_nested_transaction_rollback_on_failure() {
    let (pool, _dir) = setup_db().await;

    // Pre-insert user with email "clara@example.com"
    pool.execute_raw(
        "INSERT INTO users (id, email, name) VALUES ('u_existing', 'clara@example.com', 'Clara')"
            .to_string()
            .into(),
        vec![],
    )
    .await
    .unwrap();

    // Try inserting another user that violates the unique email constraint during parent insert
    let post_create = NestedCreate::<Post>::new()
        .set(POST_ID, "p_fail")
        .set(POST_TITLE, "Never Created");

    let nested_write = NestedRelWrite::<User, Post>::new(
        |u| u.id.clone().to_value(),
        "posts",
        "user_id",
        "id",
        vec![RelNestedOp::Create(vec![post_create])],
    );

    let res = InsertQuery::<User>::new(&pool)
        .set(USER_ID, "u_duplicate")
        .set(USER_EMAIL, "clara@example.com")
        .set(USER_NAME, "Clara Duplicate")
        .with_nested_write(nested_write)
        .save()
        .await;

    assert!(res.is_err(), "must fail due to unique constraint on email");

    // Verify post was NOT created
    let posts: Vec<Post> = SelectQuery::<Post>::new(&pool)
        .filter(POST_ID.eq("p_fail"))
        .all()
        .await
        .unwrap();

    assert_eq!(posts.len(), 0, "child post should have rolled back");
}
