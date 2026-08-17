//! Second-round bottleneck attribution, after the Phase 1 / P2-3 fixes.
//!
//! `layer_attribution` answered "is it the ORM or is it sqlx?" (answer: sqlx).
//! This one asks the follow-up: *inside* the sqlx path, which specific choices
//! ruprizzle makes are costing time, and how much is left on the table without
//! leaving `sqlx::Any`.
//!
//! Five experiments:
//!
//! 1. `test_before_acquire` — ruprizzle's `PoolConfig` now defaults it to
//!    `false`; this arm measures the cost of setting it back to `true`.
//! 2. pool checkout — per-query `acquire()` versus a held connection.
//! 3. `AnyRow` conversion — the same rows fetched natively and through `Any`,
//!    with no decoding at all, split by whether a text column is present.
//! 4. materialise-then-decode — ruprizzle's `Executor` returns `Vec<AnyRow>`
//!    and decodes in a second pass; compare with decoding as rows arrive.
//! 5. include path — where the 16.7 ms of `include_posts` actually goes.

#![allow(dead_code)]

use std::time::Instant;

use sqlx::any::AnyPoolOptions;
use sqlx::{FromRow, Row};

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
}

async fn bench<F, Fut>(label: &str, iters: u32, mut f: F) -> f64
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = usize>,
{
    let mut checksum = 0usize;
    for _ in 0..3.min(iters) {
        checksum += f().await;
    }
    let start = Instant::now();
    for _ in 0..iters {
        checksum += f().await;
    }
    let us = start.elapsed().as_secs_f64() * 1e6 / f64::from(iters);
    println!("{label:<46} {us:>10.2} us/op   ({checksum})");
    us
}

