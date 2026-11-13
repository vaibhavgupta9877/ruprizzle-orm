//! Phase breakdown of `include_posts` on the native rusqlite backend
//! (1,000 users, 10,000 posts).
//!
//! The full `exec()` call is split into the phases the include loader runs:
//!
//! 1. parent fetch (`users`)
//! 2. child fetch (`posts`, the full-table fast path)
//! 3. grouping children by parent key
//! 4. the whole `exec()` for comparison
//!
//! Each figure is the minimum and median over repeats. Noise on a shared
//! machine only adds time, so the minimum is the closest estimate of cost.
//!
//! Run:
//! ```text
//! cargo run --release -p ruprizzle --features sqlite-rusqlite --example include_phases
//! ```

#![cfg(feature = "sqlite-rusqlite")]
// The models mirror the benchmark structs field-for-field so their size matches;
// several fields exist only for that and are never read.
#![allow(dead_code)]

use std::collections::HashMap;
use std::time::Instant;

use ruprizzle::{Column, IncludeList, Model, PoolConfig, Related, SelectQuery, connect_with};

const REPEATS: usize = 7;
const ITERS: usize = 10;

#[derive(Debug, Clone, Default, sqlx::FromRow)]
struct User {
    id: i64,
    email: String,
    age: i64,
    name: String,
    created_at: i64,
    #[sqlx(skip)]
    posts: Related<Vec<Post>>,
}

impl Model for User {
    const TABLE: &'static str = "users";
    const PRIMARY_KEY: &'static str = "id";
}

#[cfg(feature = "postgres-tokio-postgres")]
ruprizzle::tokio_postgres_default_row!(User);

impl ruprizzle::rusqlite::FromRusqliteRow for User {
    fn from_rusqlite_row(row: &ruprizzle::rusqlite::RusqliteRow) -> Result<Self, ruprizzle::Error> {
        Ok(Self {
            id: ruprizzle::rusqlite::get_i64(row, 0)?,
            email: ruprizzle::rusqlite::get_text(row, 1)?,
            age: ruprizzle::rusqlite::get_i64(row, 2)?,
            name: ruprizzle::rusqlite::get_text(row, 3)?,
            created_at: ruprizzle::rusqlite::get_i64(row, 4)?,
            posts: Related::default(),
        })
    }
}

impl ruprizzle::rusqlite::FromOwnedRow for User {
    fn from_owned_row(row: &ruprizzle::rusqlite::Row) -> Result<Self, ruprizzle::Error> {
        Ok(Self {
            id: row.get::<i64>(0)?,
            email: row.get::<String>(1)?,
            age: row.get::<i64>(2)?,
            name: row.get::<String>(3)?,
            created_at: row.get::<i64>(4)?,
            posts: Related::default(),
        })
    }
}

#[derive(Debug, Clone, Default, sqlx::FromRow)]
struct Post {
    id: i64,
    author_id: i64,
    category_id: i64,
    title: String,
    published_at: i64,
    views: i64,
    // Same relation fields as the benchmark's Post: the Related payloads are
    // stored inline, so they make every Post much larger than its columns.
    #[sqlx(skip)]
    author: Related<Option<User>>,
    #[sqlx(skip)]
    category: Related<Option<Category>>,
    #[sqlx(skip)]
    comments: Related<Vec<Comment>>,
    #[sqlx(skip)]
    likes: Related<Vec<Like>>,
    #[sqlx(skip)]
    post_tags: Related<Vec<PostTag>>,
}

#[derive(Debug, Clone, Default, sqlx::FromRow)]
struct Category {
    id: i64,
    name: String,
}

#[derive(Debug, Clone, Default, sqlx::FromRow)]
struct Comment {
    id: i64,
}

#[derive(Debug, Clone, Default, sqlx::FromRow)]
struct Like {
    id: i64,
}

#[derive(Debug, Clone, Default, sqlx::FromRow)]
struct PostTag {
    post_id: i64,
}

impl Model for Post {
    const TABLE: &'static str = "posts";
    const PRIMARY_KEY: &'static str = "id";
}

#[cfg(feature = "postgres-tokio-postgres")]
ruprizzle::tokio_postgres_default_row!(Post);

impl ruprizzle::rusqlite::FromRusqliteRow for Post {
    fn from_rusqlite_row(row: &ruprizzle::rusqlite::RusqliteRow) -> Result<Self, ruprizzle::Error> {
        Ok(Self {
            id: ruprizzle::rusqlite::get_i64(row, 0)?,
            author_id: ruprizzle::rusqlite::get_i64(row, 1)?,
            category_id: ruprizzle::rusqlite::get_i64(row, 2)?,
            title: ruprizzle::rusqlite::get_text(row, 3)?,
            published_at: ruprizzle::rusqlite::get_i64(row, 4)?,
            views: ruprizzle::rusqlite::get_i64(row, 5)?,
            author: Related::default(),
            category: Related::default(),
            comments: Related::default(),
            likes: Related::default(),
            post_tags: Related::default(),
        })
    }
}

