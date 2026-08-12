use std::future::Future;
use std::path::Path;
use std::time::Instant;

use sea_orm::entity::prelude::*;
use sea_orm::{ActiveValue, Database, DbBackend, QueryOrder, QueryTrait};
use serde::Serialize;

mod entities;
use entities::{bench_bulk, post, user};

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
        orm: "sea-orm".to_string(),
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
    Fut: Future<Output = ()>,
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

    // Fallback: repo root is the grandparent of cross-orm-bench/rust/sea-orm-bench
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
            .build(DbBackend::Sqlite)
            .to_string();
        std::hint::black_box(sql);
    }));

    results.push(bench_sync("to_sql_select_filter_order", 100_000, || {
        let sql = user::Entity::find()
            .filter(user::Column::Age.gt(18i64))
            .order_by_asc(user::Column::Age)
            .order_by_asc(user::Column::Email)
            .build(DbBackend::Sqlite)
            .to_string();
        std::hint::black_box(sql);
    }));

    // End-to-end: select by PK.
    let db2 = db.clone();
    results.push(bench_async("select_by_pk", 1_000, move || {
        let db = db2.clone();
        async move {
            let row = user::Entity::find_by_id(500i64)
                .one(&db)
                .await
                .expect("fetch one user")
                .expect("user 500 not found");
            assert_eq!(row.id, 500);
            std::hint::black_box(row);
        }
    }).await);

    // End-to-end: find many 1000 rows.
    let db2 = db.clone();
    results.push(bench_async("find_many_1000", 50, move || {
        let db = db2.clone();
        async move {
            let rows = user::Entity::find()
                .all(&db)
                .await
                .expect("fetch all users");
            assert_eq!(rows.len(), 1000);
            std::hint::black_box(rows);
        }
    }).await);

    // End-to-end: filtered + ordered.
    let db2 = db.clone();
    results.push(bench_async("find_filtered_ordered", 50, move || {
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
            std::hint::black_box(rows);
        }
    }).await);

    // End-to-end: include posts for all users.
    let db2 = db.clone();
    results.push(bench_async("include_posts", 10, move || {
        let db = db2.clone();
        async move {
            let rows: Vec<(user::Model, Vec<post::Model>)> = user::Entity::find()
                .find_with_related(post::Entity)
                .all(&db)
                .await
                .expect("fetch users with posts");
            assert_eq!(rows.len(), 1000);
            let total_posts: usize = rows.iter().map(|(_, posts)| posts.len()).sum();
            assert_eq!(total_posts, 10_000);
            std::hint::black_box(rows);
        }
    }).await);

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
    results.push(bench_async("bulk_insert_1000", 10, move || {
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
            std::hint::black_box(inserted);
        }
    }).await);

    println!("\n{}", serde_json::to_string_pretty(&results).unwrap());

    let output_path = Path::new(&db_path)
        .parent()
        .unwrap()
        .join("sea-orm-results.json");
    tokio::fs::write(
        &output_path,
        serde_json::to_string_pretty(&results).unwrap(),
    )
    .await
    .expect("write results");
    println!("Wrote {}", output_path.display());

    Ok(())
}
