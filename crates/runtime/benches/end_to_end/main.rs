//! End-to-end benchmarks against a real database.
//!
//! The interesting number is our overhead versus hand-written sqlx, not our
//! speed versus another ORM on different hardware. Every case therefore has a
//! hand-written comparison arm.
//!
//! Skipped when no database is reachable, so `cargo bench` still works offline.
//!
//! The ruprizzle client for the bench schema is generated into this directory
//! and `include!`-ed below.

#![allow(dead_code, unused_imports)]
#![forbid(unsafe_code)]

include!("mod.rs");

use std::collections::HashMap;
use std::time::Duration;

use criterion::{Criterion, criterion_group, criterion_main};
use ruprizzle::{PoolConfig, connect_with};
use tokio::runtime::Runtime;

const PARENTS: i64 = 100;
const CHILDREN_PER_PARENT: i64 = 10;
const GRANDCHILDREN_PER_CHILD: i64 = 10;
const BULK_ROWS: i64 = 10_000;

fn pg_url() -> Option<String> {
    std::env::var("RUPRIZZLE_TEST_PG_URL")
        .or_else(|_| std::env::var("DATABASE_URL"))
        .ok()
}

fn in_sql(table: &str, columns: &[&str], in_col: &str, in_count: usize) -> String {
    let mut sql = String::from("SELECT ");
    for (i, col) in columns.iter().enumerate() {
        if i > 0 {
            sql.push_str(", ");
        }
        sql.push_str(col);
    }
    sql.push_str(" FROM ");
    sql.push_str(table);
    sql.push_str(" WHERE ");
    sql.push_str(in_col);
    sql.push_str(" IN (");
    for i in 0..in_count {
        if i > 0 {
            sql.push_str(", ");
        }
        sql.push_str(&format!("${}", i + 1));
    }
    sql.push(')');
    sql
}

fn bulk_insert_sql(table: &str, columns: &[&str], rows: usize, returning: &[&str]) -> String {
    let mut sql = String::from("INSERT INTO ");
    sql.push_str(table);
    sql.push_str(" (");
    for (i, col) in columns.iter().enumerate() {
        if i > 0 {
            sql.push_str(", ");
        }
        sql.push_str(col);
    }
    sql.push_str(") VALUES ");
    for r in 0..rows {
        if r > 0 {
            sql.push_str(", ");
        }
        sql.push('(');
        for c in 0..columns.len() {
            if c > 0 {
                sql.push_str(", ");
            }
            let idx = r * columns.len() + c + 1;
            sql.push_str(&format!("${idx}"));
        }
        sql.push(')');
    }
    if !returning.is_empty() {
        sql.push_str(" RETURNING ");
        for (i, col) in returning.iter().enumerate() {
            if i > 0 {
                sql.push_str(", ");
            }
            sql.push_str(col);
        }
    }
    sql
}

