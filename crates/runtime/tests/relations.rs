use ruprizzle::{
    Column, Encodable, Executor, IncludeList, IncludeOne, InsertManyQuery, InsertQuery, Model,
    NestedSetter, Pool, Related, SelectQuery, Value,
};
use sqlx::FromRow;

#[derive(Debug, Clone, Default, FromRow)]
struct User {
    id: i64,
    name: String,
    #[sqlx(skip)]
    posts: Related<Vec<Post>>,
}

#[cfg(feature = "postgres-tokio-postgres")]
ruprizzle::tokio_postgres_default_row!(User);

#[cfg(feature = "sqlite-rusqlite")]
impl ruprizzle::rusqlite::FromRusqliteRow for User {
    fn from_rusqlite_row(row: &ruprizzle::rusqlite::RusqliteRow) -> Result<Self, ruprizzle::Error> {
        Ok(Self {
            id: ::ruprizzle::rusqlite::get::<i64>(row, 0)?,
            name: ::ruprizzle::rusqlite::get::<String>(row, 1)?,
            posts: Related::default(),
        })
    }
}

#[cfg(feature = "sqlite-rusqlite")]
impl ruprizzle::rusqlite::FromOwnedRow for User {
    fn from_owned_row(row: &ruprizzle::rusqlite::Row) -> Result<Self, ruprizzle::Error> {
        Ok(Self {
            id: row.get::<i64>(0)?,
            name: row.get::<String>(1)?,
            posts: Related::default(),
        })
    }
}

impl Model for User {
    const TABLE: &'static str = "users";
}

#[derive(Debug, Clone, Default, FromRow)]
#[allow(dead_code)]
struct Post {
    id: i64,
    title: String,
    author_id: i64,
    #[sqlx(skip)]
    author: Related<Option<User>>,
    #[sqlx(skip)]
    co_posts: Related<Vec<Post>>,
}

#[cfg(feature = "postgres-tokio-postgres")]
ruprizzle::tokio_postgres_default_row!(Post);

#[cfg(feature = "sqlite-rusqlite")]
impl ruprizzle::rusqlite::FromRusqliteRow for Post {
    fn from_rusqlite_row(row: &ruprizzle::rusqlite::RusqliteRow) -> Result<Self, ruprizzle::Error> {
        Ok(Self {
            id: ::ruprizzle::rusqlite::get::<i64>(row, 0)?,
            title: ::ruprizzle::rusqlite::get::<String>(row, 1)?,
            author_id: ::ruprizzle::rusqlite::get::<i64>(row, 2)?,
            author: Related::default(),
            co_posts: Related::default(),
        })
    }
}

#[cfg(feature = "sqlite-rusqlite")]
impl ruprizzle::rusqlite::FromOwnedRow for Post {
    fn from_owned_row(row: &ruprizzle::rusqlite::Row) -> Result<Self, ruprizzle::Error> {
        Ok(Self {
            id: row.get::<i64>(0)?,
            title: row.get::<String>(1)?,
            author_id: row.get::<i64>(2)?,
            author: Related::default(),
            co_posts: Related::default(),
        })
    }
}

impl Model for Post {
    const TABLE: &'static str = "posts";
}

const USER_ID: Column<User, i64> = Column::new("users", "id");
const USER_NAME: Column<User, String> = Column::new("users", "name");

const POST_ID: Column<Post, i64> = Column::new("posts", "id");
const POST_TITLE: Column<Post, String> = Column::new("posts", "title");
const POST_AUTHOR_ID: Column<Post, i64> = Column::new("posts", "author_id");

fn posts() -> IncludeList<'static, User, Post, i64, ()> {
    IncludeList::new(
        |u| u.id,
        |u, posts| u.posts = posts,
        POST_AUTHOR_ID,
        |p| p.author_id,
    )
}

fn author() -> IncludeOne<'static, Post, User, i64, ()> {
    IncludeOne::new(
        |p| p.author_id,
        |p, author| p.author = author,
        USER_ID,
        |u| u.id,
    )
}

fn posts_by_author() -> IncludeList<'static, Post, Post, i64, ()> {
    IncludeList::new(
        |p| p.author_id,
        |p, co_posts| p.co_posts = co_posts,
        POST_AUTHOR_ID,
        |p| p.author_id,
    )
}

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
    ruprizzle::connect(&url).await.unwrap()
}