fn delta(label: &str, base: f64, v: f64) {
    println!(
        "  -> {label:<40} {:>+8.2} us  ({:+.1}%)",
        v - base,
        (v / base - 1.0) * 100.0
    );
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    sqlx::any::install_default_drivers();
    let url = format!("sqlite:///{}?mode=ro", db_path());

    // ------------------------------------------------------------------
    // 1. test_before_acquire: ruprizzle's PoolConfig default is `false`.
    // ------------------------------------------------------------------
    println!("\n=== 1. test_before_acquire (ruprizzle PoolConfig default = false) ===");
    let tested = AnyPoolOptions::new()
        .max_connections(10)
        .test_before_acquire(true)
        .connect(&url)
        .await
        .map(ruprizzle::Pool::Any)?;
    let untested = AnyPoolOptions::new()
        .max_connections(10)
        .test_before_acquire(false)
        .connect(&url)
        .await
        .map(ruprizzle::Pool::Any)?;

    let with_ping = bench("select_by_pk  test_before_acquire=true", 3000, || async {
        let u: User =
            sqlx::query_as::<sqlx::Any, User>("SELECT id, email, age FROM users WHERE id = ?")
                .bind(500i64)
                .fetch_one(&tested)
                .await
                .unwrap();
        u.id as usize
    })
    .await;
    let no_ping = bench("select_by_pk  test_before_acquire=false", 3000, || async {
        let u: User =
            sqlx::query_as::<sqlx::Any, User>("SELECT id, email, age FROM users WHERE id = ?")
                .bind(500i64)
                .fetch_one(&untested)
                .await
                .unwrap();
        u.id as usize
    })
    .await;
    delta("cost of the pre-acquire ping", no_ping, with_ping);

    let many_ping = bench("find_many_1000  test_before_acquire=true", 200, || async {
        sqlx::query_as::<sqlx::Any, User>("SELECT id, email, age FROM users")
            .fetch_all(&tested)
            .await
            .unwrap()
            .len()
    })
    .await;
    let many_no_ping = bench("find_many_1000  test_before_acquire=false", 200, || async {
        sqlx::query_as::<sqlx::Any, User>("SELECT id, email, age FROM users")
            .fetch_all(&untested)
            .await
            .unwrap()
            .len()
    })
    .await;
    delta("cost of the pre-acquire ping", many_no_ping, many_ping);

    // ------------------------------------------------------------------
    // 2. Pool checkout versus a held connection.
    // ------------------------------------------------------------------
    println!("\n=== 2. pool checkout cost (test_before_acquire=false throughout) ===");
    {
        // Not via `bench`: an `FnMut` closure cannot lend out `&mut conn` to a
        // future that outlives the call, so the loop is written out here.
        let mut conn = untested.acquire().await?;
        let mut checksum = 0usize;
        const ITERS: u32 = 3000;
        for _ in 0..3 {
            let u: User =
                sqlx::query_as::<sqlx::Any, User>("SELECT id, email, age FROM users WHERE id = ?")
                    .bind(500i64)
                    .fetch_one(&mut *conn)
                    .await
                    .unwrap();
            checksum += u.id as usize;
        }
        let start = Instant::now();
        for _ in 0..ITERS {
            let u: User =
                sqlx::query_as::<sqlx::Any, User>("SELECT id, email, age FROM users WHERE id = ?")
                    .bind(500i64)
                    .fetch_one(&mut *conn)
                    .await
                    .unwrap();
            checksum += u.id as usize;
        }
        let held = start.elapsed().as_secs_f64() * 1e6 / f64::from(ITERS);
        println!(
            "{:<46} {held:>10.2} us/op   ({checksum})",
            "select_by_pk  held connection"
        );
        delta("pool checkout per query", held, no_ping);
    }

    // ------------------------------------------------------------------
    // 3. AnyRow conversion, with no decoding at all.
    // ------------------------------------------------------------------
    println!("\n=== 3. AnyRow conversion cost (fetch only, no decode) ===");
    let native = sqlx::SqlitePool::connect_with(
        db_path()
            .parse::<sqlx::sqlite::SqliteConnectOptions>()?
            .read_only(true),
    )
    .await?;

    for (label, sql) in [
        ("2 int cols   (id, age)", "SELECT id, age FROM users"),
        (
            "3 cols w/text(id, email, age)",
            "SELECT id, email, age FROM users",
        ),
    ] {
        let n = bench(&format!("native SqliteRow  {label}"), 300, || async {
            sqlx::query(sql).fetch_all(&native).await.unwrap().len()
        })
        .await;
        let a = bench(&format!("AnyRow            {label}"), 300, || async {
            sqlx::query(sql).fetch_all(&untested).await.unwrap().len()
        })
        .await;
        delta("Any row-conversion tax", n, a);
    }

    // ------------------------------------------------------------------
    // 4. Materialise-then-decode versus decode-as-you-go.
    // ------------------------------------------------------------------
    println!("\n=== 4. Vec<AnyRow> then decode, vs decode as rows arrive ===");
    let sql = "SELECT id, email, age FROM users";

    let two_pass = bench(
        "fetch_all -> Vec<AnyRow> -> decode (ruprizzle)",
        300,
        || async {
            let rows = sqlx::query(sql).fetch_all(&untested).await.unwrap();
            let out: Vec<User> = rows
                .iter()
                .map(|r| User {
                    id: r.get::<i64, _>(0),
                    email: r.get::<String, _>(1),
                    age: r.get::<i64, _>(2),
                })
                .collect();
            out.len()
        },
    )
    .await;

    use futures_util::TryStreamExt;
    let one_pass = bench("fetch (stream) -> decode each row -> drop", 300, || async {
        let mut s = sqlx::query(sql).fetch(&untested);
        let mut out: Vec<User> = Vec::with_capacity(1024);
        while let Some(r) = s.try_next().await.unwrap() {
            out.push(User {
                id: r.get::<i64, _>(0),
                email: r.get::<String, _>(1),
                age: r.get::<i64, _>(2),
            });
        }
        out.len()
    })
    .await;
    delta(
        "cost of materialising Vec<AnyRow> first",
        one_pass,
        two_pass,
    );

    let query_as = bench("query_as::<Any, User> (sqlx's own map)", 300, || async {
        sqlx::query_as::<sqlx::Any, User>(sql)
            .fetch_all(&untested)
            .await
            .unwrap()
            .len()
    })
    .await;
    delta("vs sqlx query_as", one_pass, query_as);

    // ------------------------------------------------------------------
    // 5. Include path: 1000 users + 10000 posts.
    // ------------------------------------------------------------------
    println!("\n=== 5. include_posts breakdown (1000 users, 10000 posts) ===");
    let parents = bench("a. fetch 1000 users only", 60, || async {
        sqlx::query_as::<sqlx::Any, User>("SELECT id, email, age FROM users")
            .fetch_all(&untested)
            .await
            .unwrap()
            .len()
    })
    .await;

    let children_all = bench("b. fetch 10000 posts, no IN list", 60, || async {
        sqlx::query("SELECT id, author_id, title FROM posts")
            .fetch_all(&untested)
            .await
            .unwrap()
            .len()
    })
    .await;

    // What ruprizzle actually emits: `author_id IN (?, ?, ... x1000)`.
    let in_list = {
        let mut s = String::from("SELECT id, author_id, title FROM posts WHERE author_id IN (");
        for i in 0..1000 {
            if i > 0 {
                s.push(',');
            }
            s.push('?');
        }
        s.push(')');
        s
    };
    let children_in = bench(
        "c. fetch 10000 posts WHERE id IN (1000 binds)",
        60,
        || async {
            let mut q = sqlx::query(&in_list);
            for i in 1..=1000i64 {
                q = q.bind(i);
            }
            q.fetch_all(&untested).await.unwrap().len()
        },
    )
    .await;
    delta(
        "cost of the 1000-element IN list",
        children_all,
        children_in,
    );

    println!(
        "\n  parents + children (b) = {:.1} us; measured include_posts is ~16700 us",
        parents + children_all
    );
    println!("  parents + children (c) = {:.1} us", parents + children_in);

    // ------------------------------------------------------------------
    // 6. Isolated ping cost, the thing `test_before_acquire` pays for.
    // ------------------------------------------------------------------
    println!("\n=== 6. isolated round-trip costs ===");
    {
        use sqlx::Connection;
        let mut conn = untested.acquire().await?;
        for _ in 0..3 {
            conn.ping().await?;
        }
        let start = Instant::now();
        for _ in 0..3000 {
            conn.ping().await?;
        }
        let ping = start.elapsed().as_secs_f64() * 1e6 / 3000.0;
        println!(
            "{:<46} {ping:>10.2} us/op",
            "Connection::ping (sqlite worker round-trip)"
        );
        println!("  (Postgres ping = write_sync + wait_until_ready = one NETWORK round-trip)");
    }
    bench("SELECT 1  (Any, pooled)", 3000, || async {
        let n: i64 = sqlx::query_scalar::<sqlx::Any, i64>("SELECT 1")
            .fetch_one(&untested)
            .await
            .unwrap();
        n as usize
    })
    .await;

    // ------------------------------------------------------------------
    // 7. What ruprizzle's own Executor adds on top of raw sqlx.
    // ------------------------------------------------------------------
    println!("\n=== 7. ruprizzle Executor overhead over raw sqlx (same pool) ===");
    let raw = bench("raw sqlx::query fetch_all + decode", 300, || async {
        let rows = sqlx::query(sql).fetch_all(&untested).await.unwrap();
        rows.iter()
            .map(|r| User {
                id: r.get::<i64, _>(0),
                email: r.get::<String, _>(1),
                age: r.get::<i64, _>(2),
            })
            .fold(0, |acc, _| acc + 1)
    })
    .await;
    let via_exec = bench("via ruprizzle Executor::fetch_all_raw", 300, || async {
        use ruprizzle::Executor as _;
        let rows = untested
            .fetch_all_raw(sql.to_owned().into(), Vec::new())
            .await
            .unwrap();
        let ruprizzle::executor::RowBatch::Any(rows) = rows else {
            panic!("expected AnyRow batch")
        };
        rows.iter()
            .map(|r| User {
                id: r.get::<i64, _>(0),
                email: r.get::<String, _>(1),
                age: r.get::<i64, _>(2),
            })
            .fold(0, |acc, _| acc + 1)
    })
    .await;
    delta(
        "Executor wrapper (String alloc, Instant, tracing)",
        raw,
        via_exec,
    );

    Ok(())
}
