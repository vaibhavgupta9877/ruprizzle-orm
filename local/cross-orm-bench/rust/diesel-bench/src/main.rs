//! Diesel benchmark for cross-orm SQLite bench.
//!
//! Mirrors the ruprizzle `cross_orm_bench.rs` harness and writes an identical
//! JSON result file.

use std::env;
use std::path::PathBuf;
use std::time::Instant;

use diesel::prelude::*;
use diesel::sqlite::Sqlite;
use serde::Serialize;

mod schema {
    diesel::table! {
        users (id) {
            id -> BigInt,
            email -> Text,
            age -> BigInt,
        }
    }

    diesel::table! {
        posts (id) {
            id -> BigInt,
            author_id -> BigInt,
            title -> Text,
        }
    }

    diesel::table! {
        bench_bulk (id) {
            id -> BigInt,
            name -> Text,
            n -> BigInt,
        }
    }

    diesel::allow_tables_to_appear_in_same_query!(users, posts, bench_bulk);
}

use schema::{bench_bulk, posts, users};

#[derive(Queryable)]
#[allow(dead_code)]
struct User {
    id: i64,
    email: String,
    age: i64,
}

#[derive(Queryable)]
#[allow(dead_code)]
struct Post {
    id: i64,
    author_id: i64,
    title: String,
}

#[derive(Queryable, Insertable)]
#[diesel(table_name = bench_bulk)]
struct BenchBulk {
    id: i64,
    name: String,
    n: i64,
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
    println!(
        "{:>28} {:>10.3} us/op  (total {:>7.1} ms, {} iters)",
        name,
        avg_ms * 1000.0,
        total_ms,
        iters
    );
    BenchResult {
        orm: "diesel".to_string(),
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

fn bench_db_relative() -> PathBuf {
    PathBuf::from("local")
        .join("cross-orm-bench")
        .join("node")
        .join("bench.sqlite3")
}

fn resolve_db_path() -> PathBuf {
    if let Ok(path) = env::var("BENCH_SQLITE_PATH") {
        return PathBuf::from(path);
    }

    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let relative = bench_db_relative();
    let fallback = manifest
        .ancestors()
        .last()
        .map(|root| root.join(&relative));

    for ancestor in manifest.ancestors() {
        let candidate = ancestor.join(&relative);
        if candidate.exists() {
            return candidate;
        }
    }

    fallback.unwrap_or_else(bench_db_relative)
}

fn main() {
    let _ = dotenvy::dotenv();

    let db_path = resolve_db_path();
    if !db_path.exists() {
        panic!("bench database not found: {}", db_path.display());
    }

    let mut conn = SqliteConnection::establish(db_path.to_string_lossy().as_ref())
        .expect("failed to open SQLite connection");

    // Quick sanity check that the database was seeded.
    let user_count: i64 = users::table
        .count()
        .get_result(&mut conn)
        .expect("failed to count users");
    assert_eq!(user_count, 1000, "expected 1000 users in bench database");

    let mut results: Vec<BenchResult> = Vec::new();

    // Query construction: SELECT ... WHERE id = 500
    results.push(bench_sync("to_sql_select_by_pk", 100_000, || {
        let query = users::table.find(500i64);
        let sql = diesel::debug_query::<Sqlite, _>(&query).to_string();
        std::hint::black_box(sql);
    }));

    // Query construction: SELECT ... WHERE age > 18 ORDER BY age ASC, email ASC
    results.push(bench_sync("to_sql_select_filter_order", 100_000, || {
        let query = users::table
            .filter(users::age.gt(18i64))
            .order_by(users::age.asc())
            .then_order_by(users::email.asc());
        let sql = diesel::debug_query::<Sqlite, _>(&query).to_string();
        std::hint::black_box(sql);
    }));

    // End-to-end: select by PK.
    {
        let conn = &mut conn;
        results.push(bench_sync("select_by_pk", 1_000, || {
            let user: User = users::table
                .find(500i64)
                .first(conn)
                .expect("failed to fetch user by pk");
            assert_eq!(user.id, 500);
            std::hint::black_box(user);
        }));
    }

    // End-to-end: find many 1000 rows.
    {
        let conn = &mut conn;
        results.push(bench_sync("find_many_1000", 50, || {
            let rows: Vec<User> = users::table
                .load(conn)
                .expect("failed to fetch all users");
            assert_eq!(rows.len(), 1000);
            std::hint::black_box(rows);
        }));
    }

    // End-to-end: filtered + ordered.
    {
        let conn = &mut conn;
        results.push(bench_sync("find_filtered_ordered", 50, || {
            let rows: Vec<User> = users::table
                .filter(users::age.gt(18i64))
                .order_by(users::age.asc())
                .then_order_by(users::email.asc())
                .load(conn)
                .expect("failed to fetch filtered users");
            assert!(rows.len() >= 980, "expected ~1000 users, got {}", rows.len());
            std::hint::black_box(rows);
        }));
    }

    // End-to-end: include posts for all users.
    {
        let conn = &mut conn;
        results.push(bench_sync("include_posts", 10, || {
            let users: Vec<User> = users::table.load(conn).expect("failed to fetch users");
            assert_eq!(users.len(), 1000);

            let user_ids: Vec<i64> = users.iter().map(|u| u.id).collect();
            let posts: Vec<Post> = posts::table
                .filter(posts::author_id.eq_any(user_ids))
                .load(conn)
                .expect("failed to fetch posts");
            assert_eq!(posts.len(), 10_000);
            std::hint::black_box((users, posts));
        }));
    }

    // Bulk insert 1000 rows into bench_bulk.
    let bulk_rows: Vec<BenchBulk> = (0..1_000)
        .map(|i| BenchBulk {
            id: (i + 1) as i64,
            name: format!("bulk-{i}"),
            n: (i * 3) as i64,
        })
        .collect();

    {
        let conn = &mut conn;
        results.push(bench_sync("bulk_insert_1000", 10, || {
            diesel::delete(bench_bulk::table)
                .execute(conn)
                .expect("failed to clear bench_bulk");
            let inserted = diesel::insert_into(bench_bulk::table)
                .values(&bulk_rows)
                .execute(conn)
                .expect("failed to bulk insert");
            assert_eq!(inserted, 1_000);
            std::hint::black_box(inserted);
        }));
    }

    println!("\n{}", serde_json::to_string_pretty(&results).unwrap());

    let output_path = db_path
        .parent()
        .expect("db path has no parent")
        .join("diesel-results.json");
    std::fs::write(
        &output_path,
        serde_json::to_string_pretty(&results).unwrap(),
    )
    .expect("failed to write results");
    println!("Wrote {}", output_path.display());
}
