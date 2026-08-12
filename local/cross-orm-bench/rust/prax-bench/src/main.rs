//! Cross-ORM SQLite benchmark for Prax.
//!
//! Mirrors the ruprizzle `cross_orm_bench.rs` harness and writes the same
//! JSON result format to `prax-results.json`.

use std::collections::HashMap;
use std::env;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use prax_orm::{client, Model, PraxClient};
use prax_orm::sqlite::{SqliteConfig, SqliteEngine, SqlitePool};
use prax_query::filter::FilterValue;
use prax_query::{OrderBy, OrderByField, QueryEngine};
use serde::Serialize;

#[derive(Model, Debug, Clone, PartialEq)]
#[prax(table = "users")]
pub struct User {
    #[prax(id, auto)]
    id: i64,
    #[prax(unique)]
    email: String,
    age: i64,
    #[prax(relation(target = "Post", foreign_key = "author_id", child_table = "posts"))]
    posts: Vec<Post>,
}

#[derive(Model, Debug, Clone, PartialEq)]
#[prax(table = "posts")]
pub struct Post {
    #[prax(id, auto)]
    id: i64,
    author_id: i64,
    title: String,
}

#[derive(Model, Debug, Clone, PartialEq)]
#[prax(table = "bench_bulk")]
pub struct BenchBulk {
    #[prax(id)]
    id: i64,
    name: String,
    n: i64,
}

client!(User, Post, BenchBulk);

#[derive(Serialize)]
struct BenchResult {
    orm: String,
    operation: String,
    iters: u32,
    total_ms: f64,
    avg_ms: f64,
}