impl ruprizzle::rusqlite::FromOwnedRow for Post {
    fn from_owned_row(row: &ruprizzle::rusqlite::Row) -> Result<Self, ruprizzle::Error> {
        Ok(Self {
            id: row.get::<i64>(0)?,
            author_id: row.get::<i64>(1)?,
            category_id: row.get::<i64>(2)?,
            title: row.get::<String>(3)?,
            published_at: row.get::<i64>(4)?,
            views: row.get::<i64>(5)?,
            author: Related::default(),
            category: Related::default(),
            comments: Related::default(),
            likes: Related::default(),
            post_tags: Related::default(),
        })
    }
}

const POST_AUTHOR_ID: Column<Post, i64> = Column::new("posts", "author_id");

#[derive(Clone, Copy)]
struct Timing {
    min: f64,
    median: f64,
}

fn summarise(mut v: Vec<f64>) -> Timing {
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    Timing {
        min: v[0],
        median: v[v.len() / 2],
    }
}

async fn time_async<F, Fut>(mut f: F) -> Timing
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = ()>,
{
    for _ in 0..2 {
        f().await;
    }
    let mut samples = Vec::new();
    for _ in 0..REPEATS {
        let start = Instant::now();
        for _ in 0..ITERS {
            f().await;
        }
        samples.push(start.elapsed().as_secs_f64() * 1e6 / ITERS as f64);
    }
    summarise(samples)
}

fn row(label: &str, t: Timing) {
    println!(
        "  {label:<46} min {:>10.1}   median {:>10.1}",
        t.min, t.median
    );
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::var("BENCH_SQLITE_PATH").unwrap_or_else(|_| {
        concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../local/cross-orm-bench/node/bench.sqlite3"
        )
        .to_string()
    });
    let file = path.replace('\\', "/");
    let url = format!("sqlite:///{file}?mode=rwc&driver=rusqlite");
    let mut config = PoolConfig::default();
    config.max_connections = 8;
    let pool = connect_with(&url, &config).await?;

    // Phase 1: parent fetch.
    let parent = time_async(|| async {
        std::hint::black_box(
            SelectQuery::<User>::new(&pool)
                .fetch_all_blocking()
                .unwrap(),
        );
    })
    .await;

    // Phase 2: child fetch (what the full-table fast path does).
    let child = time_async(|| async {
        std::hint::black_box(
            SelectQuery::<Post>::new(&pool)
                .fetch_all_blocking()
                .unwrap(),
        );
    })
    .await;

    // Phase 3: grouping only, on pre-fetched data. Mirrors include.rs:
    // a HashMap from parent key to parent positions, then one bucket per
    // parent filled by walking the children.
    let users = SelectQuery::<User>::new(&pool).fetch_all_blocking()?;
    let posts = SelectQuery::<Post>::new(&pool).fetch_all_blocking()?;
    let grouping = time_async(|| {
        let users = &users;
        let posts = posts.clone();
        async move {
            let mut parent_index: HashMap<i64, Vec<usize>> = HashMap::with_capacity(users.len());
            for (i, u) in users.iter().enumerate() {
                parent_index.entry(u.id).or_default().push(i);
            }
            let bucket_hint = posts.len() / users.len();
            let mut buckets: Vec<Vec<Post>> =
                std::iter::repeat_with(|| Vec::with_capacity(bucket_hint))
                    .take(users.len())
                    .collect();
            for child in posts {
                if let Some(indices) = parent_index.get(&child.author_id) {
                    if let Some(&first) = indices.first() {
                        buckets[first].push(child);
                    }
                }
            }
            std::hint::black_box(buckets);
        }
    })
    .await;

    // Phase 4: the whole include, as the benchmark runs it.
    let full = time_async(|| async {
        let rows = SelectQuery::<User>::new(&pool)
            .include(IncludeList::new(
                |u: &User| u.id,
                |u, posts| u.posts = posts,
                POST_AUTHOR_ID,
                |p: &Post| p.author_id,
            ))
            .exec()
            .await
            .unwrap();
        std::hint::black_box(rows);
    })
    .await;

    println!(
        "size_of Post = {} bytes, size_of User = {} bytes",
        std::mem::size_of::<Post>(),
        std::mem::size_of::<User>()
    );
    println!("include_posts phases (1,000 users, 10,000 posts), us per call");
    row("1 parent fetch (users, sync)", parent);
    row("2 child fetch (posts, sync)", child);
    row("3 grouping only (pre-fetched data)", grouping);
    row("4 full include exec() (async)", full);
    println!(
        "  unexplained by 1+2+3: {:+.1} us (min)",
        full.min - parent.min - child.min - grouping.min
    );
    Ok(())
}
