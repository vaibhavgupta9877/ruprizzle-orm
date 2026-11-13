//! Where does `find_many_1000` (SQLite, native rusqlite pool) spend its time?
//!
//! Decomposes one 1,000-row read of `users` into layers, so each cost can be
//! attributed:
//!
//! 1. raw `rusqlite`, inline on this thread  (the floor)
//! 2. raw `rusqlite`, through `spawn_blocking` (the async hop alone)
//! 3. `SelectQuery::fetch_all` on the native pool (what ruprizzle adds)
//! 4. `SelectQuery::to_sql` alone (per-call SQL compilation)
//!
//! Run:
//! ```text
//! cargo run --release -p ruprizzle --features sqlite-rusqlite --example sqlite_breakdown
//! ```
//! `BENCH_SQLITE_PATH` selects the database (defaults to the checked-in bench db).

// The models mirror the benchmark structs; their fields are decoded for the
// measurement and never read afterwards.
#![allow(dead_code)]

use std::time::Instant;

use ruprizzle::{Model, PoolConfig, SelectQuery, connect_with};

const REPEATS: usize = 7;
const ITERS: usize = 200;

#[derive(Debug, Clone, Default, sqlx::FromRow)]
struct User {
    id: i64,
    email: String,
    age: i64,
    name: String,
    created_at: i64,
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
        })
    }
}

const ID: ruprizzle::Column<User, i64> = ruprizzle::Column::new("users", "id");

#[derive(Debug, Clone, Default, sqlx::FromRow)]
struct Post {
    id: i64,
    author_id: i64,
    category_id: i64,
    title: String,
    published_at: i64,
    views: i64,
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
        })
    }
}

/// Reads all posts with raw rusqlite, inline. The floor for a posts decode.
fn raw_posts(conn: &rusqlite::Connection) -> Vec<Post> {
    let mut stmt = conn
        .prepare_cached("SELECT id, author_id, category_id, title, published_at, views FROM posts")
        .expect("prepare");
    stmt.query_map([], |row| {
        Ok(Post {
            id: row.get(0)?,
            author_id: row.get(1)?,
            category_id: row.get(2)?,
            title: row.get(3)?,
            published_at: row.get(4)?,
            views: row.get(5)?,
        })
    })
    .expect("query")
    .collect::<Result<Vec<_>, _>>()
    .expect("rows")
}

const SQL: &str = "SELECT id, email, age, name, created_at FROM users";

fn db_path() -> String {
    std::env::var("BENCH_SQLITE_PATH").unwrap_or_else(|_| {
        concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../local/cross-orm-bench/node/bench.sqlite3"
        )
        .to_string()
    })
}

fn raw_inline(conn: &rusqlite::Connection) -> Vec<User> {
    let mut stmt = conn.prepare_cached(SQL).expect("prepare");
    stmt.query_map([], |row| {
        Ok(User {
            id: row.get(0)?,
            email: row.get(1)?,
            age: row.get(2)?,
            name: row.get(3)?,
            created_at: row.get(4)?,
        })
    })
    .expect("query")
    .collect::<Result<Vec<_>, _>>()
    .expect("rows")
}

fn raw_point(conn: &rusqlite::Connection, id: i64) -> User {
    let mut stmt = conn
        .prepare_cached("SELECT id, email, age, name, created_at FROM users WHERE id = ?1")
        .expect("prepare");
    stmt.query_row([id], |row| {
        Ok(User {
            id: row.get(0)?,
            email: row.get(1)?,
            age: row.get(2)?,
            name: row.get(3)?,
            created_at: row.get(4)?,
        })
    })
    .expect("row")
}

fn row(label: &str, t: Timing) {
    println!(
        "  {label:<40} min {:>9.2}   median {:>9.2}",
        t.min, t.median
    );
}

/// Noise on a shared machine only ever adds time, so the minimum across
/// repeats is the closest estimate of the true cost; the median is shown too.
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