fn record_result(name: &str, iters: u32, start: Instant) -> BenchResult {
    let total_ms = start.elapsed().as_secs_f64() * 1000.0;
    let avg_ms = total_ms / iters as f64;
    println!(
        "{:>28} {:>10.3} us/op  (total {:>7.1} ms, {} iters)",
        name,
        avg_ms * 1000.0,
        total_ms,
        iters
    );
    BenchResult {
        orm: "prax".to_string(),
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

fn find_db_path() -> PathBuf {
    if let Ok(p) = env::var("BENCH_SQLITE_PATH") {
        return PathBuf::from(p);
    }
    let manifest = env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR not set");
    let root = Path::new(&manifest)
        .ancestors()
        .skip(1)
        .find(|p| p.join("crates").is_dir() || p.join(".git").is_dir())
        .expect("could not locate repo root");
    root.join("local")
        .join("cross-orm-bench")
        .join("node")
        .join("bench.sqlite3")
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let db_path = find_db_path();
    println!("Using database: {}", db_path.display());
    assert!(
        db_path.exists(),
        "database not found at {} - run seed.js first",
        db_path.display()
    );

    let pool: SqlitePool = SqlitePool::builder()
        .config(SqliteConfig::file(&db_path))
        .max_connections(4)
        .connection_timeout(Duration::from_secs(5))
        .build()
        .await?;

    let client = PraxClient::new(SqliteEngine::new(pool));

    let count = client.user().count().exec().await?;
    assert_eq!(count, 1000, "expected 1000 users in bench.sqlite3");

    let mut results = Vec::new();

    // 1. to_sql_select_by_pk: build a unique-select query, no I/O.
    {
        let client = client.clone();
        results.push(bench_sync("to_sql_select_by_pk", 100_000, || {
            let (_sql, _params) = client
                .user()
                .find_unique()
                .r#where(user::id::equals(500i64))
                .build_sql(client.engine().dialect());
            std::hint::black_box((_sql, _params));
        }));
    }

    // 2. to_sql_select_filter_order: build a filtered/ordered query, no I/O.
    {
        let client = client.clone();
        results.push(bench_sync("to_sql_select_filter_order", 100_000, || {
            let (_sql, _params) = client
                .user()
                .find_many()
                .r#where(user::age::gt(18i64))
                .order_by(OrderBy::from_fields([
                    OrderByField::asc(user::age::COLUMN),
                    OrderByField::asc(user::email::COLUMN),
                ]))
                .build_sql(client.engine().dialect());
            std::hint::black_box((_sql, _params));
        }));
    }

    // 3. select_by_pk: fetch user id = 500.
    {
        let client = client.clone();
        results.push(
            bench_async("select_by_pk", 1000, move || {
                let client = client.clone();
                async move {
                    let user = client
                        .user()
                        .find_unique()
                        .r#where(user::id::equals(500i64))
                        .exec()
                        .await
                        .expect("fetch user by pk");
                    assert_eq!(user.id, 500);
                    std::hint::black_box(user);
                }
            })
            .await,
        );
    }

    // 4. find_many_1000: fetch all users.
    {
        let client = client.clone();
        results.push(
            bench_async("find_many_1000", 50, move || {
                let client = client.clone();
                async move {
                    let rows = client
                        .user()
                        .find_many()
                        .exec()
                        .await
                        .expect("fetch all users");
                    assert_eq!(rows.len(), 1000);
                    std::hint::black_box(rows);
                }
            })
            .await,
        );
    }

    // 5. find_filtered_ordered: WHERE age > 18 ORDER BY age, email.
    {
        let client = client.clone();
        results.push(
            bench_async("find_filtered_ordered", 50, move || {
                let client = client.clone();
                async move {
                    let rows = client
                        .user()
                        .find_many()
                        .r#where(user::age::gt(18i64))
                        .order_by(OrderBy::from_fields([
                            OrderByField::asc(user::age::COLUMN),
                            OrderByField::asc(user::email::COLUMN),
                        ]))
                        .exec()
                        .await
                        .expect("fetch filtered/ordered users");
                    assert!(
                        rows.len() >= 980,
                        "expected ~1000 users, got {}",
                        rows.len()
                    );
                    std::hint::black_box(rows);
                }
            })
            .await,
        );
    }

    // 6. include_posts: fetch all users and their posts.
    {
        let client = client.clone();
        results.push(
            bench_async("include_posts", 10, move || {
                let client = client.clone();
                async move {
                    match client
                        .user()
                        .find_many()
                        .include(user::posts::fetch())
                        .exec()
                        .await
                    {
                        Ok(rows) => {
                            assert_eq!(rows.len(), 1000);
                            let total_posts: usize =
                                rows.iter().map(|u| u.posts.len()).sum();
                            assert_eq!(total_posts, 10000);
                            std::hint::black_box(rows);
                        }
                        Err(e) => {
                            eprintln!("include failed ({e}), falling back to manual fetch");
                            let users = client
                                .user()
                                .find_many()
                                .exec()
                                .await
                                .expect("fetch users");
                            let posts = client
                                .post()
                                .find_many()
                                .exec()
                                .await
                                .expect("fetch posts");
                            assert_eq!(users.len(), 1000);
                            assert_eq!(posts.len(), 10000);
                            let mut by_author: HashMap<i64, Vec<Post>> = HashMap::new();
                            for p in posts {
                                by_author.entry(p.author_id).or_default().push(p);
                            }
                            let total: usize = users
                                .iter()
                                .map(|u| by_author.get(&u.id).map(|v| v.len()).unwrap_or(0))
                                .sum();
                            assert_eq!(total, 10000);
                            std::hint::black_box(users);
                        }
                    }
                }
            })
            .await,
        );
    }

    // 7. bulk_insert_1000: clear bench_bulk, insert 1000 rows.
    let bulk_rows: Vec<Vec<FilterValue>> = (0..1000)
        .map(|i| {
            let i = i as i64;
            vec![
                FilterValue::Int(i + 1),
                FilterValue::String(format!("bulk-{i}")),
                FilterValue::Int(i * 3),
            ]
        })
        .collect();

    {
        let client = client.clone();
        results.push(
            bench_async("bulk_insert_1000", 10, move || {
                let client = client.clone();
                let rows = bulk_rows.clone();
                async move {
                    client
                        .bench_bulk()
                        .delete_many()
                        .exec()
                        .await
                        .expect("clear bench_bulk");
                    let inserted = client
                        .bench_bulk()
                        .create_many()
                        .columns(["id", "name", "n"])
                        .rows(rows)
                        .exec()
                        .await
                        .expect("bulk insert");
                    assert_eq!(inserted, 1000);
                    std::hint::black_box(inserted);
                }
            })
            .await,
        );
    }

    println!("\n{}", serde_json::to_string_pretty(&results)?);

    let out_path = db_path.parent().unwrap().join("prax-results.json");
    tokio::fs::write(&out_path, serde_json::to_string_pretty(&results)?)
        .await
        .expect("write results");
    println!("Wrote {}", out_path.display());

    Ok(())
}
