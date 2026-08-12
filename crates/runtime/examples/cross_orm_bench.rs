//! Cross-ORM SQLite benchmark for ruprizzle.
#![allow(dead_code)]
//!
//! Compares against the same `bench.sqlite3` file used by the Prisma and
//! Drizzle benchmarks in `local/cross-orm-bench/node`.
//!
//! Set `RUST_BENCH_DRIVER=rusqlite` to measure the native `rusqlite` backend;
//! otherwise the default sqlx-backed SQLite driver is used.

use std::borrow::Cow;
use std::time::Instant;

use ruprizzle::serde::Serialize;
use ruprizzle::serde_json;
use ruprizzle::{
    Column, CountingExecutor, Encodable, Executor, IncludeList, IncludeOne, InsertManyQuery,
    Model, Related, SelectQuery,
};
use simple_process_stats::ProcessStats;
use sqlx::FromRow;

fn db_path() -> String {
    std::env::var("BENCH_SQLITE_PATH")
        .unwrap_or_else(|_| "local/cross-orm-bench/node/bench.sqlite3".to_string())
}

#[derive(Debug, Clone, Default, FromRow, Serialize)]
struct User {
    id: i64,
    email: String,
    age: i64,
    name: String,
    created_at: i64,
    #[serde(skip)]
    #[sqlx(skip)]
    posts: Related<Vec<Post>>,
    #[serde(skip)]
    #[sqlx(skip)]
    comments: Related<Vec<Comment>>,
    #[serde(skip)]
    #[sqlx(skip)]
    likes: Related<Vec<Like>>,
    #[serde(skip)]
    #[sqlx(skip)]
    following: Related<Vec<Follower>>,
    #[serde(skip)]
    #[sqlx(skip)]
    followers: Related<Vec<Follower>>,
}

#[cfg(feature = "postgres-tokio-postgres")]
ruprizzle::tokio_postgres_default_row!(User);

#[cfg(feature = "sqlite-rusqlite")]
impl ruprizzle::rusqlite::FromRusqliteRow for User {
    fn from_rusqlite_row(row: &mut ruprizzle::rusqlite::Row) -> Result<Self, ruprizzle::Error> {
        Ok(Self {
            id: row.take::<i64>(0)?,
            email: row.take::<String>(1)?,
            age: row.take::<i64>(2)?,
            name: row.take::<String>(3)?,
            created_at: row.take::<i64>(4)?,
            posts: Related::default(),
            comments: Related::default(),
            likes: Related::default(),
            following: Related::default(),
            followers: Related::default(),
        })
    }
}

impl Model for User {
    const TABLE: &'static str = "users";
}

#[derive(Debug, Clone, Default, FromRow, Serialize)]
struct Category {
    id: i64,
    name: String,
    #[serde(skip)]
    #[sqlx(skip)]
    posts: Related<Vec<Post>>,
}

#[cfg(feature = "postgres-tokio-postgres")]
ruprizzle::tokio_postgres_default_row!(Category);

#[cfg(feature = "sqlite-rusqlite")]
impl ruprizzle::rusqlite::FromRusqliteRow for Category {
    fn from_rusqlite_row(row: &mut ruprizzle::rusqlite::Row) -> Result<Self, ruprizzle::Error> {
        Ok(Self {
            id: row.take::<i64>(0)?,
            name: row.take::<String>(1)?,
            posts: Related::default(),
        })
    }
}

impl Model for Category {
    const TABLE: &'static str = "categories";
}

#[derive(Debug, Clone, Default, FromRow, Serialize)]
struct Post {
    id: i64,
    author_id: i64,
    category_id: i64,
    title: String,
    published_at: i64,
    views: i64,
    #[serde(skip)]
    #[sqlx(skip)]
    author: Related<Option<User>>,
    #[serde(skip)]
    #[sqlx(skip)]
    category: Related<Option<Category>>,
    #[serde(skip)]
    #[sqlx(skip)]
    comments: Related<Vec<Comment>>,
    #[serde(skip)]
    #[sqlx(skip)]
    likes: Related<Vec<Like>>,
    #[serde(skip)]
    #[sqlx(skip)]
    post_tags: Related<Vec<PostTag>>,
}

#[cfg(feature = "postgres-tokio-postgres")]
ruprizzle::tokio_postgres_default_row!(Post);

