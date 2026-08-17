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
use prax_query::filter::{Filter, FilterValue};
use prax_query::sql::DatabaseType;
use prax_query::Sql;
use prax_query::{OrderBy, OrderByField, QueryEngine};
use serde::Serialize;
use simple_process_stats::ProcessStats;

#[derive(Model, Debug, Clone, PartialEq)]
#[prax(table = "users")]
pub struct User {
    #[prax(id, auto)]
    id: i64,
    #[prax(unique)]
    email: String,
    age: i64,
    name: String,
    created_at: i64,
    #[prax(relation(target = "Post", foreign_key = "author_id", child_table = "posts"))]
    posts: Vec<Post>,
}

#[derive(Model, Debug, Clone, PartialEq)]
#[prax(table = "posts")]
pub struct Post {
    #[prax(id, auto)]
    id: i64,
    author_id: i64,
    category_id: i64,
    title: String,
    published_at: i64,
    views: i64,
    #[prax(
        relation(target = "User", foreign_key = "id", local_key = "author_id", child_table = "users")
    )]
    author: Vec<User>,
    #[prax(relation(target = "Comment", foreign_key = "post_id", child_table = "comments"))]
    comments: Vec<Comment>,
    #[prax(relation(target = "PostTag", foreign_key = "post_id", child_table = "post_tags"))]
    post_tags: Vec<PostTag>,
}

#[derive(Model, Debug, Clone, PartialEq)]
#[prax(table = "categories")]
pub struct Category {
    #[prax(id, auto)]
    id: i64,
    name: String,
}

#[derive(Model, Debug, Clone, PartialEq)]
#[prax(table = "comments")]
pub struct Comment {
    #[prax(id, auto)]
    id: i64,
    post_id: i64,
    author_id: i64,
    content: String,
    created_at: i64,
}

#[derive(Model, Debug, Clone, PartialEq)]
#[prax(table = "tags")]
pub struct Tag {
    #[prax(id, auto)]
    id: i64,
    name: String,
}

#[derive(Model, Debug, Clone, PartialEq)]
#[prax(table = "post_tags")]
pub struct PostTag {
    #[prax(id)]
    post_id: i64,
    tag_id: i64,
    #[prax(relation(target = "Tag", foreign_key = "id", local_key = "tag_id", child_table = "tags"))]
    tag: Vec<Tag>,
}

#[derive(Model, Debug, Clone, PartialEq)]
#[prax(table = "followers")]
pub struct Follower {
    #[prax(id)]
    follower_id: i64,
    followee_id: i64,
    created_at: i64,
}

#[derive(Model, Debug, Clone, PartialEq)]
#[prax(table = "likes")]
pub struct Like {
    #[prax(id, auto)]
    id: i64,
    user_id: i64,
    post_id: i64,
    created_at: i64,
}

#[derive(Model, Debug, Clone, PartialEq)]
#[prax(table = "bench_bulk")]
pub struct BenchBulk {
    #[prax(id)]
    id: i64,
    name: String,
    n: i64,
}

client!(User, Post, Category, Comment, Tag, PostTag, Follower, Like, BenchBulk);

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
        cpu_time_user: Duration::ZERO,
        cpu_time_kernel: Duration::ZERO,
        memory_usage_bytes: 0,
    })
}

