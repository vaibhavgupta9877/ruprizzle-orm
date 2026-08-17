//! Nested one-to-many writes (W2-06).

use ruprizzle::{
    Column, DeleteAction, DeleteQuery, Encodable, Executor, InsertQuery, Model, Pool, SelectQuery,
    UpdateQuery, connect,
};

#[derive(Debug, Clone, PartialEq, Default, sqlx::FromRow)]
struct User {
    id: i64,
    name: String,
}

#[cfg(feature = "postgres-tokio-postgres")]
ruprizzle::tokio_postgres_default_row!(User);

#[cfg(feature = "sqlite-rusqlite")]
impl ruprizzle::rusqlite::FromRusqliteRow for User {
    fn from_rusqlite_row(row: &ruprizzle::rusqlite::RusqliteRow) -> Result<Self, ruprizzle::Error> {
        Ok(Self {
            id: ::ruprizzle::rusqlite::get::<i64>(row, 0)?,
            name: ::ruprizzle::rusqlite::get::<String>(row, 1)?,
        })
    }
}

#[cfg(feature = "sqlite-rusqlite")]
impl ruprizzle::rusqlite::FromOwnedRow for User {
    fn from_owned_row(row: &ruprizzle::rusqlite::Row) -> Result<Self, ruprizzle::Error> {
        Ok(Self {
            id: row.get::<i64>(0)?,
            name: row.get::<String>(1)?,
        })
    }
}

impl Model for User {
    const TABLE: &'static str = "nw_users";
    const PRIMARY_KEY: &'static str = "id";
}

const USER_ID: Column<User, i64> = Column::new("nw_users", "id");
const USER_NAME: Column<User, String> = Column::new("nw_users", "name");

#[derive(Debug, Clone, PartialEq, Default, sqlx::FromRow)]
struct Post {
    id: i64,
    title: String,
    author_id: Option<i64>,
}

#[cfg(feature = "postgres-tokio-postgres")]
ruprizzle::tokio_postgres_default_row!(Post);

#[cfg(feature = "sqlite-rusqlite")]
impl ruprizzle::rusqlite::FromRusqliteRow for Post {
    fn from_rusqlite_row(row: &ruprizzle::rusqlite::RusqliteRow) -> Result<Self, ruprizzle::Error> {
        Ok(Self {
            id: ::ruprizzle::rusqlite::get::<i64>(row, 0)?,
            title: ::ruprizzle::rusqlite::get::<String>(row, 1)?,
            author_id: ::ruprizzle::rusqlite::get::<Option<i64>>(row, 2)?,
        })
    }
}

#[cfg(feature = "sqlite-rusqlite")]
impl ruprizzle::rusqlite::FromOwnedRow for Post {
    fn from_owned_row(row: &ruprizzle::rusqlite::Row) -> Result<Self, ruprizzle::Error> {
        Ok(Self {
            id: row.get::<i64>(0)?,
            title: row.get::<String>(1)?,
            author_id: row.get::<Option<i64>>(2)?,
        })
    }
}

impl Model for Post {
    const TABLE: &'static str = "nw_posts";
    const PRIMARY_KEY: &'static str = "id";
    const COLUMNS: &'static [&'static str] = &["id", "title", "author_id"];
}

const POST_TITLE: Column<Post, String> = Column::new("nw_posts", "title");
const POST_AUTHOR_ID: Column<Post, Option<i64>> = Column::new("nw_posts", "author_id");

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
        "CREATE TABLE nw_users (id INTEGER PRIMARY KEY AUTOINCREMENT, name TEXT NOT NULL)"
            .to_string()
            .into(),
        Vec::new(),
    )
    .await
    .unwrap();

    pool.execute_raw(
        "CREATE TABLE nw_posts (id INTEGER PRIMARY KEY AUTOINCREMENT, title TEXT NOT NULL, author_id INTEGER)"
            .to_string()
            .into(),
        Vec::new(),
    )
    .await
    .unwrap();

    pool
}

async fn seed(pool: &Pool) -> (User, Vec<Post>) {
    let alice = InsertQuery::<User>::new(pool)
        .set(USER_NAME, "alice")
        .exec()
        .await
        .unwrap();

    let orphan1 = InsertQuery::<Post>::new(pool)
        .set(POST_TITLE, "orphan1")
        .set(POST_AUTHOR_ID, None)
        .exec()
        .await
        .unwrap();
    let orphan2 = InsertQuery::<Post>::new(pool)
        .set(POST_TITLE, "orphan2")
        .set(POST_AUTHOR_ID, None)
        .exec()
        .await
        .unwrap();

    (alice, vec![orphan1, orphan2])
}

#[tokio::test]
async fn connect_existing_children() {
    let pool = fresh_pool().await;
    let (alice, posts) = seed(&pool).await;

    let affected = UpdateQuery::<User>::new(&pool)
        .filter(USER_ID.eq(alice.id))
        .connect::<Post, _, _>(
            |u| u.id.to_value(),
            "author_id",
            "id",
            posts.iter().map(|p| p.id),
        )
        .exec()
        .await
        .unwrap();
    assert_eq!(affected, 2);

    let connected = SelectQuery::<Post>::new(&pool)
        .filter(POST_AUTHOR_ID.eq(Some(alice.id)))
        .fetch_all()
        .await
        .unwrap();
    assert_eq!(connected.len(), 2);
}

