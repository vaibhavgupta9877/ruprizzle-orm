//! Batched relation `include` round-trips over Postgres and SQLite.

use ruprizzle::{
    Column, CountingExecutor, Encodable, Filter, FilterNode, IncludeList, IncludeOne,
    InsertManyQuery, InsertQuery, Model, NestedSetter, Related, SelectQuery, Value,
};
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

#[cfg(feature = "sqlite-rusqlite")]
impl ruprizzle::rusqlite::FromRusqliteRow for User {
    fn from_rusqlite_row(
        row: &mut ruprizzle::rusqlite::Row,
    ) -> Result<Self, ruprizzle::Error> {
        Ok(Self {
            id: row.take::<i64>(0)?,
            name: row.take::<String>(1)?,
            posts: Related::default(),
        })
    }
}

#[derive(Debug, Clone, PartialEq, sqlx::FromRow)]
struct Post {
    id: i64,
    title: String,
    published: i64,
    author_id: i64,
    #[sqlx(skip)]
    author: Related<Option<User>>,
    #[sqlx(skip)]
    comments: Related<Vec<Comment>>,
}

impl Model for Post {
    const TABLE: &'static str = "posts";
}

#[cfg(feature = "sqlite-rusqlite")]
impl ruprizzle::rusqlite::FromRusqliteRow for Post {
    fn from_rusqlite_row(
        row: &mut ruprizzle::rusqlite::Row,
    ) -> Result<Self, ruprizzle::Error> {
        Ok(Self {
            id: row.take::<i64>(0)?,
            title: row.take::<String>(1)?,
            published: row.take::<i64>(2)?,
            author_id: row.take::<i64>(3)?,
            author: Related::default(),
            comments: Related::default(),
        })
    }
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

#[cfg(feature = "sqlite-rusqlite")]
impl ruprizzle::rusqlite::FromRusqliteRow for Comment {
    fn from_rusqlite_row(
        row: &mut ruprizzle::rusqlite::Row,
    ) -> Result<Self, ruprizzle::Error> {
        Ok(Self {
            id: row.take::<i64>(0)?,
            body: row.take::<String>(1)?,
            post_id: row.take::<i64>(2)?,
        })
    }
}

const USER_ID: Column<User, i64> = Column::new("users", "id");
const USER_NAME: Column<User, String> = Column::new("users", "name");

const POST_ID: Column<Post, i64> = Column::new("posts", "id");
const POST_TITLE: Column<Post, String> = Column::new("posts", "title");
const POST_PUBLISHED: Column<Post, i64> = Column::new("posts", "published");
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
             CREATE TABLE posts (id BIGINT PRIMARY KEY, title TEXT NOT NULL, published INTEGER NOT NULL, author_id BIGINT NOT NULL);
             CREATE TABLE comments (id BIGINT PRIMARY KEY, body TEXT NOT NULL, post_id BIGINT NOT NULL)";
    async fn runtime_include_round_trip(db: TestDb) {
        let pool = db.pool();

        for (id, name) in [(1, "alice"), (2, "bob")] {
            InsertQuery::<User>::new(pool)
                .set(USER_ID, id)
                .set(USER_NAME, name)
                .exec()
                .await?;
        }

        for (id, title, published, author_id) in [
            (1, "first", 1, 1),
            (2, "second", 1, 1),
            (3, "third", 0, 2),
        ] {
            InsertQuery::<Post>::new(pool)
                .set(POST_ID, id)
                .set(POST_TITLE, title)
                .set(POST_PUBLISHED, published)
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

        // --- P5-04: relation quantifier filters ---

        // Users with at least one published post (alice has 2, bob has 0)
        let child_published = POST_PUBLISHED.eq(1);
        let users_with_posts: Vec<User> = SelectQuery::<User>::new(pool)
            .filter(Filter::<User>::new(FilterNode::Exists {
                child_table: "posts",
                child_col: "author_id",
                parent_table: "users",
                parent_col: "id",
                filter: Box::new(child_published.node.clone()),
                negated: false,
            }))
            .fetch_all()
            .await?;
        assert_eq!(users_with_posts.len(), 1);
        assert_eq!(users_with_posts[0].id, 1);

        // Users with no published posts (bob)
        let users_no_posts: Vec<User> = SelectQuery::<User>::new(pool)
            .filter(Filter::<User>::new(FilterNode::Exists {
                child_table: "posts",
                child_col: "author_id",
                parent_table: "users",
                parent_col: "id",
                filter: Box::new(child_published.node.clone()),
                negated: true,
            }))
            .fetch_all()
            .await?;
        assert_eq!(users_no_posts.len(), 1);
        assert_eq!(users_no_posts[0].id, 2);

        // Users where every post is published (alice: all published, bob: has an unpublished post)
        let users_every: Vec<User> = SelectQuery::<User>::new(pool)
            .filter(Filter::<User>::new(FilterNode::Exists {
                child_table: "posts",
                child_col: "author_id",
                parent_table: "users",
                parent_col: "id",
                filter: Box::new((!child_published).node),
                negated: true,
            }))
            .fetch_all()
            .await?;
        assert_eq!(users_every.len(), 1);
        assert_eq!(users_every[0].id, 1);

        // Posts whose author is named "alice"
        let author_alice = USER_NAME.eq("alice");
        let posts_by_alice: Vec<Post> = SelectQuery::<Post>::new(pool)
            .filter(Filter::<Post>::new(FilterNode::Exists {
                child_table: "users",
                child_col: "id",
                parent_table: "posts",
                parent_col: "author_id",
                filter: Box::new(author_alice.node),
                negated: false,
            }))
            .fetch_all()
            .await?;
        assert_eq!(posts_by_alice.len(), 2);
        assert!(posts_by_alice.iter().all(|p| p.author_id == 1));
    }
}

/// Seeds `users` users, each with `posts_each` posts, each with `comments_each`
/// comments. Ids are dense and deterministic so assertions can name rows.
async fn seed(
    pool: &ruprizzle::Pool,
    users: i64,
    posts_each: i64,
    comments_each: i64,
) -> ruprizzle_testkit::Result {
    for u in 1..=users {
        InsertQuery::<User>::new(pool)
            .set(USER_ID, u)
            .set(USER_NAME, format!("user{u}"))
            .exec()
            .await?;

        for p in 1..=posts_each {
            let post_id = u * 1_000 + p;
            InsertQuery::<Post>::new(pool)
                .set(POST_ID, post_id)
                .set(POST_TITLE, format!("post{post_id}"))
                .set(POST_PUBLISHED, 1)
                .set(POST_AUTHOR_ID, u)
                .exec()
                .await?;

            for c in 1..=comments_each {
                InsertQuery::<Comment>::new(pool)
                    .set(COMMENT_ID, post_id * 100 + c)
                    .set(COMMENT_BODY, format!("comment{c}"))
                    .set(COMMENT_POST_ID, post_id)
                    .exec()
                    .await?;
            }
        }
    }
    Ok(())
}

both_dbs! {
    setup = "CREATE TABLE users (id BIGINT PRIMARY KEY, name TEXT NOT NULL);
             CREATE TABLE posts (id BIGINT PRIMARY KEY, title TEXT NOT NULL, published INTEGER NOT NULL, author_id BIGINT NOT NULL);
             CREATE TABLE comments (id BIGINT PRIMARY KEY, body TEXT NOT NULL, post_id BIGINT NOT NULL)";
    /// G5's exit gate: a two-level include costs one query per *level*, not one
    /// per row. Any future refactor that reintroduces N+1 fails right here.
    async fn include_is_bounded(db: TestDb) {
        let pool = db.pool();
        seed(pool, 10, 5, 3).await?;

        let counter = CountingExecutor::new(pool);
        let users: Vec<User> = SelectQuery::<User>::new(&counter)
            .include(posts().include(comments()))
            .exec()
            .await?;

        // users, posts, comments — not 1 + 10 + 50.
        assert_eq!(counter.count(), 3);
        assert_eq!(users.len(), 10);
        assert!(users.iter().all(|u| u.posts.get().len() == 5));
        assert!(users
            .iter()
            .all(|u| u.posts.get().iter().all(|p| p.comments.get().len() == 3)));

        // A many-to-one include over 50 posts is still a single query, because
        // the repeated author keys are de-duplicated before the `IN`.
        counter.reset();
        let posts_with_author: Vec<Post> = SelectQuery::<Post>::new(&counter)
            .include(author())
            .exec()
            .await?;
        assert_eq!(counter.count(), 2);
        assert_eq!(posts_with_author.len(), 50);
        assert!(posts_with_author.iter().all(|p| p.author.get().is_some()));
    }
}

both_dbs! {
    setup = "CREATE TABLE users (id BIGINT PRIMARY KEY, name TEXT NOT NULL);
             CREATE TABLE posts (id BIGINT PRIMARY KEY, title TEXT NOT NULL, published INTEGER NOT NULL, author_id BIGINT NOT NULL);
             CREATE TABLE comments (id BIGINT PRIMARY KEY, body TEXT NOT NULL, post_id BIGINT NOT NULL)";
    /// `take` is per parent, not per batch — the distinction a plain `LIMIT`
    /// gets silently wrong as soon as there is more than one parent.
    async fn per_relation_take_is_per_parent(db: TestDb) {
        let pool = db.pool();
        seed(pool, 4, 5, 0).await?;

        let counter = CountingExecutor::new(pool);
        let users: Vec<User> = SelectQuery::<User>::new(&counter)
            .include(posts().order_by(POST_ID.desc()).take(2))
            .exec()
            .await?;

        // Still one query for the whole level, window function and all.
        assert_eq!(counter.count(), 2);
        assert_eq!(users.len(), 4);
        for user in &users {
            let taken = user.posts.get();
            assert_eq!(taken.len(), 2, "user {} got {} posts", user.id, taken.len());
            // Ordering is honoured inside each partition: the two highest ids.
            assert_eq!(taken[0].id, user.id * 1_000 + 5);
            assert_eq!(taken[1].id, user.id * 1_000 + 4);
        }
    }
}

both_dbs! {
    setup = "CREATE TABLE users (id BIGINT PRIMARY KEY, name TEXT NOT NULL);
             CREATE TABLE posts (id BIGINT PRIMARY KEY, title TEXT NOT NULL, published INTEGER NOT NULL, author_id BIGINT NOT NULL);
             CREATE TABLE comments (id BIGINT PRIMARY KEY, body TEXT NOT NULL, post_id BIGINT NOT NULL)";
    async fn nested_create_round_trip(db: TestDb) {
        let pool = db.pool();

        struct SetPosts;
        impl NestedSetter<User> for SetPosts {
            fn set(&self, parent: &mut User, batch: ruprizzle::executor::RowBatch) {
                parent.posts = Related::Loaded(ruprizzle::executor::decode_rows::<Post>(batch).unwrap());
            }
        }

        let user: User = InsertQuery::new(pool)
            .set(USER_ID, 1)
            .set(USER_NAME, "alice")
            .with_related(
                |u| u.id.to_value(),
                "author_id",
                InsertManyQuery::<Post>::new(pool)
                    .row([
                        ("id", Value::I64(10)),
                        ("title", Value::Str("first".to_string().into())),
                        ("published", Value::I64(1)),
                    ])
                    .row([
                        ("id", Value::I64(11)),
                        ("title", Value::Str("second".to_string().into())),
                        ("published", Value::I64(1)),
                    ]),
                SetPosts,
            )
            .exec()
            .await?;

        assert_eq!(user.id, 1);
        assert_eq!(user.name, "alice");
        assert_eq!(user.posts.get().len(), 2);
        assert_eq!(user.posts.get()[0].title, "first");
        assert_eq!(user.posts.get()[1].title, "second");
        assert!(user.posts.get().iter().all(|p| p.author_id == 1));
    }
}