#[cfg(feature = "sqlite-rusqlite")]
impl ruprizzle::rusqlite::FromRusqliteRow for Post {
    fn from_rusqlite_row(row: &mut ruprizzle::rusqlite::Row) -> Result<Self, ruprizzle::Error> {
        Ok(Self {
            id: row.take::<i64>(0)?,
            author_id: row.take::<i64>(1)?,
            category_id: row.take::<i64>(2)?,
            title: row.take::<String>(3)?,
            published_at: row.take::<i64>(4)?,
            views: row.take::<i64>(5)?,
            author: Related::default(),
            category: Related::default(),
            comments: Related::default(),
            likes: Related::default(),
            post_tags: Related::default(),
        })
    }
}

impl Model for Post {
    const TABLE: &'static str = "posts";
}

#[derive(Debug, Clone, Default, FromRow, Serialize)]
struct Comment {
    id: i64,
    post_id: i64,
    author_id: i64,
    content: String,
    created_at: i64,
    #[serde(skip)]
    #[sqlx(skip)]
    post: Related<Option<Post>>,
    #[serde(skip)]
    #[sqlx(skip)]
    author: Related<Option<User>>,
}

#[cfg(feature = "postgres-tokio-postgres")]
ruprizzle::tokio_postgres_default_row!(Comment);

#[cfg(feature = "sqlite-rusqlite")]
impl ruprizzle::rusqlite::FromRusqliteRow for Comment {
    fn from_rusqlite_row(row: &mut ruprizzle::rusqlite::Row) -> Result<Self, ruprizzle::Error> {
        Ok(Self {
            id: row.take::<i64>(0)?,
            post_id: row.take::<i64>(1)?,
            author_id: row.take::<i64>(2)?,
            content: row.take::<String>(3)?,
            created_at: row.take::<i64>(4)?,
            post: Related::default(),
            author: Related::default(),
        })
    }
}

impl Model for Comment {
    const TABLE: &'static str = "comments";
}

#[derive(Debug, Clone, Default, FromRow, Serialize)]
struct Tag {
    id: i64,
    name: String,
    #[serde(skip)]
    #[sqlx(skip)]
    post_tags: Related<Vec<PostTag>>,
}

#[cfg(feature = "postgres-tokio-postgres")]
ruprizzle::tokio_postgres_default_row!(Tag);

#[cfg(feature = "sqlite-rusqlite")]
impl ruprizzle::rusqlite::FromRusqliteRow for Tag {
    fn from_rusqlite_row(row: &mut ruprizzle::rusqlite::Row) -> Result<Self, ruprizzle::Error> {
        Ok(Self {
            id: row.take::<i64>(0)?,
            name: row.take::<String>(1)?,
            post_tags: Related::default(),
        })
    }
}

impl Model for Tag {
    const TABLE: &'static str = "tags";
}

#[derive(Debug, Clone, Default, FromRow, Serialize)]
struct PostTag {
    post_id: i64,
    tag_id: i64,
    #[serde(skip)]
    #[sqlx(skip)]
    post: Related<Option<Post>>,
    #[serde(skip)]
    #[sqlx(skip)]
    tag: Related<Option<Tag>>,
}

#[cfg(feature = "postgres-tokio-postgres")]
ruprizzle::tokio_postgres_default_row!(PostTag);

#[cfg(feature = "sqlite-rusqlite")]
impl ruprizzle::rusqlite::FromRusqliteRow for PostTag {
    fn from_rusqlite_row(row: &mut ruprizzle::rusqlite::Row) -> Result<Self, ruprizzle::Error> {
        Ok(Self {
            post_id: row.take::<i64>(0)?,
            tag_id: row.take::<i64>(1)?,
            post: Related::default(),
            tag: Related::default(),
        })
    }
}

impl Model for PostTag {
    const TABLE: &'static str = "post_tags";
    const PRIMARY_KEY: &'static str = "post_id";
}

#[derive(Debug, Clone, Default, FromRow, Serialize)]
struct Follower {
    follower_id: i64,
    followee_id: i64,
    created_at: i64,
    #[serde(skip)]
    #[sqlx(skip)]
    follower: Related<Option<User>>,
    #[serde(skip)]
    #[sqlx(skip)]
    followee: Related<Option<User>>,
}

#[cfg(feature = "postgres-tokio-postgres")]
ruprizzle::tokio_postgres_default_row!(Follower);