async fn setup(pool: &ruprizzle::Pool) {
    sqlx::query("DROP TABLE IF EXISTS bench_rows, bench_bulk, bench_parents, bench_children, bench_grandchildren")
        .execute(pool)
        .await
        .expect("drop");
    sqlx::query(
        "CREATE TABLE bench_rows (id BIGINT PRIMARY KEY, name TEXT NOT NULL, n BIGINT NOT NULL)",
    )
    .execute(pool)
    .await
    .expect("create bench_rows");
    sqlx::query(
        "CREATE TABLE bench_bulk (id BIGINT PRIMARY KEY, name TEXT NOT NULL, n BIGINT NOT NULL)",
    )
    .execute(pool)
    .await
    .expect("create bench_bulk");
    sqlx::query("CREATE TABLE bench_parents (id BIGINT PRIMARY KEY, name TEXT NOT NULL)")
        .execute(pool)
        .await
        .expect("create bench_parents");
    sqlx::query(
        "CREATE TABLE bench_children (id BIGINT PRIMARY KEY, parent_id BIGINT NOT NULL, name TEXT NOT NULL)",
    )
    .execute(pool)
    .await
    .expect("create bench_children");
    sqlx::query(
        "CREATE TABLE bench_grandchildren (id BIGINT PRIMARY KEY, child_id BIGINT NOT NULL, name TEXT NOT NULL)",
    )
    .execute(pool)
    .await
    .expect("create bench_grandchildren");

    let rows: Vec<(i64, String, i64)> = (0..1_000i64)
        .map(|i| (i, format!("row-{i}"), i * 2))
        .collect();
    let sql = bulk_insert_sql("bench_rows", &["id", "name", "n"], rows.len(), &[]);
    let mut q = sqlx::query(&sql);
    for (id, name, n) in &rows {
        q = q.bind(*id).bind(name).bind(*n);
    }
    q.execute(pool).await.expect("seed bench_rows");

    let parents: Vec<(i64, String)> = (1..=PARENTS).map(|i| (i, format!("parent-{i}"))).collect();
    let parent_sql = bulk_insert_sql("bench_parents", &["id", "name"], parents.len(), &[]);
    let mut q = sqlx::query(&parent_sql);
    for (id, name) in &parents {
        q = q.bind(*id).bind(name);
    }
    q.execute(pool).await.expect("seed bench_parents");

    let children: Vec<(i64, i64, String)> = (1..=PARENTS * CHILDREN_PER_PARENT)
        .map(|i| {
            let parent_id = ((i - 1) / CHILDREN_PER_PARENT) + 1;
            (i, parent_id, format!("child-{i}"))
        })
        .collect();
    let child_sql = bulk_insert_sql(
        "bench_children",
        &["id", "parent_id", "name"],
        children.len(),
        &[],
    );
    let mut q = sqlx::query(&child_sql);
    for (id, parent_id, name) in &children {
        q = q.bind(*id).bind(*parent_id).bind(name);
    }
    q.execute(pool).await.expect("seed bench_children");

    let grandchildren: Vec<(i64, i64, String)> =
        (1..=PARENTS * CHILDREN_PER_PARENT * GRANDCHILDREN_PER_CHILD)
            .map(|i| {
                let child_id = ((i - 1) / GRANDCHILDREN_PER_CHILD) + 1;
                (i, child_id, format!("grandchild-{i}"))
            })
            .collect();
    let grandchild_sql = bulk_insert_sql(
        "bench_grandchildren",
        &["id", "child_id", "name"],
        grandchildren.len(),
        &[],
    );
    let mut q = sqlx::query(&grandchild_sql);
    for (id, child_id, name) in &grandchildren {
        q = q.bind(*id).bind(*child_id).bind(name);
    }
    q.execute(pool).await.expect("seed bench_grandchildren");
}

