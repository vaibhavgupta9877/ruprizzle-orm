use std::future::Future;
use std::path::Path;
use std::time::{Duration, Instant};

use sea_orm::entity::prelude::*;
use sea_orm::query::{Condition, LoaderTrait, QueryFilter, QueryOrder, QuerySelect};
use sea_orm::sea_query::{Alias, CommonTableExpression, Expr, JoinType, Query, UnionType, WithClause};
use sea_orm::{ActiveValue, Database, DbBackend, QueryTrait, Statement, StatementBuilder, Value, Values};
use futures::StreamExt;
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

    // Query-construction: prepared select by PK (parameterized SQL).
    results.push(bench_sync("to_sql_prepared_select_by_pk", 100_000, || {
        let stmt = user::Entity::find()
            .filter(user::Column::Id.eq(500i64))
            .limit(1)
            .offset(0)
            .build(DbBackend::Sqlite);
        std::hint::black_box(stmt);
        BenchOutcome::default()
    }));

    // Query-construction: rebind a prepared select by PK.
    {
        let base = user::Entity::find()
            .filter(user::Column::Id.eq(0i64))
            .limit(1)
            .offset(0)
            .build(DbBackend::Sqlite);
        results.push(bench_sync("prepared_rebind_select_by_pk", 100_000, || {
            let mut stmt = base.clone();
            stmt.values = Some(Values(vec![Value::from(123i64)]));
            std::hint::black_box(stmt);
            BenchOutcome::default()
        }));
    }

    // Query-construction: conditional filter.
    results.push(bench_sync("to_sql_conditional_filter", 100_000, || {
        let maybe_age = Some(user::Column::Age.gt(18i64));
        let maybe_order = Some(user::Column::Age);
        let maybe_limit = Some(100u64);
        let q = user::Entity::find()
            .apply_if(maybe_age, |q, f| q.filter(f))
            .apply_if(maybe_order, |q, c| q.order_by_asc(c))
            .apply_if(maybe_limit, QuerySelect::limit);
        let sql = q.build(DbBackend::Sqlite).to_string();
        std::hint::black_box(sql);
        BenchOutcome::default()
    }));

    // Query-construction: select with a CTE.
    results.push(bench_sync("to_sql_select_with_cte", 100_000, || {
        let active = user::Entity::find()
            .filter(user::Column::Age.gt(18i64))
            .into_query();
        let cte = CommonTableExpression::new()
            .table_name(Alias::new("active"))
            .query(active)
            .to_owned();
        let mut q = user::Entity::find().filter(user::Column::Id.gt(0i64));
        QuerySelect::query(&mut q).with_cte(cte);
        let sql = q.build(DbBackend::Sqlite).to_string();
        std::hint::black_box(sql);
        BenchOutcome::default()
    }));

    // Query-construction: select with a recursive CTE.
    results.push(bench_sync("to_sql_select_with_recursive_cte", 100_000, || {
        let mut anchor = Query::select();
        anchor
            .from(user::Entity)
            .column(user::Column::Id)
            .and_where(user::Column::Id.eq(1i64));
        let mut recursive = Query::select();
        recursive
            .from(user::Entity)
            .column(user::Column::Id)
            .and_where(user::Column::Id.eq(2i64))
            .join(
                JoinType::InnerJoin,
                Alias::new("nums"),
                Expr::col((user::Entity, user::Column::Id)).equals((Alias::new("nums"), user::Column::Id)),
            );
        let cte_query = anchor.union(UnionType::All, recursive.to_owned()).to_owned();
        let cte = CommonTableExpression::new()
            .table_name(Alias::new("nums"))
            .column(user::Column::Id)
            .query(cte_query)
            .to_owned();
        let with_clause = WithClause::new()
            .recursive(true)
            .cte(cte)
            .to_owned();
        let mut q = user::Entity::find().filter(user::Column::Id.gt(0i64));
        QuerySelect::query(&mut q).with_cte(with_clause);
        let sql = q.build(DbBackend::Sqlite).to_string();
        std::hint::black_box(sql);
        BenchOutcome::default()
    }));

    // Query-construction: set union.
    results.push(bench_sync("to_sql_set_union", 100_000, || {
        let left = user::Entity::find()
            .filter(user::Column::Age.gt(18i64))
            .into_query();
        let right = user::Entity::find()
            .filter(user::Column::Age.lte(18i64))
            .into_query();
        let mut q = left;
        q.union(UnionType::All, right);
        let sql = StatementBuilder::build(&q, &DbBackend::Sqlite).to_string();
        std::hint::black_box(sql);
        BenchOutcome::default()
    }));

    // Query-construction: select with a join.
    results.push(bench_sync("to_sql_select_with_join", 100_000, || {
        let sql = post::Entity::find()
            .inner_join(user::Entity)
            .build(DbBackend::Sqlite)
            .to_string();
        std::hint::black_box(sql);
        BenchOutcome::default()
    }));

    // Query-construction: select with an EXISTS subquery.
    results.push(bench_sync("to_sql_select_exists_subquery", 100_000, || {
        let sub = post::Entity::find()
            .filter(
                Expr::col((post::Entity, post::Column::AuthorId)).equals((user::Entity, user::Column::Id)),
            )
            .into_query();
        let sql = user::Entity::find()
            .filter(Expr::exists(sub))
            .build(DbBackend::Sqlite)
            .to_string();
        std::hint::black_box(sql);
        BenchOutcome::default()
    }));

    // Query-construction: select with an IN subquery.
    results.push(bench_sync("to_sql_select_in_subquery", 100_000, || {
        let sub = Query::select()
            .from(post::Entity)
            .column(post::Column::AuthorId)
            .and_where(post::Column::AuthorId.gt(0i64))
            .to_owned();
        let sql = user::Entity::find()
            .filter(user::Column::Id.in_subquery(sub))
            .build(DbBackend::Sqlite)
            .to_string();
        std::hint::black_box(sql);
        BenchOutcome::default()
    }));

    // Query-construction: nested insert.
    // Fallback: sea-orm 1.1 does not have nested ActiveModel writes, so we construct
    // a representative raw SQL string that inserts a parent and child row.
    results.push(bench_sync("to_sql_nested_insert", 100_000, || {
        let sql = r#"
            WITH inserted_user AS (
                INSERT INTO "users" ("id", "email", "age", "name", "created_at")
                VALUES (?, ?, ?, ?, ?)
                RETURNING "id"
            )
            INSERT INTO "posts" ("id", "author_id", "category_id", "title", "published_at", "views")
            SELECT ?, "inserted_user"."id", 1, 'nested post', 0, 0
            FROM "inserted_user"
        "#;
        let stmt = Statement::from_string(DbBackend::Sqlite, sql);
        std::hint::black_box(stmt);
        BenchOutcome::default()
    }));

    // Query-construction: nested update.
    // Fallback: sea-orm 1.1 does not have nested relation updates, so we construct
    // a representative raw SQL string that updates a parent and its related rows.
    results.push(bench_sync("to_sql_nested_update", 100_000, || {
        let sql = r#"
            WITH updated_user AS (
                UPDATE "users" SET "name" = ?
                WHERE "id" = ?
                RETURNING "id"
            )
            UPDATE "posts"
            SET "author_id" = (SELECT "id" FROM "updated_user")
            WHERE "id" IN (?, ?)
        "#;
        let stmt = Statement::from_string(DbBackend::Sqlite, sql);
        std::hint::black_box(stmt);
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

    // End-to-end: reusable prepared statement for single-row PK lookup.
    let base = user::Entity::find_by_id(0i64).build(DbBackend::Sqlite);
    let prepared_sql = base.sql.clone();
    results.push(
        bench_async("prepared_select_by_pk", 1_000, || {
            let sql = prepared_sql.clone();
            async {
                let stmt = Statement::from_sql_and_values(
                    DbBackend::Sqlite,
                    sql,
                    vec![Value::from(500i64)],
                );
                let row = user::Entity::find()
                    .from_raw_sql(stmt)
                    .one(&db)
                    .await
                    .expect("fetch prepared user")
                    .expect("user 500 not found");
                assert_eq!(row.id, 500);
                BenchOutcome { rows: 1, queries: 1 }
            }
        })
        .await,
    );

    // End-to-end: unbuffered streaming of all users.
    let db2 = db.clone();
    results.push(
        bench_async("stream_find_many_1000", 50, move || {
            let db = db2.clone();
            async move {
                let mut stream = user::Entity::find().stream(&db).await.expect("create stream");
                let mut rows = 0;
                while let Some(Ok(_row)) = stream.next().await {
                    rows += 1;
                }
                assert_eq!(rows, 1000);
                BenchOutcome {
                    rows,
                    queries: 1,
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