#[cfg(feature = "sqlite-rusqlite")]
impl ruprizzle::rusqlite::FromRusqliteRow for Follower {
    fn from_rusqlite_row(row: &mut ruprizzle::rusqlite::Row) -> Result<Self, ruprizzle::Error> {
        Ok(Self {
            follower_id: row.take::<i64>(0)?,
            followee_id: row.take::<i64>(1)?,
            created_at: row.take::<i64>(2)?,
            follower: Related::default(),
            followee: Related::default(),
        })
    }
}

impl Model for Follower {
    const TABLE: &'static str = "followers";
    const PRIMARY_KEY: &'static str = "follower_id";
}

#[derive(Debug, Clone, Default, FromRow, Serialize)]
struct Like {
    id: i64,
    user_id: i64,
    post_id: i64,
    created_at: i64,
    #[serde(skip)]
    #[sqlx(skip)]
    user: Related<Option<User>>,
    #[serde(skip)]
    #[sqlx(skip)]
    post: Related<Option<Post>>,
}

#[cfg(feature = "postgres-tokio-postgres")]
ruprizzle::tokio_postgres_default_row!(Like);

#[cfg(feature = "sqlite-rusqlite")]
impl ruprizzle::rusqlite::FromRusqliteRow for Like {
    fn from_rusqlite_row(row: &mut ruprizzle::rusqlite::Row) -> Result<Self, ruprizzle::Error> {
        Ok(Self {
            id: row.take::<i64>(0)?,
            user_id: row.take::<i64>(1)?,
            post_id: row.take::<i64>(2)?,
            created_at: row.take::<i64>(3)?,
            user: Related::default(),
            post: Related::default(),
        })
    }
}

impl Model for Like {
    const TABLE: &'static str = "likes";
}

#[derive(Debug, Clone, Default, FromRow, Serialize)]
struct BenchBulk {
    id: i64,
    name: String,
    n: i64,
}

#[cfg(feature = "postgres-tokio-postgres")]
ruprizzle::tokio_postgres_default_row!(BenchBulk);

#[cfg(feature = "sqlite-rusqlite")]
impl ruprizzle::rusqlite::FromRusqliteRow for BenchBulk {
    fn from_rusqlite_row(row: &mut ruprizzle::rusqlite::Row) -> Result<Self, ruprizzle::Error> {
        Ok(Self {
            id: row.take::<i64>(0)?,
            name: row.take::<String>(1)?,
            n: row.take::<i64>(2)?,
        })
    }
}

impl Model for BenchBulk {
    const TABLE: &'static str = "bench_bulk";
}

const USER_ID: Column<User, i64> = Column::new("users", "id");
const USER_EMAIL: Column<User, String> = Column::new("users", "email");
const USER_AGE: Column<User, i64> = Column::new("users", "age");
const USER_NAME: Column<User, String> = Column::new("users", "name");
const USER_CREATED_AT: Column<User, i64> = Column::new("users", "created_at");

const POST_ID: Column<Post, i64> = Column::new("posts", "id");
const POST_AUTHOR_ID: Column<Post, i64> = Column::new("posts", "author_id");
const POST_CATEGORY_ID: Column<Post, i64> = Column::new("posts", "category_id");
const POST_TITLE: Column<Post, String> = Column::new("posts", "title");
const POST_PUBLISHED_AT: Column<Post, i64> = Column::new("posts", "published_at");
const POST_VIEWS: Column<Post, i64> = Column::new("posts", "views");

const COMMENT_POST_ID: Column<Comment, i64> = Column::new("comments", "post_id");
const COMMENT_AUTHOR_ID: Column<Comment, i64> = Column::new("comments", "author_id");

const POST_TAG_POST_ID: Column<PostTag, i64> = Column::new("post_tags", "post_id");
const POST_TAG_TAG_ID: Column<PostTag, i64> = Column::new("post_tags", "tag_id");

const TAG_ID: Column<Tag, i64> = Column::new("tags", "id");
const CATEGORY_ID: Column<Category, i64> = Column::new("categories", "id");

#[derive(Default)]
struct BenchOutcome {
    rows: usize,
    queries: usize,
}

#[derive(Serialize)]
struct BenchResult {
    orm: String,
    operation: String,
    iters: u32,
    total_ms: f64,
    avg_ms: f64,
    qps: f64,
    rows_returned: usize,
    queries_issued: usize,
    peak_rss_mb: f64,
    cpu_time_ms: f64,
}