#[tokio::test]
async fn include_one_to_many_round_trip() {
    let pool = fresh_pool().await;

    pool.execute_raw(
        "CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT NOT NULL)"
            .to_string()
            .into(),
        Vec::new(),
    )
    .await
    .unwrap();
    pool.execute_raw(
        "CREATE TABLE posts (id INTEGER PRIMARY KEY, title TEXT NOT NULL, author_id INTEGER NOT NULL)".to_string().into(),
        Vec::new(),
    )
    .await
    .unwrap();

    for (id, name) in [(1, "alice"), (2, "bob")] {
        InsertQuery::<User>::new(&pool)
            .set(USER_ID, id)
            .set(USER_NAME, name)
            .exec()
            .await
            .unwrap();
    }

    for (id, title, author_id) in [(1, "first", 1), (2, "second", 1), (3, "third", 2)] {
        InsertQuery::<Post>::new(&pool)
            .set(POST_ID, id)
            .set(POST_TITLE, title)
            .set(POST_AUTHOR_ID, author_id)
            .exec()
            .await
            .unwrap();
    }

    let users: Vec<User> = SelectQuery::<User>::new(&pool)
        .include(posts())
        .exec()
        .await
        .unwrap();

    assert_eq!(users.len(), 2);
    let alice = users.iter().find(|u| u.id == 1).unwrap();
    let bob = users.iter().find(|u| u.id == 2).unwrap();
    assert_eq!(alice.posts.get().len(), 2);
    assert_eq!(bob.posts.get().len(), 1);

    let posts: Vec<Post> = SelectQuery::<Post>::new(&pool)
        .include(author())
        .exec()
        .await
        .unwrap();

    assert_eq!(posts.len(), 3);
    let first = posts.iter().find(|p| p.id == 1).unwrap();
    assert!(first.author.get().is_some());
    assert_eq!(first.author.get().as_ref().unwrap().name, "alice");
}

#[tokio::test]
async fn include_with_filter_and_take_round_trip() {
    let pool = fresh_pool().await;

    pool.execute_raw(
        "CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT NOT NULL)"
            .to_string()
            .into(),
        Vec::new(),
    )
    .await
    .unwrap();
    pool.execute_raw(
        "CREATE TABLE posts (id INTEGER PRIMARY KEY, title TEXT NOT NULL, author_id INTEGER NOT NULL)".to_string().into(),
        Vec::new(),
    )
    .await
    .unwrap();

    InsertQuery::<User>::new(&pool)
        .set(USER_ID, 1)
        .set(USER_NAME, "alice")
        .exec()
        .await
        .unwrap();

    for (id, title) in [(1, "a"), (2, "b"), (3, "c"), (4, "d")] {
        InsertQuery::<Post>::new(&pool)
            .set(POST_ID, id)
            .set(POST_TITLE, title)
            .set(POST_AUTHOR_ID, 1)
            .exec()
            .await
            .unwrap();
    }

    let users: Vec<User> = SelectQuery::<User>::new(&pool)
        .include(posts().take(2).order_by(POST_ID.asc()))
        .exec()
        .await
        .unwrap();

    assert_eq!(users.len(), 1);
    assert_eq!(users[0].posts.get().len(), 2);
    assert_eq!(users[0].posts.get()[0].id, 1);
    assert_eq!(users[0].posts.get()[1].id, 2);
}

#[tokio::test]
async fn exec_one_and_optional_with_include_round_trip() {
    let pool = fresh_pool().await;

    pool.execute_raw(
        "CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT NOT NULL)"
            .to_string()
            .into(),
        Vec::new(),
    )
    .await
    .unwrap();
    pool.execute_raw(
        "CREATE TABLE posts (id INTEGER PRIMARY KEY, title TEXT NOT NULL, author_id INTEGER NOT NULL)".to_string().into(),
        Vec::new(),
    )
    .await
    .unwrap();

    for (id, name) in [(1, "alice"), (2, "bob")] {
        InsertQuery::<User>::new(&pool)
            .set(USER_ID, id)
            .set(USER_NAME, name)
            .exec()
            .await
            .unwrap();
    }

    for (id, title, author_id) in [(1, "first", 1), (2, "second", 1), (3, "third", 2)] {
        InsertQuery::<Post>::new(&pool)
            .set(POST_ID, id)
            .set(POST_TITLE, title)
            .set(POST_AUTHOR_ID, author_id)
            .exec()
            .await
            .unwrap();
    }

    let alice = SelectQuery::<User>::new(&pool)
        .filter(USER_ID.eq(1))
        .include(posts())
        .exec_one()
        .await
        .unwrap();
    assert_eq!(alice.name, "alice");
    assert_eq!(alice.posts.get().len(), 2);

    let bob = SelectQuery::<User>::new(&pool)
        .filter(USER_ID.eq(2))
        .include(posts())
        .exec_optional()
        .await
        .unwrap()
        .expect("bob exists");
    assert_eq!(bob.name, "bob");
    assert_eq!(bob.posts.get().len(), 1);
    assert_eq!(bob.posts.get()[0].title, "third");

    let missing = SelectQuery::<User>::new(&pool)
        .filter(USER_ID.eq(99))
        .include(posts())
        .exec_optional()
        .await
        .unwrap();
    assert!(missing.is_none());
}

