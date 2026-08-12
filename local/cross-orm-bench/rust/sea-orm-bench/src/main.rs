use std::future::Future;
use std::path::Path;
use std::time::{Duration, Instant};

use sea_orm::entity::prelude::*;
use sea_orm::query::{Condition, LoaderTrait};
use sea_orm::{ActiveValue, Database, DbBackend, QueryOrder, QuerySelect, QueryTrait};
use serde::Serialize;
use simple_process_stats::ProcessStats;

mod entities;
use entities::{bench_bulk, comment, post, post_tag, tag, user};

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
        orm: "sea-orm".to_string(),
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
    Fut: Future<Output = BenchOutcome>,
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

fn resolve_db_path() -> String {
    if let Ok(p) = std::env::var("BENCH_SQLITE_PATH") {
        return p.replace('\\', "/");
    }

    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let rel = Path::new("local")
        .join("cross-orm-bench")
        .join("node")
        .join("bench.sqlite3");

    for ancestor in manifest.ancestors() {
        let candidate = ancestor.join(&rel);
        if candidate.exists() {
            return candidate.to_string_lossy().replace('\\', "/");
        }
    }

    manifest
        .ancestors()
        .nth(3)
        .map(|root| root.join(&rel).to_string_lossy().replace('\\', "/"))
        .unwrap_or_else(|| rel.to_string_lossy().replace('\\', "/"))
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let db_path = resolve_db_path();
    let url = format!("sqlite:///{}?mode=rwc", db_path);
    let db = Database::connect(&url).await?;

    let count = user::Entity::find().count(&db).await?;
    assert_eq!(count, 1000, "expected 1000 users in bench.sqlite3");

    let mut results = Vec::new();

    // Query construction (no I/O).
    results.push(bench_sync("to_sql_select_by_pk", 100_000, || {
        let sql = user::Entity::find()
            .filter(user::Column::Id.eq(500i64))
            .limit(1)
            .offset(0)
            .build(DbBackend::Sqlite)
            .to_string();
        std::hint::black_box(sql);
        BenchOutcome::default()
    }));

    results.push(bench_sync("to_sql_select_filter_order", 100_000, || {
        let sql = user::Entity::find()
            .filter(user::Column::Age.gt(18i64))
            .filter(user::Column::Email.contains("@example.com"))
            .order_by_asc(user::Column::Age)
            .order_by_asc(user::Column::Email)
            .limit(1000)
            .offset(0)
            .build(DbBackend::Sqlite)
            .to_string();
        std::hint::black_box(sql);
        BenchOutcome::default()
    }));

    results.push(bench_sync("to_sql_select_in_list", 100_000, || {
        let ids: Vec<i64> = (1..=50).collect();
        let sql = user::Entity::find()
            .filter(user::Column::Id.is_in(ids))
            .order_by_asc(user::Column::Id)
            .limit(50)
            .offset(0)
            .build(DbBackend::Sqlite)
            .to_string();
        std::hint::black_box(sql);
        BenchOutcome::default()
    }));

    results.push(bench_sync("to_sql_select_complex_filter", 100_000, || {
        let sql = user::Entity::find()
            .filter(
                Condition::all()
                    .add(user::Column::Age.gt(18i64))
                    .add(user::Column::Email.contains("example.com"))
                    .add(user::Column::Id.between(100i64, 900i64)),
            )
            .order_by_asc(user::Column::Age)
            .order_by_asc(user::Column::Email)
            .limit(100)
            .offset(0)
            .build(DbBackend::Sqlite)
            .to_string();
        std::hint::black_box(sql);
        BenchOutcome::default()
    }));

    results.push(bench_sync("to_sql_select_paginated", 100_000, || {
        let sql = user::Entity::find()
            .filter(user::Column::Age.gt(18i64))
            .filter(user::Column::Email.contains("example.com"))
            .order_by_asc(user::Column::Age)
            .order_by_asc(user::Column::Email)
            .limit(20)
            .offset(500)
            .build(DbBackend::Sqlite)
            .to_string();
        std::hint::black_box(sql);
        BenchOutcome::default()
    }));

    // End-to-end: select by PK.
    let db2 = db.clone();
    results.push(
        bench_async("select_by_pk", 1_000, move || {
            let db = db2.clone();
            async move {
                let row = user::Entity::find_by_id(500i64)
                    .one(&db)
                    .await
                    .expect("fetch one user")
                    .expect("user 500 not found");
                assert_eq!(row.id, 500);
                std::hint::black_box(row);
                BenchOutcome { rows: 1, queries: 1 }
            }
        })
        .await,
    );

    // End-to-end: find many 1000 rows.
    let db2 = db.clone();
    results.push(
        bench_async("find_many_1000", 50, move || {
            let db = db2.clone();
            async move {
                let rows = user::Entity::find()
                    .all(&db)
                    .await
                    .expect("fetch all users");
                assert_eq!(rows.len(), 1000);
                std::hint::black_box(rows);
                BenchOutcome {
                    rows: 1000,
                    queries: 1,
                }
            }
        })
        .await,
    );

    // End-to-end: filtered + ordered.
    let db2 = db.clone();
    results.push(
        bench_async("find_filtered_ordered", 50, move || {
            let db = db2.clone();
            async move {
                let rows = user::Entity::find()
                    .filter(user::Column::Age.gt(18i64))
                    .order_by_asc(user::Column::Age)
                    .order_by_asc(user::Column::Email)
                    .all(&db)
                    .await
                    .expect("fetch filtered users");
                assert!(rows.len() >= 980, "expected ~1000 users, got {}", rows.len());
                let n = rows.len();
                std::hint::black_box(rows);
                BenchOutcome { rows: n, queries: 1 }
            }
        })
        .await,
    );

    // End-to-end: filtered + ordered + paginated.
    let db2 = db.clone();
    results.push(
        bench_async("find_filtered_paginated", 50, move || {
            let db = db2.clone();
            async move {
                let rows = user::Entity::find()
                    .filter(user::Column::Age.gt(18i64))
                    .order_by_asc(user::Column::Age)
                    .order_by_asc(user::Column::Email)
                    .limit(20)
                    .offset(500)
                    .all(&db)
                    .await
                    .expect("fetch paginated users");
                assert_eq!(rows.len(), 20);
                BenchOutcome { rows: 20, queries: 1 }
            }
        })
        .await,
    );

    // End-to-end: IN list with 50 ids.
    let ids_50: Vec<i64> = (1..=50).collect();
    let db2 = db.clone();
    results.push(
        bench_async("find_in_list", 100, move || {
            let db = db2.clone();
            let ids = ids_50.clone();
            async move {
                let rows = user::Entity::find()
                    .filter(user::Column::Id.is_in(ids))
                    .order_by_asc(user::Column::Id)
                    .all(&db)
                    .await
                    .expect("fetch users by id list");
                assert_eq!(rows.len(), 50);
                BenchOutcome { rows: 50, queries: 1 }
            }
        })
        .await,
    );

    // End-to-end: complex filter with multiple parameters.
    let db2 = db.clone();
    results.push(
        bench_async("find_complex_filter", 50, move || {
            let db = db2.clone();
            async move {
                let rows = user::Entity::find()
                    .filter(
                        Condition::all()
                            .add(user::Column::Age.gt(18i64))
                            .add(user::Column::Email.contains("example.com"))
                            .add(user::Column::Id.between(100i64, 900i64)),
                    )
                    .order_by_asc(user::Column::Age)
                    .order_by_asc(user::Column::Email)
                    .limit(100)
                    .all(&db)
                    .await
                    .expect("fetch complex filtered users");
                assert_eq!(rows.len(), 100);
                BenchOutcome {
                    rows: 100,
                    queries: 1,
                }
            }
        })
        .await,
    );

    // End-to-end: count with filter.
    let db2 = db.clone();
    results.push(
        bench_async("count_filtered", 100, move || {
            let db = db2.clone();
            async move {
                let count = user::Entity::find()
                    .filter(user::Column::Age.gt(18i64))
                    .count(&db)
                    .await
                    .expect("count users");
                assert!(count >= 980, "expected ~1000 users, got {}", count);
                BenchOutcome {
                    rows: count as usize,
                    queries: 1,
                }
            }
        })
        .await,
    );

    // End-to-end: exists with filter.
    let db2 = db.clone();
    results.push(
        bench_async("exists_filtered", 100, move || {
            let db = db2.clone();
            async move {
                let exists = user::Entity::find()
                    .filter(user::Column::Age.gt(18i64))
                    .exists(&db)
                    .await
                    .expect("exists users");
                assert!(exists);
                BenchOutcome {
                    rows: if exists { 1 } else { 0 },
                    queries: 1,
                }
            }
        })
        .await,
    );

    // End-to-end: include posts for all users.
    let db2 = db.clone();
    results.push(
        bench_async("include_posts", 10, move || {
            let db = db2.clone();
            async move {
                let users = user::Entity::find().all(&db).await.expect("fetch users");
                assert_eq!(users.len(), 1000);
                let posts = users.load_many(post::Entity, &db).await.expect("load posts");
                let total_posts: usize = posts.iter().map(|p| p.len()).sum();
                assert_eq!(total_posts, 10_000);
                BenchOutcome {
                    rows: users.len(),
                    queries: 2,
                }
            }
        })
        .await,
    );

    // End-to-end: include author for all posts.
    let db2 = db.clone();
    results.push(
        bench_async("include_author", 10, move || {
            let db = db2.clone();
            async move {
                let posts = post::Entity::find().all(&db).await.expect("fetch posts");
                assert_eq!(posts.len(), 10_000);
                let _authors = posts
                    .load_one(user::Entity, &db)
                    .await
                    .expect("load authors");
                BenchOutcome {
                    rows: posts.len(),
                    queries: 2,
                }
            }
        })
        .await,
    );

    // End-to-end: include posts and their comments.
    let db2 = db.clone();
    results.push(
        bench_async("include_posts_and_comments", 10, move || {
            let db = db2.clone();
            async move {
                let users = user::Entity::find().all(&db).await.expect("fetch users");
                assert_eq!(users.len(), 1000);
                let posts_by_user = users.load_many(post::Entity, &db).await.expect("load posts");
                let total_posts: usize = posts_by_user.iter().map(|p| p.len()).sum();
                assert_eq!(total_posts, 10_000);

                let all_posts: Vec<post::Model> = posts_by_user
                    .into_iter()
                    .flat_map(|v| v.into_iter())
                    .collect();
                let comments = all_posts
                    .load_many(comment::Entity, &db)
                    .await
                    .expect("load comments");
                let total_comments: usize = comments.iter().map(|c| c.len()).sum();
                assert_eq!(total_comments, 50_000);

                BenchOutcome {
                    rows: users.len(),
                    queries: 3,
                }
            }
        })
        .await,
    );

    // End-to-end: posts with tags (many-to-many through post_tags).
    let db2 = db.clone();
    results.push(
        bench_async("include_posts_with_tags", 10, move || {
            let db = db2.clone();
            async move {
                let posts = post::Entity::find().all(&db).await.expect("fetch posts");
                assert_eq!(posts.len(), 10_000);
                let tags = posts
                    .load_many_to_many(tag::Entity, post_tag::Entity, &db)
                    .await
                    .expect("load tags");
                let total_post_tags: usize = tags.iter().map(|t| t.len()).sum();
                assert_eq!(total_post_tags, 30_000);
                BenchOutcome {
                    rows: posts.len(),
                    queries: 3,
                }
            }
        })
        .await,
    );

    // End-to-end: find popular posts and include author.
    let db2 = db.clone();
    results.push(
        bench_async("find_popular_posts", 50, move || {
            let db = db2.clone();
            async move {
                let posts = post::Entity::find()
                    .filter(post::Column::Views.gt(1000i64))
                    .order_by_desc(post::Column::Views)
                    .limit(100)
                    .all(&db)
                    .await
                    .expect("fetch popular posts");
                assert_eq!(posts.len(), 100);
                let _authors = posts
                    .load_one(user::Entity, &db)
                    .await
                    .expect("load authors");
                BenchOutcome {
                    rows: posts.len(),
                    queries: 2,
                }
            }
        })
        .await,
    );

    // Bulk insert 1000 rows into bench_bulk.
    let bulk_rows: Vec<_> = (1..=1000)
        .map(|i| bench_bulk::ActiveModel {
            id: ActiveValue::Set(i as i64),
            name: ActiveValue::Set(format!("bulk-{i}")),
            n: ActiveValue::Set(i as i64 * 3),
        })
        .collect();

    // Ensure a clean starting state.
    bench_bulk::Entity::delete_many().exec(&db).await?;

    let db2 = db.clone();
    results.push(
        bench_async("bulk_insert_1000", 10, move || {
            let db = db2.clone();
            let rows = bulk_rows.clone();
            async move {
                bench_bulk::Entity::delete_many()
                    .exec(&db)
                    .await
                    .expect("clear bench_bulk");
                let inserted = bench_bulk::Entity::insert_many(rows)
                    .exec_without_returning(&db)
                    .await
                    .expect("bulk insert");
                assert_eq!(inserted, 1000);
                BenchOutcome {
                    rows: 1000,
                    queries: 2,
                }
            }
        })
        .await,
    );

    println!("\n{}", serde_json::to_string_pretty(&results).unwrap());

    // Primary output in the harness directory (as requested).
    let crate_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("sea-orm-results.json");
    tokio::fs::write(
        &crate_path,
        serde_json::to_string_pretty(&results).unwrap(),
    )
    .await
    .expect("write crate results");
    println!("Wrote {}", crate_path.display());

    // Secondary copy next to the SQLite file for the shared runner.
    if let Some(parent) = Path::new(&db_path).parent() {
        let node_path = parent.join("sea-orm-results.json");
        tokio::fs::write(&node_path, serde_json::to_string_pretty(&results).unwrap())
            .await
            .expect("write node results");
        println!("Wrote {}", node_path.display());
    }

    Ok(())
}