fn sample_stats() -> ProcessStats {
    ProcessStats::get().unwrap_or_else(|_| ProcessStats {
        cpu_time_user: std::time::Duration::ZERO,
        cpu_time_kernel: std::time::Duration::ZERO,
        memory_usage_bytes: 0,
    })
}

fn record_result(
    name: &str,
    iters: u32,
    total: Instant,
    before: ProcessStats,
    after: ProcessStats,
    outcome: &BenchOutcome,
) -> BenchResult {
    let total_ms = total.elapsed().as_secs_f64() * 1000.0;
    let avg_ms = total_ms / iters as f64;
    let qps = iters as f64 * 1000.0 / total_ms.max(f64::MIN_POSITIVE);
    let cpu = (after.cpu_time_user - before.cpu_time_user)
        + (after.cpu_time_kernel - before.cpu_time_kernel);
    let cpu_time_ms = cpu.as_secs_f64() * 1000.0;
    let peak_rss = before.memory_usage_bytes.max(after.memory_usage_bytes);
    let peak_rss_mb = peak_rss as f64 / (1024.0 * 1024.0);
    println!(
        "{:>28} {:>10.3} us/op  (total {:>7.1} ms, {} iters)",
        name,
        avg_ms * 1000.0,
        total_ms,
        iters
    );
    BenchResult {
        orm: "ruprizzle".to_string(),
        operation: name.to_string(),
        iters,
        total_ms,
        avg_ms,
        qps,
        rows_returned: outcome.rows,
        queries_issued: outcome.queries,
        peak_rss_mb,
        cpu_time_ms,
    }
}

fn bench_sync<F>(name: &str, iters: u32, mut f: F) -> BenchResult
where
    F: FnMut() -> BenchOutcome,
{
    for _ in 0..3.min(iters) {
        let _ = f();
    }
    let before = sample_stats();
    let start = Instant::now();
    let mut last = BenchOutcome::default();
    for _ in 0..iters {
        last = f();
    }
    let after = sample_stats();
    record_result(name, iters, start, before, after, &last)
}

async fn bench_async<F, Fut>(name: &str, iters: u32, mut f: F) -> BenchResult
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = BenchOutcome>,
{
    for _ in 0..3.min(iters) {
        let _ = f().await;
    }
    let before = sample_stats();
    let start = Instant::now();
    let mut last = BenchOutcome::default();
    for _ in 0..iters {
        last = f().await;
    }
    let after = sample_stats();
    record_result(name, iters, start, before, after, &last)
}