fn bench_end_to_end(c: &mut Criterion) {
    let Some(url) = pg_url() else {
        eprintln!("skipping end_to_end benches: no RUPRIZZLE_TEST_PG_URL");
        return;
    };

    let rt = Runtime::new().expect("tokio runtime");
    let mut config = PoolConfig::default();
    config.min_connections = 4;
    config.max_connections = 4;
    let pool = match rt.block_on(connect_with(&url, &config)) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("skipping end_to_end benches: could not connect to `{url}`: {e}");
            return;
        }
    };

    rt.block_on(setup(&pool));

    let db = Db::from_pool(pool.clone());

    let mut group = c.benchmark_group("end_to_end");
    group.measurement_time(Duration::from_secs(10));

    // Single-row select by primary key.
    group.bench_function("sqlx_single_row_by_pk", |b| {
        b.iter(|| {
            rt.block_on(async {
                let _: (i64, String, i64) =
                    sqlx::query_as("SELECT id, name, n FROM bench_rows WHERE id = $1")
                        .bind(500i64)
                        .fetch_one(&pool)
                        .await
                        .expect("fetch");
            });
        });
    });

    group.bench_function("ruprizzle_single_row_by_pk", |b| {
        b.iter(|| {
            rt.block_on(async {
                let row = db
                    .bench_row()
                    .find_many()
                    .filter(crate::bench_row::ID.eq(500i64))
                    .fetch_one()
                    .await
                    .expect("fetch");
                assert_eq!(row.id, 500);
            });
        });
    });

    // 1 000-row select.
    group.bench_function("sqlx_thousand_rows", |b| {
        b.iter(|| {
            rt.block_on(async {
                let rows: Vec<(i64, String, i64)> =
                    sqlx::query_as("SELECT id, name, n FROM bench_rows")
                        .fetch_all(&pool)
                        .await
                        .expect("fetch");
                assert_eq!(rows.len(), 1_000);
            });
        });
    });

    group.bench_function("ruprizzle_thousand_rows", |b| {
        b.iter(|| {
            rt.block_on(async {
                let rows: Vec<crate::BenchRow> =
                    db.bench_row().find_many().fetch_all().await.expect("fetch");
                assert_eq!(rows.len(), 1_000);
            });
        });
    });

    // 2-level include.
    group.bench_function("sqlx_two_level_include", |b| {
        let parent_sql = "SELECT id, name FROM bench_parents".to_string();
        b.iter(|| {
            rt.block_on(async {
                let parents: Vec<(i64, String)> = sqlx::query_as(&parent_sql)
                    .fetch_all(&pool)
                    .await
                    .expect("fetch parents");
                let parent_ids: Vec<i64> = parents.iter().map(|p| p.0).collect();

                let children_sql = in_sql(
                    "bench_children",
                    &["id", "parent_id", "name"],
                    "parent_id",
                    parent_ids.len(),
                );
                let mut q = sqlx::query_as::<_, (i64, i64, String)>(&children_sql);
                for id in &parent_ids {
                    q = q.bind(*id);
                }
                let children: Vec<(i64, i64, String)> =
                    q.fetch_all(&pool).await.expect("fetch children");

                let child_ids: Vec<i64> = children.iter().map(|c| c.0).collect();
                let grandchildren_sql = in_sql(
                    "bench_grandchildren",
                    &["id", "child_id", "name"],
                    "child_id",
                    child_ids.len(),
                );
                let mut q = sqlx::query_as::<_, (i64, i64, String)>(&grandchildren_sql);
                for id in &child_ids {
                    q = q.bind(*id);
                }
                let grandchildren: Vec<(i64, i64, String)> =
                    q.fetch_all(&pool).await.expect("fetch grandchildren");

                let mut child_map: HashMap<i64, Vec<(i64, i64, String)>> = HashMap::new();
                let mut grandchild_map: HashMap<i64, Vec<(i64, i64, String)>> = HashMap::new();
                for c in children {
                    child_map.entry(c.1).or_default().push(c);
                }
                for g in grandchildren {
                    grandchild_map.entry(g.1).or_default().push(g);
                }

                let mut total_children = 0usize;
                let mut total_grandchildren = 0usize;
                for p in &parents {
                    if let Some(cs) = child_map.get(&p.0) {
                        total_children += cs.len();
                        for c in cs {
                            if let Some(gs) = grandchild_map.get(&c.0) {
                                total_grandchildren += gs.len();
                            }
                        }
                    }
                }
                assert_eq!(parents.len(), 100);
                assert_eq!(total_children, 1_000);
                assert_eq!(total_grandchildren, 10_000);
            });
        });
    });

    group.bench_function("ruprizzle_two_level_include", |b| {
        b.iter(|| {
            rt.block_on(async {
                let parents = db
                    .bench_parent()
                    .find_many()
                    .include(
                        crate::bench_parent::children()
                            .include(crate::bench_child::grandchildren()),
                    )
                    .exec()
                    .await
                    .expect("exec");
                assert_eq!(parents.len(), 100);
                let total_children: usize = parents.iter().map(|p| p.children.get().len()).sum();
                let total_grandchildren: usize = parents
                    .iter()
                    .map(|p| {
                        p.children
                            .get()
                            .iter()
                            .map(|c| c.grandchildren.get().len())
                            .sum::<usize>()
                    })
                    .sum();
                assert_eq!(total_children, 1_000);
                assert_eq!(total_grandchildren, 10_000);
            });
        });
    });

    // Bulk insert 10 000 rows.
    let bulk_data: Vec<BenchBulkInsert> = (0..BULK_ROWS)
        .map(|i| BenchBulkInsert {
            id: i,
            name: format!("bulk-{i}"),
            n: i * 3,
        })
        .collect();
    let bulk_sql = bulk_insert_sql(
        "bench_bulk",
        &["id", "name", "n"],
        bulk_data.len(),
        &["id", "name", "n"],
    );

    group.bench_function("sqlx_bulk_insert_10000", |b| {
        b.iter(|| {
            rt.block_on(async {
                sqlx::query("TRUNCATE TABLE bench_bulk")
                    .execute(&pool)
                    .await
                    .expect("truncate");
                let mut q = sqlx::query_as::<_, (i64, String, i64)>(&bulk_sql);
                for row in &bulk_data {
                    q = q.bind(row.id).bind(row.name.as_str()).bind(row.n);
                }
                let rows: Vec<(i64, String, i64)> = q.fetch_all(&pool).await.expect("insert");
                assert_eq!(rows.len(), 10_000);
            });
        });
    });

    group.bench_function("ruprizzle_bulk_insert_10000", |b| {
        b.iter(|| {
            rt.block_on(async {
                sqlx::query("TRUNCATE TABLE bench_bulk")
                    .execute(&pool)
                    .await
                    .expect("truncate");
                let rows: Vec<crate::BenchBulk> =
                    ruprizzle::InsertManyQuery::<BenchBulk>::new(&pool)
                        .rows(bulk_data.iter().map(|row| {
                            [
                                ("id", ruprizzle::Encodable::to_value(&row.id)),
                                ("name", ruprizzle::Encodable::to_value(&row.name)),
                                ("n", ruprizzle::Encodable::to_value(&row.n)),
                            ]
                        }))
                        .exec()
                        .await
                        .expect("insert");
                assert_eq!(rows.len(), 10_000);
            });
        });
    });

    group.finish();
}

criterion_group!(benches, bench_end_to_end);
criterion_main!(benches);
