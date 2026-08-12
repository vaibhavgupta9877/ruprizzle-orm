//! Breakdown of the `include_posts` path (1 000 users + 10 000 posts).
//!
//! `BenchmarkResults.md` records 16.2 ms for this shape. This splits that into
//! transport (what sqlx charges for 11 000 rows), the ORM's own grouping /
//! dedup / attach work, and the `Any` wrapper.

#![allow(dead_code)]

use std::collections::HashMap;
use std::time::Instant;

use ruprizzle::{Column, IncludeList, Model, Related, SelectQuery};
use sqlx::FromRow;

/// The cross-ORM benchmark database, as an absolute path.
///
/// Defaults to the checked-in benchmark database relative to the workspace
/// root; override with `RUPRIZZLE_BENCH_DB`.
fn db_path() -> String {
    let raw = std::env::var("RUPRIZZLE_BENCH_DB")
        .unwrap_or_else(|_| "local/cross-orm-bench/node/bench.sqlite3".to_owned());
    let Ok(abs) = std::fs::canonicalize(&raw) else {
        return raw;
    };
    // `canonicalize` yields a Windows extended-length path (`\\?\D:\...`);
    // SQLite wants forward slashes and no prefix.
    let abs = abs.to_string_lossy().replace('\\', "/");
    abs.strip_prefix("//?/").unwrap_or(&abs).to_owned()
}

#[derive(Debug, Clone, FromRow)]
struct User {
    id: i64,
    email: String,
    age: i64,
    #[sqlx(skip)]
    posts: Related<Vec<Post>>,
}

#[cfg(feature = "sqlite-rusqlite")]
impl ruprizzle::rusqlite::FromRusqliteRow for User {
    fn from_rusqlite_row(
        row: &ruprizzle::rusqlite::Row,
    ) -> Result<Self, ruprizzle::Error> {
        Ok(Self {
            id: row.get::<i64>(0)?,
            email: row.get::<String>(1)?,
            age: row.get::<i64>(2)?,
            posts: Related::default(),
        })
    }
}

impl Model for User {
    const TABLE: &'static str = "users";
}

#[derive(Debug, Clone, FromRow)]
struct Post {
    id: i64,
    author_id: i64,
    title: String,
}

#[cfg(feature = "sqlite-rusqlite")]
impl ruprizzle::rusqlite::FromRusqliteRow for Post {
    fn from_rusqlite_row(
        row: &ruprizzle::rusqlite::Row,
    ) -> Result<Self, ruprizzle::Error> {
        Ok(Self {
            id: row.get::<i64>(0)?,
            author_id: row.get::<i64>(1)?,
            title: row.get::<String>(2)?,
        })
    }
}

impl Model for Post {
    const TABLE: &'static str = "posts";
}

const POST_AUTHOR_ID: Column<Post, i64> = Column::new("posts", "author_id");

fn posts_include() -> IncludeList<'static, User, Post, i64, ()> {
    IncludeList::new(|u| u.id, |u, p| u.posts = p, POST_AUTHOR_ID, |p| p.author_id)
}

