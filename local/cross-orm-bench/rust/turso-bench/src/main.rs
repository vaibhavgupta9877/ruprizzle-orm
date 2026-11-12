//! Turso (`turso` crate, pure-Rust SQLite-compatible engine) benchmark.
//!
//! Turso has no ORM of its own, so this harness runs the same read and write
//! shapes as `cross_orm_bench.rs` against the `turso` driver directly, with
//! hand-written row decoding. It is a driver baseline, not an ORM comparison:
//! it shows what the engine costs, with ruprizzle's query building removed.
//!
//! Writes the same JSON result shape as the other harnesses.

use std::collections::HashMap;
use std::env;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use serde::Serialize;
use simple_process_stats::ProcessStats;
use turso::{Builder, Connection};

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

#[derive(Default)]
struct BenchOutcome {
    rows: usize,
    queries: usize,
}

#[derive(Debug, Clone)]
struct User {
    id: i64,
    email: String,
    age: i64,
    name: String,
    created_at: i64,
}

#[derive(Debug, Clone)]
struct Post {
    id: i64,
    author_id: i64,
}

const USER_COLS: &str = "id, email, age, name, created_at";

fn sample_stats() -> ProcessStats {
    ProcessStats::get().unwrap_or_else(|_| ProcessStats {
        cpu_time_user: Duration::ZERO,
        cpu_time_kernel: Duration::ZERO,
        memory_usage_bytes: 0,
    })
}

fn record(
    name: &str,
    iters: u32,
    start: Instant,
    before: ProcessStats,
    after: ProcessStats,
    outcome: &BenchOutcome,
) -> BenchResult {
    let total_ms = start.elapsed().as_secs_f64() * 1000.0;
    let avg_ms = total_ms / iters as f64;
    let qps = iters as f64 * 1000.0 / total_ms.max(f64::MIN_POSITIVE);
    let cpu = after.cpu_time_user.saturating_sub(before.cpu_time_user)
        + after.cpu_time_kernel.saturating_sub(before.cpu_time_kernel);
    let peak = before.memory_usage_bytes.max(after.memory_usage_bytes);
    println!(
        "{:>28} {:>10.3} us/op  (total {:>7.1} ms, {} iters)",
        name,
        avg_ms * 1000.0,
        total_ms,
        iters
    );
    BenchResult {
        orm: "turso".to_string(),
        operation: name.to_string(),
        iters,
        total_ms,
        avg_ms,
        qps,
        rows_returned: outcome.rows,
        queries_issued: outcome.queries,
        peak_rss_mb: peak as f64 / (1024.0 * 1024.0),
        cpu_time_ms: cpu.as_secs_f64() * 1000.0,
    }
}

/// Runs `f` `iters` times after three warm-up calls, timing the loop.
async fn bench<F, Fut>(name: &str, iters: u32, mut f: F) -> BenchResult
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
    record(name, iters, start, before, after, &last)
}

async fn query_users(conn: &Connection, sql: &str, params: impl turso::IntoParams) -> Vec<User> {
    let stmt = conn.prepare_cached(sql).await.expect("prepare");
    let mut stmt = stmt;
    let mut rows = stmt.query(params).await.expect("query");
    let mut out = Vec::new();
    while let Some(row) = rows.next().await.expect("row") {
        out.push(User {
            id: row.get(0).expect("id"),
            email: row.get(1).expect("email"),
            age: row.get(2).expect("age"),
            name: row.get(3).expect("name"),
            created_at: row.get(4).expect("created_at"),
        });
    }
    out
}

fn db_path() -> PathBuf {
    if let Ok(path) = env::var("BENCH_SQLITE_PATH") {
        return PathBuf::from(path);
    }
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    for ancestor in manifest.ancestors() {
        let candidate = ancestor.join("local/cross-orm-bench/node/bench.sqlite3");
        if candidate.exists() {
            return candidate;
        }
    }
    PathBuf::from("local/cross-orm-bench/node/bench.sqlite3")
}

fn results_path() -> PathBuf {
    if let Ok(dir) = env::var("BENCH_RESULTS_DIR") {
        return PathBuf::from(dir).join("turso-results.json");
    }
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("turso-results.json")
}

