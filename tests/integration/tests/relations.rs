//! Batched relation `include` round-trips over Postgres and SQLite.

use ruprizzle::{Column, IncludeList, IncludeOne, InsertQuery, Model, Related, SelectQuery};
use ruprizzle_testkit::both_dbs;

#[derive(Debug, Clone, PartialEq, sqlx::FromRow)]
struct User {
    id: i64,
    name: String,
    #[sqlx(skip)]
    posts: Related<Vec<Post>>,
}

impl Model for User {
    const TABLE: &'static str = "users";
}

#[derive(Debug, Clone, PartialEq, sqlx::FromRow)]
#[allow(dead_code)]
struct Post {
    id: i64,
    title: String,
    author_id: i64,
    #[sqlx(skip)]
    author: Related<Option<User>>,
    #[sqlx(skip)]
    comments: Related<Vec<Comment>>,
}

impl Model for Post {
    const TABLE: &'static str = "posts";
}

#[derive(Debug, Clone, PartialEq, sqlx::FromRow)]
#[allow(dead_code)]
struct Comment {
    id: i64,
    body: String,
    post_id: i64,
}

impl Model for Comment {
    const TABLE: &'static str = "comments";
}

const USER_ID: Column<User, i64> = Column::new("users", "id");
const USER_NAME: Column<User, String> = Column::new("users", "name");

const POST_ID: Column<Post, i64> = Column::new("posts", "id");
const POST_TITLE: Column<Post, String> = Column::new("posts", "title");
const POST_AUTHOR_ID: Column<Post, i64> = Column::new("posts", "author_id");

const COMMENT_ID: Column<Comment, i64> = Column::new("comments", "id");
const COMMENT_BODY: Column<Comment, String> = Column::new("comments", "body");
const COMMENT_POST_ID: Column<Comment, i64> = Column::new("comments", "post_id");

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

fn comments() -> IncludeList<'static, Post, Comment, i64, ()> {
    IncludeList::new(
        |p| p.id,
        |p, comments| p.comments = comments,
        COMMENT_POST_ID,
        |c| c.post_id,
    )
}

both_dbs! {
    setup = "CREATE TABLE users (id BIGINT PRIMARY KEY, name TEXT NOT NULL);
             CREATE TABLE posts (id BIGINT PRIMARY KEY, title TEXT NOT NULL, author_id BIGINT NOT NULL);
             CREATE TABLE comments (id BIGINT PRIMARY KEY, body TEXT NOT NULL, post_id BIGINT NOT NULL)";
    async fn runtime_include_round_trip(db: TestDb) {
        let pool = db.any_pool();

        for (id, name) in [(1, "alice"), (2, "bob")] {
            InsertQuery::<User>::new(pool)
                .set(USER_ID, id)
                .set(USER_NAME, name)
                .exec()
                .await?;
        }

        for (id, title, author_id) in [(1, "first", 1), (2, "second", 1), (3, "third", 2)] {
            InsertQuery::<Post>::new(pool)
                .set(POST_ID, id)
                .set(POST_TITLE, title)
                .set(POST_AUTHOR_ID, author_id)
                .exec()
                .await?;
        }

        for (id, body, post_id) in [(1, "hello", 1), (2, "world", 1), (3, "foo", 2)] {
            InsertQuery::<Comment>::new(pool)
                .set(COMMENT_ID, id)
                .set(COMMENT_BODY, body)
                .set(COMMENT_POST_ID, post_id)
                .exec()
                .await?;
        }

        let users: Vec<User> = SelectQuery::<User>::new(pool)
            .include(posts().include(comments()))
            .exec()
            .await?;

        assert_eq!(users.len(), 2);
        let alice = users.iter().find(|u| u.id == 1).unwrap();
        let bob = users.iter().find(|u| u.id == 2).unwrap();
        assert_eq!(alice.posts.get().len(), 2);
        assert_eq!(bob.posts.get().len(), 1);

        let first = alice.posts.get().iter().find(|p| p.id == 1).unwrap();
        assert_eq!(first.comments.get().len(), 2);

        let posts: Vec<Post> = SelectQuery::<Post>::new(pool)
            .include(author())
            .exec()
            .await?;

        assert_eq!(posts.len(), 3);
        let first = posts.iter().find(|p| p.id == 1).unwrap();
        assert!(first.author.get().is_some());
        assert_eq!(first.author.get().as_ref().unwrap().name, "alice");
    }
}