#[tokio::main]
async fn main() -> Result<(), ruprizzle::Error> {
    let native = std::env::var("RUST_BENCH_DRIVER").is_ok_and(|v| v == "rusqlite");
    let suffix = if native { "&driver=rusqlite" } else { "" };
    let path = db_path();
    let url = format!("sqlite:///{}?mode=rwc{}", path, suffix);
    let pool = ruprizzle::connect(&url).await?;

    let count = SelectQuery::<User>::new(&pool).count().await?;
    assert_eq!(count, 1000, "expected 1000 users in bench.sqlite3");

    let mut results = Vec::new();

    // Query construction (no I/O).
    results.push(bench_sync("to_sql_select_by_pk", 100_000, || {
        let q = SelectQuery::<User>::new(&pool)
            .filter(USER_ID.eq(500i64))
            .limit(1)
            .offset(0);
        std::hint::black_box(q.to_sql());
        BenchOutcome::default()
    }));

    results.push(bench_sync("to_sql_select_filter_order", 100_000, || {
        let q = SelectQuery::<User>::new(&pool)
            .filter(USER_AGE.gt(18i64).and(USER_EMAIL.contains("@example.com")))
            .order_by(USER_AGE.asc())
            .order_by(USER_EMAIL.asc())
            .limit(1000)
            .offset(0);
        std::hint::black_box(q.to_sql());
        BenchOutcome::default()
    }));

    results.push(bench_sync("to_sql_select_in_list", 100_000, || {
        let ids: Vec<i64> = (1..=50).collect();
        let q = SelectQuery::<User>::new(&pool)
            .filter(USER_ID.in_set(ids))
            .order_by(USER_ID.asc())
            .limit(50);
        std::hint::black_box(q.to_sql());
        BenchOutcome::default()
    }));

    results.push(bench_sync("to_sql_select_complex_filter", 100_000, || {
        let q = SelectQuery::<User>::new(&pool)
            .filter(
                USER_AGE
                    .gt(18i64)
                    .and(USER_EMAIL.contains("example.com"))
                    .and(USER_ID.between(100i64, 900i64)),
            )
            .order_by(USER_AGE.asc())
            .order_by(USER_EMAIL.asc())
            .limit(100);
        std::hint::black_box(q.to_sql());
        BenchOutcome::default()
    }));

    results.push(bench_sync("to_sql_select_paginated", 100_000, || {
        let q = SelectQuery::<User>::new(&pool)
            .filter(USER_AGE.gt(18i64).and(USER_EMAIL.contains("example.com")))
            .order_by(USER_AGE.asc())
            .order_by(USER_EMAIL.asc())
            .limit(20)
            .offset(4000);
        std::hint::black_box(q.to_sql());
        BenchOutcome::default()
    }));

    // End-to-end: select by PK.
    let pool2 = pool.clone();
    results.push(
        bench_async("select_by_pk", 1000, move || {
            let pool = pool2.clone();
            async move {
                let counter = CountingExecutor::new(&pool);
                let row = SelectQuery::<User>::new(&counter)
                    .filter(USER_ID.eq(500i64))
                    .fetch_one()
                    .await
                    .expect("fetch one user");
                assert_eq!(row.id, 500);
                BenchOutcome {
                    rows: 1,
                    queries: counter.count(),
                }
            }
        })
        .await,
    );

    // End-to-end: find many 1000 rows.
    let pool2 = pool.clone();
    results.push(
        bench_async("find_many_1000", 50, move || {
            let pool = pool2.clone();
            async move {
                let counter = CountingExecutor::new(&pool);
                let rows = SelectQuery::<User>::new(&counter)
                    .fetch_all()
                    .await
                    .expect("fetch all users");
                assert_eq!(rows.len(), 1000);
                BenchOutcome {
                    rows: rows.len(),
                    queries: counter.count(),
                }
            }
        })
        .await,
    );

    // End-to-end: filtered + ordered.
    let pool2 = pool.clone();
    results.push(
        bench_async("find_filtered_ordered", 50, move || {
            let pool = pool2.clone();
            async move {
                let counter = CountingExecutor::new(&pool);
                let rows = SelectQuery::<User>::new(&counter)
                    .filter(USER_AGE.gt(18i64))
                    .order_by(USER_AGE.asc())
                    .order_by(USER_EMAIL.asc())
                    .fetch_all()
                    .await
                    .expect("fetch filtered users");
                assert!(rows.len() >= 980, "expected ~1000 users, got {}", rows.len());
                BenchOutcome {
                    rows: rows.len(),
                    queries: counter.count(),
                }
            }
        })
        .await,
    );

    // End-to-end: filtered + ordered + paginated.
    let pool2 = pool.clone();
    results.push(
        bench_async("find_filtered_paginated", 50, move || {
            let pool = pool2.clone();
            async move {
                let counter = CountingExecutor::new(&pool);
                let rows = SelectQuery::<User>::new(&counter)
                    .filter(USER_AGE.gt(18i64))
                    .order_by(USER_AGE.asc())
                    .order_by(USER_EMAIL.asc())
                    .limit(20)
                    .offset(500)
                    .fetch_all()
                    .await
                    .expect("fetch paginated users");
                assert_eq!(rows.len(), 20);
                BenchOutcome {
                    rows: rows.len(),
                    queries: counter.count(),
                }
            }
        })
        .await,
    );

    // End-to-end: IN list with 50 ids.
    let ids_50: Vec<i64> = (1..=50).collect();
    let pool2 = pool.clone();
    results.push(
        bench_async("find_in_list", 100, move || {
            let pool = pool2.clone();
            let ids = ids_50.clone();
            async move {
                let counter = CountingExecutor::new(&pool);
                let rows = SelectQuery::<User>::new(&counter)
                    .filter(USER_ID.in_set(ids))
                    .order_by(USER_ID.asc())
                    .fetch_all()
                    .await
                    .expect("fetch users by id list");
                assert_eq!(rows.len(), 50);
                BenchOutcome {
                    rows: rows.len(),
                    queries: counter.count(),
                }
            }
        })
        .await,
    );

    // End-to-end: complex filter with multiple parameters.
    let pool2 = pool.clone();
    results.push(
        bench_async("find_complex_filter", 50, move || {
            let pool = pool2.clone();
            async move {
                let counter = CountingExecutor::new(&pool);
                let rows = SelectQuery::<User>::new(&counter)
                    .filter(
                        USER_AGE
                            .gt(18i64)
                            .and(USER_EMAIL.contains("example.com"))
                            .and(USER_ID.between(100i64, 900i64)),
                    )
                    .order_by(USER_AGE.asc())
                    .order_by(USER_EMAIL.asc())
                    .limit(100)
                    .fetch_all()
                    .await
                    .expect("fetch complex filtered users");
                assert_eq!(rows.len(), 100);
                BenchOutcome {
                    rows: rows.len(),
                    queries: counter.count(),
                }
            }
        })
        .await,
    );

    // End-to-end: count with filter.
    let pool2 = pool.clone();
    results.push(
        bench_async("count_filtered", 100, move || {
            let pool = pool2.clone();
            async move {
                let counter = CountingExecutor::new(&pool);
                let count = SelectQuery::<User>::new(&counter)
                    .filter(USER_AGE.gt(18i64))
                    .count()
                    .await
                    .expect("count users");
                assert!(count >= 980, "expected ~1000 users, got {}", count);
                BenchOutcome {
                    rows: count as usize,
                    queries: counter.count(),
                }
            }
        })
        .await,
    );

    // End-to-end: exists with filter.
    let pool2 = pool.clone();
    results.push(
        bench_async("exists_filtered", 100, move || {
            let pool = pool2.clone();
            async move {
                let counter = CountingExecutor::new(&pool);
                let exists = SelectQuery::<User>::new(&counter)
                    .filter(USER_AGE.gt(18i64))
                    .exists()
                    .await
                    .expect("exists users");
                assert!(exists);
                BenchOutcome {
                    rows: if exists { 1 } else { 0 },
                    queries: counter.count(),
                }
            }
        })
        .await,
    );

    // End-to-end: include posts for all users.
    let pool2 = pool.clone();
    results.push(
        bench_async("include_posts", 10, move || {
            let pool = pool2.clone();
            async move {
                let counter = CountingExecutor::new(&pool);
                let rows = SelectQuery::<User>::new(&counter)
                    .include(IncludeList::new(
                        |u: &User| u.id,
                        |u, posts| u.posts = posts,
                        POST_AUTHOR_ID,
                        |p: &Post| p.author_id,
                    ))
                    .exec()
                    .await
                    .expect("fetch users with posts");
                assert_eq!(rows.len(), 1000);
                let total_posts: usize = rows.iter().map(|u| u.posts.get().len()).sum();
                assert_eq!(total_posts, 10000);
                BenchOutcome {
                    rows: rows.len(),
                    queries: counter.count(),
                }
            }
        })
        .await,
    );

    // End-to-end: include author for all posts.
    let pool2 = pool.clone();
    results.push(
        bench_async("include_author", 10, move || {
            let pool = pool2.clone();
            async move {
                let counter = CountingExecutor::new(&pool);
                let rows = SelectQuery::<Post>::new(&counter)
                    .include(IncludeOne::new(
                        |p: &Post| p.author_id,
                        |p, author| p.author = author,
                        USER_ID,
                        |u: &User| u.id,
                    ))
                    .exec()
                    .await
                    .expect("fetch posts with author");
                assert_eq!(rows.len(), 10000);
                BenchOutcome {
                    rows: rows.len(),
                    queries: counter.count(),
                }
            }
        })
        .await,
    );

    // End-to-end: include posts and their comments.
    let pool2 = pool.clone();
    results.push(
        bench_async("include_posts_and_comments", 10, move || {
            let pool = pool2.clone();
            async move {
                let counter = CountingExecutor::new(&pool);
                let rows = SelectQuery::<User>::new(&counter)
                    .include(
                        IncludeList::new(
                            |u: &User| u.id,
                            |u, posts| u.posts = posts,
                            POST_AUTHOR_ID,
                            |p: &Post| p.author_id,
                        )
                        .include(IncludeList::new(
                            |p: &Post| p.id,
                            |p, comments| p.comments = comments,
                            COMMENT_POST_ID,
                            |c: &Comment| c.post_id,
                        )),
                    )
                    .exec()
                    .await
                    .expect("fetch users with posts and comments");
                assert_eq!(rows.len(), 1000);
                let total_posts: usize = rows.iter().map(|u| u.posts.get().len()).sum();
                assert_eq!(total_posts, 10000);
                let total_comments: usize = rows
                    .iter()
                    .map(|u| {
                        u.posts
                            .get()
                            .iter()
                            .map(|p| p.comments.get().len())
                            .sum::<usize>()
                    })
                    .sum();
                assert_eq!(total_comments, 50000);
                BenchOutcome {
                    rows: rows.len(),
                    queries: counter.count(),
                }
            }
        })
        .await,
    );

    // End-to-end: posts with tags (many-to-many through post_tags).
    let pool2 = pool.clone();
    results.push(
        bench_async("include_posts_with_tags", 10, move || {
            let pool = pool2.clone();
            async move {
                let counter = CountingExecutor::new(&pool);
                let rows = SelectQuery::<Post>::new(&counter)
                    .include(
                        IncludeList::new(
                            |p: &Post| p.id,
                            |p, post_tags| p.post_tags = post_tags,
                            POST_TAG_POST_ID,
                            |pt: &PostTag| pt.post_id,
                        )
                        .include(IncludeOne::new(
                            |pt: &PostTag| pt.tag_id,
                            |pt, tag| pt.tag = tag,
                            TAG_ID,
                            |t: &Tag| t.id,
                        )),
                    )
                    .exec()
                    .await
                    .expect("fetch posts with tags");
                assert_eq!(rows.len(), 10000);
                let total_post_tags: usize = rows.iter().map(|p| p.post_tags.get().len()).sum();
                assert_eq!(total_post_tags, 30000);
                BenchOutcome {
                    rows: rows.len(),
                    queries: counter.count(),
                }
            }
        })
        .await,
    );

    // End-to-end: find popular posts (views > 1000) and order by views,
    // including the author to exercise a N:1 join on a filtered list.
    let pool2 = pool.clone();
    results.push(
        bench_async("find_popular_posts", 50, move || {
            let pool = pool2.clone();
            async move {
                let counter = CountingExecutor::new(&pool);
                let rows = SelectQuery::<Post>::new(&counter)
                    .filter(POST_VIEWS.gt(1000i64))
                    .include(IncludeOne::new(
                        |p: &Post| p.author_id,
                        |p, author| p.author = author,
                        USER_ID,
                        |u: &User| u.id,
                    ))
                    .order_by(POST_VIEWS.desc())
                    .limit(100)
                    .exec()
                    .await
                    .expect("fetch popular posts");
                assert_eq!(rows.len(), 100);
                BenchOutcome {
                    rows: rows.len(),
                    queries: counter.count(),
                }
            }
        })
        .await,
    );

    // Bulk insert 1000 rows into bench_bulk.
    let bulk_rows: Vec<_> = (0..1000)
        .map(|i| {
            [
                ("id", (i + 1i64).to_value()),
                ("name", format!("bulk-{i}").to_value()),
                ("n", (i * 3i64).to_value()),
            ]
        })
        .collect();

    pool.execute_raw(Cow::Borrowed("DELETE FROM bench_bulk"), Vec::new())
        .await?;

    let pool2 = pool.clone();
    results.push(
        bench_async("bulk_insert_1000", 10, move || {
            let pool = pool2.clone();
            let rows = bulk_rows.clone();
            async move {
                pool.execute_raw(Cow::Borrowed("DELETE FROM bench_bulk"), Vec::new())
                    .await
                    .expect("clear bench_bulk");
                let inserted = InsertManyQuery::<BenchBulk>::new(&pool)
                    .rows(rows.iter().map(|r| r.iter().cloned()))
                    .exec()
                    .await
                    .expect("bulk insert");
                assert_eq!(inserted.len(), 1000);
                BenchOutcome {
                    rows: inserted.len(),
                    queries: 2, // DELETE + one chunked INSERT (1000 rows fit under SQLite param limit)
                }
            }
        })
        .await,
    );

    println!("\n{}", serde_json::to_string_pretty(&results).unwrap());

    let filename = if native {
        "ruprizzle-rusqlite-results.json"
    } else {
        "ruprizzle-results.json"
    };
    let path = std::path::Path::new(&path)
        .parent()
        .unwrap()
        .join(filename);
    tokio::fs::write(&path, serde_json::to_string_pretty(&results).unwrap())
        .await
        .expect("write results");
    println!("Wrote {}", path.display());

    Ok(())
}