#[tokio::main]
async fn main() {
    let path = db_path();
    let db = Builder::new_local(&path.to_string_lossy())
        .build()
        .await
        .expect("open turso database");
    let conn = db.connect().expect("connect");

    let count = {
        let mut stmt = conn
            .prepare("SELECT COUNT(*) FROM users")
            .await
            .expect("prepare count");
        let mut rows = stmt.query(()).await.expect("count query");
        let row = rows.next().await.expect("row").expect("one row");
        row.get::<i64>(0).expect("count")
    };
    assert_eq!(count, 1000, "expected 1000 users in bench database");

    let mut results = Vec::new();

    results.push(
        bench("select_by_pk", 1000, || async {
            let rows = query_users(
                &conn,
                &format!("SELECT {USER_COLS} FROM users WHERE id = ? LIMIT 1"),
                (500i64,),
            )
            .await;
            assert_eq!(rows[0].id, 500);
            BenchOutcome { rows: 1, queries: 1 }
        })
        .await,
    );

    results.push(
        bench("prepared_select_by_pk", 1000, || async {
            let rows = query_users(
                &conn,
                &format!("SELECT {USER_COLS} FROM users WHERE id = ? LIMIT 1"),
                (500i64,),
            )
            .await;
            assert_eq!(rows[0].id, 500);
            BenchOutcome { rows: 1, queries: 1 }
        })
        .await,
    );

    results.push(
        bench("find_many_1000", 50, || async {
            let rows = query_users(&conn, &format!("SELECT {USER_COLS} FROM users"), ()).await;
            assert_eq!(rows.len(), 1000);
            BenchOutcome {
                rows: rows.len(),
                queries: 1,
            }
        })
        .await,
    );

    results.push(
        bench("find_filtered_ordered", 50, || async {
            let rows = query_users(
                &conn,
                &format!(
                    "SELECT {USER_COLS} FROM users WHERE age > ? ORDER BY age ASC, email ASC"
                ),
                (18i64,),
            )
            .await;
            assert!(rows.len() >= 980);
            BenchOutcome {
                rows: rows.len(),
                queries: 1,
            }
        })
        .await,
    );

    results.push(
        bench("find_filtered_paginated", 50, || async {
            let rows = query_users(
                &conn,
                &format!(
                    "SELECT {USER_COLS} FROM users WHERE age > ? ORDER BY age ASC, email ASC LIMIT 20 OFFSET 500"
                ),
                (18i64,),
            )
            .await;
            assert_eq!(rows.len(), 20);
            BenchOutcome {
                rows: rows.len(),
                queries: 1,
            }
        })
        .await,
    );

    let ids: Vec<i64> = (1..=50).collect();
    let in_list = ids
        .iter()
        .map(|i| i.to_string())
        .collect::<Vec<_>>()
        .join(",");
    results.push(
        bench("find_in_list", 100, || async {
            let rows = query_users(
                &conn,
                &format!("SELECT {USER_COLS} FROM users WHERE id IN ({in_list}) ORDER BY id ASC"),
                (),
            )
            .await;
            assert_eq!(rows.len(), 50);
            BenchOutcome {
                rows: rows.len(),
                queries: 1,
            }
        })
        .await,
    );

    results.push(
        bench("find_complex_filter", 50, || async {
            let rows = query_users(
                &conn,
                &format!(
                    "SELECT {USER_COLS} FROM users WHERE age > ? AND email LIKE ? AND id BETWEEN ? AND ? \
                     ORDER BY age ASC, email ASC LIMIT 100"
                ),
                (18i64, "%example.com%".to_string(), 100i64, 900i64),
            )
            .await;
            assert_eq!(rows.len(), 100);
            BenchOutcome {
                rows: rows.len(),
                queries: 1,
            }
        })
        .await,
    );

    results.push(
        bench("count_filtered", 100, || async {
            let mut stmt = conn
                .prepare("SELECT COUNT(*) FROM users WHERE age > ?")
                .await
                .expect("prepare");
            let mut rows = stmt.query((18i64,)).await.expect("query");
            let row = rows.next().await.expect("row").expect("one row");
            let n: i64 = row.get(0).expect("count");
            assert!(n >= 980);
            BenchOutcome {
                rows: n as usize,
                queries: 1,
            }
        })
        .await,
    );

    results.push(
        bench("exists_filtered", 100, || async {
            let mut stmt = conn
                .prepare("SELECT 1 FROM users WHERE age > ? LIMIT 1")
                .await
                .expect("prepare");
            let mut rows = stmt.query((18i64,)).await.expect("query");
            let found = rows.next().await.expect("row").is_some();
            assert!(found);
            BenchOutcome {
                rows: 1,
                queries: 1,
            }
        })
        .await,
    );

    // include_posts, expressed the way an ORM-less driver does it: two
    // queries and an in-memory group by author.
    results.push(
        bench("include_posts", 10, || async {
            let users = query_users(&conn, &format!("SELECT {USER_COLS} FROM users"), ()).await;
            let mut stmt = conn
                .prepare_cached("SELECT id, author_id FROM posts")
                .await
                .expect("prepare posts");
            let mut rows = stmt.query(()).await.expect("posts query");
            let mut posts = Vec::new();
            while let Some(row) = rows.next().await.expect("row") {
                posts.push(Post {
                    id: row.get(0).expect("id"),
                    author_id: row.get(1).expect("author_id"),
                });
            }
            let mut grouped: HashMap<i64, Vec<Post>> = HashMap::new();
            for p in posts {
                grouped.entry(p.author_id).or_default().push(p);
            }
            let total: usize = users
                .iter()
                .map(|u| grouped.get(&u.id).map_or(0, Vec::len))
                .sum();
            assert_eq!(users.len(), 1000);
            BenchOutcome {
                rows: users.len() + total,
                queries: 2,
            }
        })
        .await,
    );

    results.push(
        bench("bulk_insert_1000", 10, || async {
            conn.execute("DELETE FROM bench_bulk", ())
                .await
                .expect("clear bench_bulk");
            conn.execute("BEGIN", ()).await.expect("begin");
            let mut stmt = conn
                .prepare("INSERT INTO bench_bulk (id, name, n) VALUES (?, ?, ?)")
                .await
                .expect("prepare insert");
            for i in 1..=1000i64 {
                stmt.execute((i, format!("bulk-{i}"), i % 7))
                    .await
                    .expect("insert");
                stmt.reset().expect("reset");
            }
            conn.execute("COMMIT", ()).await.expect("commit");
            BenchOutcome {
                rows: 1000,
                queries: 2,
            }
        })
        .await,
    );

    let json = serde_json::to_string_pretty(&results).expect("serialize");
    let out = results_path();
    if let Some(parent) = out.parent() {
        std::fs::create_dir_all(parent).expect("results dir");
    }
    std::fs::write(&out, json).expect("write results");
    println!("wrote {}", out.display());
}