async fn bench<F, Fut>(label: &str, iters: u32, mut f: F) -> f64
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = usize>,
{
    let mut checksum = 0usize;
    for _ in 0..2.min(iters) {
        checksum += f().await;
    }
    let start = Instant::now();
    for _ in 0..iters {
        checksum += f().await;
    }
    let ms = start.elapsed().as_secs_f64() * 1e3 / f64::from(iters);
    println!("{label:<52} {ms:>8.2} ms/op  ({checksum})");
    ms
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    sqlx::any::install_default_drivers();
    let url = format!("sqlite:///{}?mode=ro", db_path());
    let any = ruprizzle::connect(&url).await?;
    let native = sqlx::SqlitePool::connect_with(
        db_path().parse::<sqlx::sqlite::SqliteConnectOptions>()?.read_only(true),
    )
    .await?;

    println!("\n--- floors ---");
    let t_native = bench("A native sqlx: 2 queries, no grouping", 30, || async {
        let u: Vec<(i64, String, i64)> =
            sqlx::query_as("SELECT id, email, age FROM users").fetch_all(&native).await.unwrap();
        let p: Vec<(i64, i64, String)> = sqlx::query_as("SELECT id, author_id, title FROM posts")
            .fetch_all(&native)
            .await
            .unwrap();
        u.len() + p.len()
    })
    .await;

    let t_any = bench("B Any driver: 2 queries, no grouping", 30, || async {
        let u: Vec<User> = sqlx::query_as::<sqlx::Any, User>("SELECT id, email, age FROM users")
            .fetch_all(&any)
            .await
            .unwrap();
        let p: Vec<Post> =
            sqlx::query_as::<sqlx::Any, Post>("SELECT id, author_id, title FROM posts")
                .fetch_all(&any)
                .await
                .unwrap();
        u.len() + p.len()
    })
    .await;

    let t_manual = bench("C B + hand-rolled HashMap group + attach", 30, || async {
        let mut u: Vec<User> = sqlx::query_as::<sqlx::Any, User>("SELECT id, email, age FROM users")
            .fetch_all(&any)
            .await
            .unwrap();
        let p: Vec<Post> =
            sqlx::query_as::<sqlx::Any, Post>("SELECT id, author_id, title FROM posts")
                .fetch_all(&any)
                .await
                .unwrap();
        let mut map: HashMap<i64, Vec<Post>> = HashMap::with_capacity(u.len());
        for post in p {
            map.entry(post.author_id).or_default().push(post);
        }
        let mut n = 0;
        for user in &mut u {
            let v = map.remove(&user.id).unwrap_or_default();
            n += v.len();
            user.posts = Related::Loaded(v);
        }
        n
    })
    .await;

    // Sanity-check the include actually attaches before timing it.
    {
        let users = SelectQuery::<User>::new(&any)
            .include(posts_include())
            .exec()
            .await
            .unwrap();
        let loaded = users.iter().filter(|u| u.posts.is_loaded()).count();
        let attached: usize = users.iter().map(|u| u.posts.try_get().map_or(0, Vec::len)).sum();
        println!("  [sanity] users={} loaded={} attached_posts={}", users.len(), loaded, attached);
    }

    let t_rz = bench("D ruprizzle .include(posts())", 30, || async {
        let users = SelectQuery::<User>::new(&any)
            .include(posts_include())
            .exec()
            .await
            .unwrap();
        users.iter().map(|u| u.posts.try_get().map_or(0, Vec::len)).sum()
    })
    .await;

    println!("\n--- attribution ---");
    println!("  transport floor (A)                {t_native:>8.2} ms");
    println!("  Any wrapper     (B-A)              {:>8.2} ms", t_any - t_native);
    println!("  group + attach  (C-B)              {:>8.2} ms", t_manual - t_any);
    println!("  ruprizzle extra (D-C)              {:>8.2} ms", t_rz - t_manual);
    println!("  ---");
    println!("  ruprizzle total (D)                {t_rz:>8.2} ms");
    println!("  headroom above hand-written (D-C)  {:>8.2} ms  ({:.0}%)",
        t_rz - t_manual, (t_rz / t_manual - 1.0) * 100.0);

    // What does the IN-list / dedup path cost on its own?
    println!("\n--- the 1000-key IN list the loader builds ---");
    let keys: Vec<i64> = (1..=1000).collect();
    let start = Instant::now();
    let mut total = 0usize;
    for _ in 0..1000 {
        let mut seen = std::collections::HashSet::with_capacity(keys.len());
        let d: Vec<i64> = keys.iter().copied().filter(|k| seen.insert(*k)).collect();
        total += d.len();
    }
    println!("  dedup 1000 keys                    {:>8.2} us  ({total})",
        start.elapsed().as_secs_f64() * 1e6 / 1000.0);

    let sq = SelectQuery::<Post>::new(&any).filter(POST_AUTHOR_ID.in_set(keys.clone()));
    let start = Instant::now();
    for _ in 0..1000 {
        std::hint::black_box(sq.to_sql());
    }
    println!("  compile IN(1000) to SQL            {:>8.2} us",
        start.elapsed().as_secs_f64() * 1e6 / 1000.0);

    let compiled = sq.to_sql();
    println!("  -> sql len {} bytes, {} binds", compiled.sql.len(), compiled.binds.len());

    Ok(())
}
