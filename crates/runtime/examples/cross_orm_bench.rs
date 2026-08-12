//! Cross-ORM SQLite benchmark for ruprizzle.
//!
//! Compares against the same `bench.sqlite3` file used by the Prisma and
//! Drizzle benchmarks in `local/cross-orm-bench/node`.

use std::time::Instant;

use ruprizzle::{Column, Encodable, IncludeList, InsertManyQuery, Model, Related, SelectQuery};
use ruprizzle::serde::Serialize;
use ruprizzle::serde_json;
use sqlx::FromRow;

const DB_PATH: &str = "D:/SaaS/rust/ruprizzle-orm/local/cross-orm-bench/node/bench.sqlite3";

#[derive(Debug, Clone, FromRow, Serialize)]
struct User {
    id: i64,
    email: String,
    age: i64,
    #[serde(skip)]
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

#[derive(Debug, Clone, FromRow, Serialize)]
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

#[derive(Debug, Clone, FromRow, Serialize)]
struct BenchBulk {
    id: i64,
    name: String,
    n: i64,
}

#[cfg(feature = "sqlite-rusqlite")]
impl ruprizzle::rusqlite::FromRusqliteRow for BenchBulk {
    fn from_rusqlite_row(
        row: &ruprizzle::rusqlite::Row,
    ) -> Result<Self, ruprizzle::Error> {
        Ok(Self {
            id: row.get::<i64>(0)?,
            name: row.get::<String>(1)?,
            n: row.get::<i64>(2)?,
        })
    }
}

impl Model for BenchBulk {
    const TABLE: &'static str = "bench_bulk";
}

const USER_ID: Column<User, i64> = Column::new("users", "id");
const USER_EMAIL: Column<User, String> = Column::new("users", "email");
const USER_AGE: Column<User, i64> = Column::new("users", "age");

const POST_AUTHOR_ID: Column<Post, i64> = Column::new("posts", "author_id");

fn posts() -> IncludeList<'static, User, Post, i64, ()> {
    IncludeList::new(
        |u| u.id,
        |u, posts| u.posts = posts,
        POST_AUTHOR_ID,
        |p| p.author_id,
    )
}

#[derive(Serialize)]
struct BenchResult {
    orm: String,
    operation: String,
    iters: u32,
    total_ms: f64,
    avg_ms: f64,
}

fn record_result(name: &str, iters: u32, total: Instant) -> BenchResult {
    let total_ms = total.elapsed().as_secs_f64() * 1000.0;
    let avg_ms = total_ms / iters as f64;
    println!("{:>28} {:>10.3} us/op  (total {:>7.1} ms, {} iters)", name, avg_ms * 1000.0, total_ms, iters);
    BenchResult {
        orm: "ruprizzle".to_string(),
        operation: name.to_string(),
        iters,
        total_ms,
        avg_ms,
    }
}

fn bench_sync<F>(name: &str, iters: u32, mut f: F) -> BenchResult
where
    F: FnMut(),
{
    for _ in 0..3.min(iters) {
        f();
    }
    let start = Instant::now();
    for _ in 0..iters {
        f();
    }
    record_result(name, iters, start)
}

async fn bench_async<F, Fut>(name: &str, iters: u32, mut f: F) -> BenchResult
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = ()>,
{
    for _ in 0..3.min(iters) {
        f().await;
    }
    let start = Instant::now();
    for _ in 0..iters {
        f().await;
    }
    record_result(name, iters, start)
}

#[tokio::main]
async fn main() -> Result<(), ruprizzle::Error> {
    let url = format!("sqlite:///{}?mode=rwc", DB_PATH);
    let pool = ruprizzle::connect(&url).await?;

    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users")
        .fetch_one(&pool)
        .await?;
    assert_eq!(count, 1000, "expected 1000 users in bench.sqlite3");

    let mut results = Vec::new();

    // Query construction (no I/O).
    results.push(bench_sync("to_sql_select_by_pk", 100_000, || {
        let q = SelectQuery::<User>::new(&pool)
            .filter(USER_ID.eq(500i64))
            .limit(1)
            .offset(0);
        std::hint::black_box(q.to_sql());
    }));

    results.push(bench_sync("to_sql_select_filter_order", 100_000, || {
        let q = SelectQuery::<User>::new(&pool)
            .filter(USER_AGE.gt(18i64).and(USER_EMAIL.contains("@example.com")))
            .order_by(USER_AGE.asc())
            .order_by(USER_EMAIL.asc())
            .limit(1000)
            .offset(0);
        std::hint::black_box(q.to_sql());
    }));

    // End-to-end: select by PK.
    let pool2 = pool.clone();
    results.push(bench_async("select_by_pk", 1000, move || {
        let pool = pool2.clone();
        async move {
            let row = SelectQuery::<User>::new(&pool)
                .filter(USER_ID.eq(500i64))
                .fetch_one()
                .await
                .expect("fetch one user");
            assert_eq!(row.id, 500);
            std::hint::black_box(row);
        }
    }).await);

    // End-to-end: find many 1000 rows.
    let pool2 = pool.clone();
    results.push(bench_async("find_many_1000", 50, move || {
        let pool = pool2.clone();
        async move {
            let rows = SelectQuery::<User>::new(&pool)
                .fetch_all()
                .await
                .expect("fetch all users");
            assert_eq!(rows.len(), 1000);
            std::hint::black_box(rows);
        }
    }).await);

    // End-to-end: filtered + ordered.
    let pool2 = pool.clone();
    results.push(bench_async("find_filtered_ordered", 50, move || {
        let pool = pool2.clone();
        async move {
            let rows = SelectQuery::<User>::new(&pool)
                .filter(USER_AGE.gt(18i64))
                .order_by(USER_AGE.asc())
                .order_by(USER_EMAIL.asc())
                .fetch_all()
                .await
                .expect("fetch filtered users");
            assert!(rows.len() >= 980, "expected ~1000 users, got {}", rows.len());
            std::hint::black_box(rows);
        }
    }).await);

    // End-to-end: include posts for all users.
    let pool2 = pool.clone();
    results.push(bench_async("include_posts", 10, move || {
        let pool = pool2.clone();
        async move {
            let rows = SelectQuery::<User>::new(&pool)
                .include(posts())
                .exec()
                .await
                .expect("fetch users with posts");
            assert_eq!(rows.len(), 1000);
            let total_posts: usize = rows.iter().map(|u| u.posts.get().len()).sum();
            assert_eq!(total_posts, 10000);
            std::hint::black_box(rows);
        }
    }).await);

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

    sqlx::query("DELETE FROM bench_bulk").execute(&pool).await?;

    let pool2 = pool.clone();
    results.push(bench_async("bulk_insert_1000", 10, move || {
        let pool = pool2.clone();
        let rows = bulk_rows.clone();
        async move {
            sqlx::query("DELETE FROM bench_bulk")
                .execute(&pool)
                .await
                .expect("clear bench_bulk");
            let inserted = InsertManyQuery::<BenchBulk>::new(&pool)
                .rows(rows.iter().map(|r| r.iter().cloned()))
                .exec()
                .await
                .expect("bulk insert");
            assert_eq!(inserted.len(), 1000);
            std::hint::black_box(inserted);
        }
    }).await);

    println!("\n{}", serde_json::to_string_pretty(&results).unwrap());

    let path = std::path::Path::new(DB_PATH).parent().unwrap().join("ruprizzle-results.json");
    tokio::fs::write(&path, serde_json::to_string_pretty(&results).unwrap())
        .await
        .expect("write results");
    println!("Wrote {}", path.display());

    Ok(())
}