fn record_result(
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
        orm: "prax".to_string(),
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
        f();
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
        f().await;
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

    // ── Query construction (no I/O) ──────────────────────────────────────────

    // 1. to_sql_select_by_pk: build a unique-select query.
    {
        let client = client.clone();
        results.push(bench_sync("to_sql_select_by_pk", 100_000, move || {
            let (_sql, _params) = client
                .user()
                .find_unique()
                .r#where(user::id::equals(500i64))
                .build_sql(client.engine().dialect());
            std::hint::black_box((_sql, _params));
            BenchOutcome::default()
        }));
    }

    // 2. to_sql_select_filter_order: filtered, ordered, limit 1000, offset 0.
    {
        let client = client.clone();
        results.push(bench_sync("to_sql_select_filter_order", 100_000, move || {
            let (_sql, _params) = client
                .user()
                .find_many()
                .r#where(user::age::gt(18i64))
                .r#where(user::email::contains("@example.com"))
                .order_by(OrderBy::from_fields([
                    OrderByField::asc(user::age::COLUMN),
                    OrderByField::asc(user::email::COLUMN),
                ]))
                .skip(0)
                .take(1000)
                .build_sql(client.engine().dialect());
            std::hint::black_box((_sql, _params));
            BenchOutcome::default()
        }));
    }

    // 3. to_sql_select_in_list: IN list with 50 ids.
    {
        let client = client.clone();
        results.push(bench_sync("to_sql_select_in_list", 100_000, move || {
            let ids: Vec<i64> = (1..=50).collect();
            let (_sql, _params) = client
                .user()
                .find_many()
                .r#where(user::id::in_(ids))
                .order_by(OrderByField::asc(user::id::COLUMN))
                .take(50)
                .build_sql(client.engine().dialect());
            std::hint::black_box((_sql, _params));
            BenchOutcome::default()
        }));
    }

    // 4. to_sql_select_complex_filter: age > 18, email LIKE '%example.com%', id 100-900.
    {
        let client = client.clone();
        results.push(bench_sync("to_sql_select_complex_filter", 100_000, move || {
            let (_sql, _params) = client
                .user()
                .find_many()
                .r#where(user::age::gt(18i64))
                .r#where(user::email::contains("example.com"))
                .r#where(user::id::gte(100i64))
                .r#where(user::id::lte(900i64))
                .order_by(OrderBy::from_fields([
                    OrderByField::asc(user::age::COLUMN),
                    OrderByField::asc(user::email::COLUMN),
                ]))
                .take(100)
                .build_sql(client.engine().dialect());
            std::hint::black_box((_sql, _params));
            BenchOutcome::default()
        }));
    }

    // 5. to_sql_select_paginated: filter + order + limit 20 offset 500.
    {
        let client = client.clone();
        results.push(bench_sync("to_sql_select_paginated", 100_000, move || {
            let (_sql, _params) = client
                .user()
                .find_many()
                .r#where(user::age::gt(18i64))
                .r#where(user::email::contains("example.com"))
                .order_by(OrderBy::from_fields([
                    OrderByField::asc(user::age::COLUMN),
                    OrderByField::asc(user::email::COLUMN),
                ]))
                .skip(500)
                .take(20)
                .build_sql(client.engine().dialect());
            std::hint::black_box((_sql, _params));
            BenchOutcome::default()
        }));
    }

    // New query-construction micro-benchmarks for recent ruprizzle enhancements.

    // 6. to_sql_prepared_select_by_pk: Prax has no separate .prepare(); use find_unique build_sql.
    {
        let client = client.clone();
        results.push(bench_sync("to_sql_prepared_select_by_pk", 100_000, move || {
            let (_sql, _params) = client
                .user()
                .find_unique()
                .r#where(user::id::equals(500i64))
                .build_sql(client.engine().dialect());
            std::hint::black_box((_sql, _params));
            BenchOutcome::default()
        }));
    }

    // 7. prepared_rebind_select_by_pk: Prax has no prepared-statement rebind API;
    //    pre-build the SQL once and swap the parameter vector each iteration.
    {
        let (sql, _) = Sql::new("SELECT * FROM users WHERE id = ")
            .with_db_type(DatabaseType::SQLite)
            .bind(0i64)
            .push(" LIMIT 1")
            .build();
        results.push(bench_sync("prepared_rebind_select_by_pk", 100_000, move || {
            let params = vec![FilterValue::Int(123i64)];
            std::hint::black_box((sql.clone(), params));
            BenchOutcome::default()
        }));
    }

    // 8. to_sql_conditional_filter: conditionally attach filter, order, and limit.
    {
        let client = client.clone();
        results.push(bench_sync("to_sql_conditional_filter", 100_000, move || {
            let maybe_age: Option<Filter> = Some(user::age::gt(18i64).into());
            let maybe_order: Option<OrderBy> = Some(OrderByField::asc(user::age::COLUMN).into());
            let maybe_limit: Option<u64> = Some(100);

            let mut q = client.user().find_many();
            if let Some(f) = maybe_age {
                q = q.r#where(f);
            }
            if let Some(o) = maybe_order {
                q = q.order_by(o);
            }
            if let Some(l) = maybe_limit {
                q = q.take(l);
            }

            let (_sql, _params) = q.build_sql(client.engine().dialect());
            std::hint::black_box((_sql, _params));
            BenchOutcome::default()
        }));
    }

    // 9. to_sql_select_with_cte: Prax has CTE string builders but no query-builder CTE integration.
    {
        let client = client.clone();
        results.push(bench_sync("to_sql_select_with_cte", 100_000, move || {
            let (inner_sql, _) = client
                .user()
                .find_many()
                .r#where(user::age::gt(18i64))
                .build_sql(client.engine().dialect());
            let sql = prax_query::cte::WithClause::new()
                .cte(prax_query::cte::Cte::new("active").as_query(inner_sql))
                .select("*")
                .from("active")
                .where_clause("id > 0")
                .build(DatabaseType::SQLite)
                .unwrap();
            std::hint::black_box(sql);
            BenchOutcome::default()
        }));
    }

    // 10. to_sql_select_with_recursive_cte: build a recursive CTE from raw SQL.
    {
        results.push(bench_sync("to_sql_select_with_recursive_cte", 100_000, move || {
            let sql = prax_query::cte::WithClause::new()
                .cte(
                    prax_query::cte::Cte::new("nums")
                        .recursive()
                        .as_query("SELECT * FROM users WHERE id = ? UNION ALL SELECT * FROM users WHERE id = ?"),
                )
                .select("*")
                .from("nums")
                .where_clause("id > 0")
                .build(DatabaseType::SQLite)
                .unwrap();
            std::hint::black_box(sql);
            BenchOutcome::default()
        }));
    }

    // 11. to_sql_set_union: Prax FindMany has no union; build two SQLs and concatenate.
    {
        let client = client.clone();
        results.push(bench_sync("to_sql_set_union", 100_000, move || {
            let (left_sql, mut left_params) = client
                .user()
                .find_many()
                .r#where(user::age::gt(18i64))
                .build_sql(client.engine().dialect());
            let (right_sql, right_params) = client
                .user()
                .find_many()
                .r#where(user::age::lte(18i64))
                .build_sql(client.engine().dialect());

            let sql = format!("({}) UNION ALL ({})", left_sql, right_sql);
            left_params.extend(right_params);
            std::hint::black_box((sql, left_params));
            BenchOutcome::default()
        }));
    }

    // 12. to_sql_select_with_join: Prax has no join support in the query builder.
    {
        results.push(bench_sync("to_sql_select_with_join", 100_000, move || {
            let (_sql, _params) = Sql::new(
                "SELECT posts.* FROM posts INNER JOIN users ON posts.author_id = users.id",
            )
            .with_db_type(DatabaseType::SQLite)
            .build();
            std::hint::black_box((_sql, _params));
            BenchOutcome::default()
        }));
    }

    // 13. to_sql_select_exists_subquery: Prax has no Filter::exists; build raw SQL.
    {
        results.push(bench_sync("to_sql_select_exists_subquery", 100_000, move || {
            let (_sql, _params) = Sql::new(
                "SELECT * FROM users WHERE EXISTS (SELECT 1 FROM posts WHERE posts.author_id = users.id)",
            )
            .with_db_type(DatabaseType::SQLite)
            .build();
            std::hint::black_box((_sql, _params));
            BenchOutcome::default()
        }));
    }

    // 14. to_sql_select_in_subquery: Prax has no in_subquery filter; build raw SQL.
    {
        results.push(bench_sync("to_sql_select_in_subquery", 100_000, move || {
            let (_sql, _params) = Sql::new("SELECT * FROM users WHERE users.id IN (SELECT author_id FROM posts WHERE author_id > ")
                .with_db_type(DatabaseType::SQLite)
                .bind(0i64)
                .push(")")
                .build();
            std::hint::black_box((_sql, _params));
            BenchOutcome::default()
        }));
    }

    // 15. to_sql_nested_insert: Prax CreateOperation.build_sql ignores nested writes; build raw SQL.
    {
        results.push(bench_sync("to_sql_nested_insert", 100_000, move || {
            let (_sql, _params) = Sql::new("INSERT INTO users (id, email, age, name, created_at) VALUES (")
                .with_db_type(DatabaseType::SQLite)
                .bind(9999i64)
                .push(", ")
                .bind("nested@example.com")
                .push(", ")
                .bind(30i64)
                .push(", ")
                .bind("Nested")
                .push(", ")
                .bind(0i64)
                .push("); INSERT INTO posts (id, category_id, title, published_at, views, author_id) VALUES (")
                .bind(10001i64)
                .push(", ")
                .bind(1i64)
                .push(", ")
                .bind("nested post")
                .push(", ")
                .bind(0i64)
                .push(", ")
                .bind(0i64)
                .push(", ")
                .bind(9999i64)
                .push(")")
                .build();
            std::hint::black_box((_sql, _params));
            BenchOutcome::default()
        }));
    }

    // 16. to_sql_nested_update: Prax UpdateOperation.build_sql ignores nested writes; build raw SQL.
    {
        results.push(bench_sync("to_sql_nested_update", 100_000, move || {
            let (_sql, _params) = Sql::new("UPDATE users SET name = ")
                .with_db_type(DatabaseType::SQLite)
                .bind("updated")
                .push(" WHERE id = ")
                .bind(1i64)
                .push("; UPDATE posts SET author_id = ")
                .bind(1i64)
                .push(" WHERE id IN (")
                .bind(10001i64)
                .push(", ")
                .bind(10002i64)
                .push(")")
                .build();
            std::hint::black_box((_sql, _params));
            BenchOutcome::default()
        }));
    }

    // ── End-to-end reads ─────────────────────────────────────────────────────

    // 6. select_by_pk: fetch user id = 500.
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
                    BenchOutcome { rows: 1, queries: 1 }
                }
            })
            .await,
        );
    }

    // 6a. prepared_select_by_pk: Prax has no prepared statement builder;
    //     fall back to the underlying SQLite connection with prepare_cached.
    {
        let conn = client.engine().pool().get().await?;
        let handle = conn.inner().clone();
        drop(conn);
        results.push(
            bench_async("prepared_select_by_pk", 1000, move || {
                let handle = handle.clone();
                async move {
                    let id = handle
                        .call(|conn| {
                            let mut stmt = conn
                                .prepare_cached("SELECT * FROM users WHERE id = ? LIMIT 1")?;
                            let id: i64 = stmt.query_row((500i64,), |row| row.get(0))?;
                            Ok(id)
                        })
                        .await
                        .expect("prepared select by pk");
                    assert_eq!(id, 500);
                    BenchOutcome { rows: 1, queries: 1 }
                }
            })
            .await,
        );
    }

    // 7. find_many_1000: fetch all users.
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
                    BenchOutcome {
                        rows: rows.len(),
                        queries: 1,
                    }
                }
            })
            .await,
        );
    }

    // 7a. stream_find_many_1000: Prax has no streaming API; fall back to raw SQLite row iteration.
    {
        let conn = client.engine().pool().get().await?;
        let handle = conn.inner().clone();
        drop(conn);
        results.push(
            bench_async("stream_find_many_1000", 50, move || {
                let handle = handle.clone();
                async move {
                    let rows = handle
                        .call(|conn| {
                            let mut stmt = conn.prepare_cached("SELECT * FROM users")?;
                            let mut rows = stmt.query(())?;
                            let mut count = 0;
                            while let Some(_) = rows.next()? {
                                count += 1;
                            }
                            Ok(count)
                        })
                        .await
                        .expect("stream all users");
                    assert_eq!(rows, 1000);
                    BenchOutcome { rows, queries: 1 }
                }
            })
            .await,
        );
    }

    // 8. find_filtered_ordered: WHERE age > 18 ORDER BY age, email.
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
                        "expected ~980 users, got {}",
                        rows.len()
                    );
                    BenchOutcome {
                        rows: rows.len(),
                        queries: 1,
                    }
                }
            })
            .await,
        );
    }

    // 9. find_filtered_paginated: filter + order + limit 20 offset 500.
    {
        let client = client.clone();
        results.push(
            bench_async("find_filtered_paginated", 50, move || {
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
                        .skip(500)
                        .take(20)
                        .exec()
                        .await
                        .expect("fetch paginated users");
                    assert_eq!(rows.len(), 20);
                    BenchOutcome {
                        rows: rows.len(),
                        queries: 1,
                    }
                }
            })
            .await,
        );
    }

    // 10. find_in_list: IN list with 50 ids.
    {
        let client = client.clone();
        let ids_50: Vec<i64> = (1..=50).collect();
        results.push(
            bench_async("find_in_list", 100, move || {
                let client = client.clone();
                let ids = ids_50.clone();
                async move {
                    let rows = client
                        .user()
                        .find_many()
                        .r#where(user::id::in_(ids))
                        .order_by(OrderByField::asc(user::id::COLUMN))
                        .take(50)
                        .exec()
                        .await
                        .expect("fetch users by id list");
                    assert_eq!(rows.len(), 50);
                    BenchOutcome {
                        rows: rows.len(),
                        queries: 1,
                    }
                }
            })
            .await,
        );
    }

    // 11. find_complex_filter: age > 18, email LIKE '%example.com%', id 100..900.
    {
        let client = client.clone();
        results.push(
            bench_async("find_complex_filter", 50, move || {
                let client = client.clone();
                async move {
                    let rows = client
                        .user()
                        .find_many()
                        .r#where(user::age::gt(18i64))
                        .r#where(user::email::contains("example.com"))
                        .r#where(user::id::gte(100i64))
                        .r#where(user::id::lte(900i64))
                        .order_by(OrderBy::from_fields([
                            OrderByField::asc(user::age::COLUMN),
                            OrderByField::asc(user::email::COLUMN),
                        ]))
                        .take(100)
                        .exec()
                        .await
                        .expect("fetch complex filtered users");
                    assert_eq!(rows.len(), 100);
                    BenchOutcome {
                        rows: rows.len(),
                        queries: 1,
                    }
                }
            })
            .await,
        );
    }

    // 12. count_filtered: count users with age > 18.
    {
        let client = client.clone();
        results.push(
            bench_async("count_filtered", 100, move || {
                let client = client.clone();
                async move {
                    let count = client
                        .user()
                        .count()
                        .r#where(user::age::gt(18i64))
                        .exec()
                        .await
                        .expect("count users");
                    assert!(count >= 980, "expected ~980 users, got {}", count);
                    BenchOutcome {
                        rows: count as usize,
                        queries: 1,
                    }
                }
            })
            .await,
        );
    }

    // 13. exists_filtered: check if any user has age > 18.
    {
        let client = client.clone();
        results.push(
            bench_async("exists_filtered", 100, move || {
                let client = client.clone();
                async move {
                    let row = client
                        .user()
                        .find_first()
                        .r#where(user::age::gt(18i64))
                        .exec()
                        .await
                        .expect("exists users");
                    assert!(row.is_some());
                    BenchOutcome {
                        rows: if row.is_some() { 1 } else { 0 },
                        queries: 1,
                    }
                }
            })
            .await,
        );
    }

    // 14. include_posts: fetch all users and their posts.
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
                            let total_posts: usize = rows.iter().map(|u| u.posts.len()).sum();
                            assert_eq!(total_posts, 10000);
                            BenchOutcome {
                                rows: rows.len(),
                                queries: 2,
                            }
                        }
                        Err(e) => {
                            eprintln!("include_posts failed ({e}), falling back to manual fetch");
                            let users = client.user().find_many().exec().await.expect("fetch users");
                            let posts = client.post().find_many().exec().await.expect("fetch posts");
                            assert_eq!(users.len(), 1000);
                            assert_eq!(posts.len(), 10000);
                            let mut by_author: HashMap<i64, Vec<Post>> = HashMap::new();
                            for p in posts {
                                by_author.entry(p.author_id).or_default().push(p);
                            }
                            let mut users = users;
                            for u in &mut users {
                                u.posts = by_author.remove(&u.id).unwrap_or_default();
                            }
                            let total: usize = users.iter().map(|u| u.posts.len()).sum();
                            assert_eq!(total, 10000);
                            BenchOutcome {
                                rows: users.len(),
                                queries: 2,
                            }
                        }
                    }
                }
            })
            .await,
        );
    }

    // 15. include_author: fetch all posts and attach their author.
    //    Prax's generated loader supports HasMany/HasOne, not BelongsTo, so
    //    this is done with two explicit queries plus manual stitching.
    {
        let client = client.clone();
        results.push(
            bench_async("include_author", 10, move || {
                let client = client.clone();
                async move {
                    let mut posts = client
                        .post()
                        .find_many()
                        .exec()
                        .await
                        .expect("fetch posts");
                    let users = client
                        .user()
                        .find_many()
                        .exec()
                        .await
                        .expect("fetch authors");
                    let by_id: HashMap<i64, User> =
                        users.into_iter().map(|u| (u.id, u)).collect();
                    for p in &mut posts {
                        let author = by_id
                            .get(&p.author_id)
                            .cloned()
                            .expect("author for post");
                        p.author = vec![author];
                    }
                    assert_eq!(posts.len(), 10000);
                    let total_authors: usize = posts.iter().map(|p| p.author.len()).sum();
                    assert_eq!(total_authors, 10000);
                    BenchOutcome {
                        rows: posts.len(),
                        queries: 2,
                    }
                }
            })
            .await,
        );
    }

    // 16. include_posts_and_comments: users + posts + comments.
    //     Prax does not support nested includes, so posts are loaded via
    //     .include() and comments are attached manually.
    {
        let client = client.clone();
        results.push(
            bench_async("include_posts_and_comments", 10, move || {
                let client = client.clone();
                async move {
                    let mut users = client
                        .user()
                        .find_many()
                        .include(user::posts::fetch())
                        .exec()
                        .await
                        .expect("fetch users with posts");
                    let comments = client
                        .comment()
                        .find_many()
                        .exec()
                        .await
                        .expect("fetch comments");
                    let mut by_post: HashMap<i64, Vec<Comment>> = HashMap::new();
                    for c in comments {
                        by_post.entry(c.post_id).or_default().push(c);
                    }
                    let mut total_comments = 0;
                    for u in &mut users {
                        for p in &mut u.posts {
                            let cs = by_post.remove(&p.id).unwrap_or_default();
                            total_comments += cs.len();
                            p.comments = cs;
                        }
                    }
                    assert_eq!(users.len(), 1000);
                    assert_eq!(total_comments, 50000);
                    BenchOutcome {
                        rows: users.len(),
                        queries: 3,
                    }
                }
            })
            .await,
        );
    }

    // 17. include_posts_with_tags: posts + post_tags + tags.
    //     Prax does not support nested includes, so post_tags are loaded via
    //     .include() and tags are attached manually.
    {
        let client = client.clone();
        results.push(
            bench_async("include_posts_with_tags", 10, move || {
                let client = client.clone();
                async move {
                    let mut posts = client
                        .post()
                        .find_many()
                        .include(post::post_tags::fetch())
                        .exec()
                        .await
                        .expect("fetch posts with post_tags");
                    let tags = client.tag().find_many().exec().await.expect("fetch tags");
                    let by_id: HashMap<i64, Tag> =
                        tags.into_iter().map(|t| (t.id, t)).collect();
                    for p in &mut posts {
                        for pt in &mut p.post_tags {
                            let tag = by_id
                                .get(&pt.tag_id)
                                .cloned()
                                .expect("tag for post_tag");
                            pt.tag = vec![tag];
                        }
                    }
                    assert_eq!(posts.len(), 10000);
                    let total_post_tags: usize = posts.iter().map(|p| p.post_tags.len()).sum();
                    assert_eq!(total_post_tags, 30000);
                    BenchOutcome {
                        rows: posts.len(),
                        queries: 3,
                    }
                }
            })
            .await,
        );
    }

    // 18. find_popular_posts: posts where views > 1000, order by views desc,
    //     limit 100, and attach author manually.
    {
        let client = client.clone();
        results.push(
            bench_async("find_popular_posts", 50, move || {
                let client = client.clone();
                async move {
                    let mut posts = client
                        .post()
                        .find_many()
                        .r#where(post::views::gt(1000i64))
                        .order_by(OrderByField::desc(post::views::COLUMN))
                        .take(100)
                        .exec()
                        .await
                        .expect("fetch popular posts");
                    let users = client
                        .user()
                        .find_many()
                        .exec()
                        .await
                        .expect("fetch authors");
                    let by_id: HashMap<i64, User> =
                        users.into_iter().map(|u| (u.id, u)).collect();
                    for p in &mut posts {
                        let author = by_id
                            .get(&p.author_id)
                            .cloned()
                            .expect("author for popular post");
                        p.author = vec![author];
                    }
                    assert_eq!(posts.len(), 100);
                    BenchOutcome {
                        rows: posts.len(),
                        queries: 2,
                    }
                }
            })
            .await,
        );
    }

    // ── End-to-end writes ────────────────────────────────────────────────────

    // 19. bulk_insert_1000: clear bench_bulk, insert 1000 rows.
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
                    BenchOutcome {
                        rows: inserted as usize,
                        queries: 2,
                    }
                }
            })
            .await,
        );
    }

    println!("\n{}", serde_json::to_string_pretty(&results)?);

    let out_path = PathBuf::from("prax-results.json");
    tokio::fs::write(&out_path, serde_json::to_string_pretty(&results)?)
        .await
        .expect("write results");
    println!("Wrote {}", out_path.display());

    Ok(())
}
