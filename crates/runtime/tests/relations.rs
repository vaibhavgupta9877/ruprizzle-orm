use ruprizzle::{Column, IncludeList, IncludeOne, InsertQuery, Model, Pool, Related, SelectQuery};
use sqlx::FromRow;

#[derive(Debug, Clone, FromRow)]
struct User {
    id: i64,
    name: String,
    #[sqlx(skip)]
    posts: Related<Vec<Post>>,
}

impl Model for User {
    const TABLE: &'static str = "users";
}

#[derive(Debug, Clone, FromRow)]
#[allow(dead_code)]
struct Post {
    id: i64,
    title: String,
    author_id: i64,
    #[sqlx(skip)]
    author: Related<Option<User>>,
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

async fn fresh_pool() -> Pool {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("test.sqlite");
    let file = path.to_str().unwrap().replace('\\', "/");
    let url = format!("sqlite:///{}?mode=rwc", file);
    ruprizzle::connect(&url).await.unwrap()
}

#[tokio::test]
async fn include_one_to_many_round_trip() {
    let pool = fresh_pool().await;

    sqlx::query("CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT NOT NULL)")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query(
        "CREATE TABLE posts (id INTEGER PRIMARY KEY, title TEXT NOT NULL, author_id INTEGER NOT NULL)",
    )
    .execute(&pool)
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

    sqlx::query("CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT NOT NULL)")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query(
        "CREATE TABLE posts (id INTEGER PRIMARY KEY, title TEXT NOT NULL, author_id INTEGER NOT NULL)",
    )
    .execute(&pool)
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