fn time_per_call_us(mut f: impl FnMut()) -> Timing {
    for _ in 0..10 {
        f();
    }
    let mut samples = Vec::new();
    for _ in 0..REPEATS {
        let start = Instant::now();
        for _ in 0..ITERS {
            f();
        }
        samples.push(start.elapsed().as_secs_f64() * 1e6 / ITERS as f64);
    }
    summarise(samples)
}

#[cfg(feature = "sqlite-rusqlite")]
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = db_path();
    let file = path.replace('\\', "/");
    let url = format!("sqlite:///{file}?mode=rwc&driver=rusqlite");
    let mut config = PoolConfig::default();
    config.max_connections = 8;
    let pool = connect_with(&url, &config).await?;

    let conn =
        rusqlite::Connection::open_with_flags(&path, rusqlite::OpenFlags::SQLITE_OPEN_READ_WRITE)?;
    conn.pragma_update(None, "journal_mode", "WAL")?;
    // Mirror the pool connection settings so the floor is measured on an
    // equivalent connection (see rusqlite.rs `apply_pragmas`).
    if std::env::var("BREAKDOWN_POOL_PRAGMAS").is_ok() {
        conn.pragma_update(None, "synchronous", "NORMAL")?;
        conn.pragma_update(None, "temp_store", "MEMORY")?;
        conn.pragma_update(None, "cache_size", -64_000i64)?;
        conn.pragma_update(None, "mmap_size", 268_435_456i64)?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        conn.busy_timeout(std::time::Duration::from_secs(60))?;
        conn.set_prepared_statement_cache_capacity(256);
    }
    assert_eq!(raw_inline(&conn).len(), 1000, "expected 1000 users");

    // Async timing: median and min over REPEATS, ITERS calls each.
    async fn time_async<F, Fut>(mut f: F) -> Timing
    where
        F: FnMut() -> Fut,
        Fut: std::future::Future<Output = ()>,
    {
        for _ in 0..10 {
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

    let p = |label: &str, t: Timing| {
        println!(
            "  {label:<34} min {:>9.2}   median {:>9.2}",
            t.min, t.median
        )
    };

    // ---- find_many_1000 ----
    let floor = time_per_call_us(|| {
        std::hint::black_box(raw_inline(&conn));
    });
    let orm = time_async(|| async {
        std::hint::black_box(SelectQuery::<User>::new(&pool).fetch_all().await.unwrap());
    })
    .await;
    let compile = time_per_call_us(|| {
        std::hint::black_box(SelectQuery::<User>::new(&pool).to_sql().unwrap());
    });
    let hop = time_async(|| async {
        std::hint::black_box(tokio::task::spawn_blocking(|| 0_u8).await.unwrap());
    })
    .await;

    println!("find_many_1000 (1000 rows), us per call");
    p("raw rusqlite inline (floor)", floor);
    p("ruprizzle SelectQuery::fetch_all", orm);
    p("ruprizzle to_sql alone", compile);
    p("spawn_blocking hop (no SQL)", hop);
    println!(
        "  ORM over floor, min: {:+.2} us ({:+.1}%)",
        orm.min - floor.min,
        (orm.min - floor.min) / floor.min * 100.0
    );

    // ---- select_by_pk ----
    // Raw statement executed on a blocking thread: isolates "the statement
    // runs on another thread" from everything the pool does.
    let shared = std::sync::Arc::new(std::sync::Mutex::new(conn));
    let point_raw_blocking = time_async(|| {
        let shared = shared.clone();
        async move {
            let user = tokio::task::spawn_blocking(move || {
                let c = shared.lock().unwrap();
                raw_point(&c, 500)
            })
            .await
            .unwrap();
            std::hint::black_box(user);
        }
    })
    .await;
    let conn = std::sync::Arc::try_unwrap(shared)
        .unwrap_or_else(|_| panic!("sole owner"))
        .into_inner()
        .unwrap();

    let point_raw = time_per_call_us(|| {
        std::hint::black_box(raw_point(&conn, 500));
    });
    let point_orm = time_async(|| async {
        std::hint::black_box(
            SelectQuery::<User>::new(&pool)
                .filter(ID.eq(500_i64))
                .fetch_one()
                .await
                .unwrap(),
        );
    })
    .await;
    let point_compile = time_per_call_us(|| {
        std::hint::black_box(
            SelectQuery::<User>::new(&pool)
                .filter(ID.eq(500_i64))
                .to_sql()
                .unwrap(),
        );
    });

    // Pool layers without SelectQuery: dispatch and checkout only.
    use ruprizzle::Executor;
    let pool_exec_select1 = time_async(|| async {
        std::hint::black_box(
            pool.fetch_all_raw("SELECT 1".into(), Vec::new())
                .await
                .unwrap(),
        );
    })
    .await;
    let pool_fetch_raw = time_async(|| async {
        std::hint::black_box(
            pool.fetch_all_raw(
                "SELECT id, email, age, name, created_at FROM users WHERE id = ?".into(),
                vec![ruprizzle::Value::I64(500)],
            )
            .await
            .unwrap(),
        );
    })
    .await;

    println!();
    println!("select_by_pk (1 row), us per call");
    row("pool fetch_all_raw SELECT 1", pool_exec_select1);
    row("pool fetch_all_raw (same SQL, Vec binds)", pool_fetch_raw);
    row("raw statement on spawn_blocking", point_raw_blocking);
    p("raw rusqlite inline (floor)", point_raw);
    p("ruprizzle SelectQuery::fetch_one", point_orm);
    p("ruprizzle to_sql alone", point_compile);
    p("spawn_blocking hop (no SQL)", hop);
    println!(
        "  ORM over floor, min: {:+.2} us ({:+.1}%)",
        point_orm.min - point_raw.min,
        (point_orm.min - point_raw.min) / point_raw.min * 100.0
    );

    // ---- posts: 10,000 rows, the dominant cost of include_posts ----
    const POST_ITERS: usize = 20;
    async fn time_posts<F, Fut>(mut f: F) -> Timing
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
            for _ in 0..POST_ITERS {
                f().await;
            }
            samples.push(start.elapsed().as_secs_f64() * 1e6 / POST_ITERS as f64);
        }
        summarise(samples)
    }
    let posts_raw = {
        let mut samples = Vec::new();
        for _ in 0..2 {
            std::hint::black_box(raw_posts(&conn));
        }
        for _ in 0..REPEATS {
            let start = Instant::now();
            for _ in 0..POST_ITERS {
                std::hint::black_box(raw_posts(&conn));
            }
            samples.push(start.elapsed().as_secs_f64() * 1e6 / POST_ITERS as f64);
        }
        summarise(samples)
    };
    let posts_sync = {
        let mut samples = Vec::new();
        for _ in 0..2 {
            std::hint::black_box(
                SelectQuery::<Post>::new(&pool)
                    .fetch_all_blocking()
                    .unwrap(),
            );
        }
        for _ in 0..REPEATS {
            let start = Instant::now();
            for _ in 0..POST_ITERS {
                std::hint::black_box(
                    SelectQuery::<Post>::new(&pool)
                        .fetch_all_blocking()
                        .unwrap(),
                );
            }
            samples.push(start.elapsed().as_secs_f64() * 1e6 / POST_ITERS as f64);
        }
        summarise(samples)
    };
    let posts_async = time_posts(|| async {
        std::hint::black_box(SelectQuery::<Post>::new(&pool).fetch_all().await.unwrap());
    })
    .await;

    println!();
    println!("posts, 10,000 rows, us per call");
    row("raw rusqlite inline (floor)", posts_raw);
    row("ruprizzle SelectQuery::fetch_all_blocking", posts_sync);
    row("ruprizzle SelectQuery::fetch_all (async)", posts_async);
    Ok(())
}