#[tokio::test]
async fn set_replaces_existing_children() {
    let pool = fresh_pool().await;
    let (alice, posts) = seed(&pool).await;

    UpdateQuery::<User>::new(&pool)
        .filter(USER_ID.eq(alice.id))
        .connect::<Post, _, _>(
            |u| u.id.to_value(),
            "author_id",
            "id",
            vec![posts[0].id, posts[1].id],
        )
        .exec()
        .await
        .unwrap();

    let extra = InsertQuery::<Post>::new(&pool)
        .set(POST_TITLE, "extra")
        .set(POST_AUTHOR_ID, None)
        .exec()
        .await
        .unwrap();

    let affected = UpdateQuery::<User>::new(&pool)
        .filter(USER_ID.eq(alice.id))
        .set_related::<Post, _, _>(|u| u.id.to_value(), "author_id", "id", vec![extra.id])
        .exec()
        .await
        .unwrap();
    assert_eq!(affected, 3); // 2 disconnected + 1 connected

    let current = SelectQuery::<Post>::new(&pool)
        .filter(POST_AUTHOR_ID.eq(Some(alice.id)))
        .fetch_all()
        .await
        .unwrap();
    assert_eq!(current.len(), 1);
    assert_eq!(current[0].id, extra.id);
}

#[tokio::test]
async fn disconnect_specific_children() {
    let pool = fresh_pool().await;
    let (alice, posts) = seed(&pool).await;

    UpdateQuery::<User>::new(&pool)
        .filter(USER_ID.eq(alice.id))
        .connect::<Post, _, _>(
            |u| u.id.to_value(),
            "author_id",
            "id",
            vec![posts[0].id, posts[1].id],
        )
        .exec()
        .await
        .unwrap();

    let affected = UpdateQuery::<User>::new(&pool)
        .filter(USER_ID.eq(alice.id))
        .disconnect::<Post, _, _>(|u| u.id.to_value(), "author_id", "id", vec![posts[0].id])
        .exec()
        .await
        .unwrap();
    assert_eq!(affected, 1);

    let current = SelectQuery::<Post>::new(&pool)
        .filter(POST_AUTHOR_ID.eq(Some(alice.id)))
        .fetch_all()
        .await
        .unwrap();
    assert_eq!(current.len(), 1);
    assert_eq!(current[0].id, posts[1].id);
}

#[tokio::test]
async fn delete_cascade_removes_children() {
    let pool = fresh_pool().await;
    let (alice, posts) = seed(&pool).await;

    UpdateQuery::<User>::new(&pool)
        .filter(USER_ID.eq(alice.id))
        .connect::<Post, _, _>(
            |u| u.id.to_value(),
            "author_id",
            "id",
            vec![posts[0].id, posts[1].id],
        )
        .exec()
        .await
        .unwrap();

    let affected = DeleteQuery::<User>::new(&pool)
        .filter(USER_ID.eq(alice.id))
        .cascade::<Post>("author_id", DeleteAction::Cascade)
        .exec()
        .await
        .unwrap();
    assert_eq!(affected, 3); // 2 children + 1 parent

    let remaining = SelectQuery::<Post>::new(&pool).count().await.unwrap();
    assert_eq!(remaining, 0);
}

#[tokio::test]
async fn delete_set_null_clears_fk() {
    let pool = fresh_pool().await;
    let (alice, posts) = seed(&pool).await;

    UpdateQuery::<User>::new(&pool)
        .filter(USER_ID.eq(alice.id))
        .connect::<Post, _, _>(
            |u| u.id.to_value(),
            "author_id",
            "id",
            vec![posts[0].id, posts[1].id],
        )
        .exec()
        .await
        .unwrap();

    let affected = DeleteQuery::<User>::new(&pool)
        .filter(USER_ID.eq(alice.id))
        .cascade::<Post>("author_id", DeleteAction::SetNull)
        .exec()
        .await
        .unwrap();
    assert_eq!(affected, 3); // 2 children + 1 parent

    let users = SelectQuery::<User>::new(&pool).count().await.unwrap();
    assert_eq!(users, 0);

    let posts = SelectQuery::<Post>::new(&pool).fetch_all().await.unwrap();
    assert_eq!(posts.len(), 2);
    assert!(posts.iter().all(|p| p.author_id.is_none()));
}

#[tokio::test]
async fn delete_restrict_blocks_when_children_exist() {
    let pool = fresh_pool().await;
    let (alice, posts) = seed(&pool).await;

    UpdateQuery::<User>::new(&pool)
        .filter(USER_ID.eq(alice.id))
        .connect::<Post, _, _>(|u| u.id.to_value(), "author_id", "id", vec![posts[0].id])
        .exec()
        .await
        .unwrap();

    let err = DeleteQuery::<User>::new(&pool)
        .filter(USER_ID.eq(alice.id))
        .cascade::<Post>("author_id", DeleteAction::Restrict)
        .exec()
        .await
        .unwrap_err();

    assert!(err.to_string().contains("child rows exist"));
}