#[tokio::test]
async fn nested_create_round_trip() {
    let pool = fresh_pool().await;

    pool.execute_raw(
        "CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT NOT NULL)"
            .to_string()
            .into(),
        Vec::new(),
    )
    .await
    .unwrap();
    pool.execute_raw(
        "CREATE TABLE posts (id INTEGER PRIMARY KEY, title TEXT NOT NULL, author_id INTEGER NOT NULL)".to_string().into(),
        Vec::new(),
    )
    .await
    .unwrap();

    struct SetPosts;
    impl NestedSetter<User> for SetPosts {
        fn set(&self, parent: &mut User, batch: ruprizzle::executor::RowBatch) {
            parent.posts =
                Related::Loaded(ruprizzle::executor::decode_rows::<Post>(batch).unwrap());
        }
    }

    let user: User = InsertQuery::new(&pool)
        .set(USER_ID, 1)
        .set(USER_NAME, "alice")
        .with_related(
            |u| u.id.to_value(),
            "author_id",
            InsertManyQuery::<Post>::new(&pool)
                .row([("title", Value::Str("first".to_string().into()))])
                .row([("title", Value::Str("second".to_string().into()))]),
            SetPosts,
        )
        .exec()
        .await
        .unwrap();

    assert_eq!(user.id, 1);
    assert_eq!(user.name, "alice");
    assert_eq!(user.posts.get().len(), 2);
    assert_eq!(user.posts.get()[0].title, "first");
    assert_eq!(user.posts.get()[1].title, "second");
    assert!(user.posts.get().iter().all(|p| p.author_id == 1));
}

#[tokio::test]
async fn include_list_duplicate_parent_key_round_trip() {
    let pool = fresh_pool().await;

    pool.execute_raw(
        "CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT NOT NULL)"
            .to_string()
            .into(),
        Vec::new(),
    )
    .await
    .unwrap();
    pool.execute_raw(
        "CREATE TABLE posts (id INTEGER PRIMARY KEY, title TEXT NOT NULL, author_id INTEGER NOT NULL)".to_string().into(),
        Vec::new(),
    )
    .await
    .unwrap();

    InsertQuery::<User>::new(&pool)
        .set(USER_ID, 1)
        .set(USER_NAME, "alice")
        .exec()
        .await
        .unwrap();

    for (id, title) in [(1, "a"), (2, "b"), (3, "c")] {
        InsertQuery::<Post>::new(&pool)
            .set(POST_ID, id)
            .set(POST_TITLE, title)
            .set(POST_AUTHOR_ID, 1)
            .exec()
            .await
            .unwrap();
    }

    // Both posts 1 and 2 share author_id == 1. With the duplicate-key fix,
    // both must receive the same set of co-posts, including their own clone.
    let posts: Vec<Post> = SelectQuery::<Post>::new(&pool)
        .filter(POST_ID.in_set(vec![1, 2]))
        .include(posts_by_author())
        .exec()
        .await
        .unwrap();

    assert_eq!(posts.len(), 2);
    let first = posts.iter().find(|p| p.id == 1).unwrap();
    let second = posts.iter().find(|p| p.id == 2).unwrap();
    assert_eq!(first.co_posts.get().len(), 3);
    assert_eq!(second.co_posts.get().len(), 3);
    assert_eq!(
        first.co_posts.get()[0].title,
        second.co_posts.get()[0].title
    );
}
