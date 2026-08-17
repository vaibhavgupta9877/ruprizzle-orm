//! Diesel benchmark for cross-orm SQLite bench.
//!
//! Mirrors the ruprizzle `cross_orm_bench.rs` harness and writes an identical
//! JSON result file.

use std::collections::HashMap;
use std::env;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use diesel::dsl::exists;
use diesel::prelude::*;
use diesel::query_dsl::CombineDsl;
use diesel::sql_types::{BigInt, Text};
use diesel::sqlite::Sqlite;
use serde::Serialize;
use simple_process_stats::ProcessStats;

mod schema {
    diesel::table! {
        users (id) {
            id -> BigInt,
            email -> Text,
            age -> BigInt,
            name -> Text,
            created_at -> BigInt,
        }
    }

    diesel::table! {
        categories (id) {
            id -> BigInt,
            name -> Text,
        }
    }

    diesel::table! {
        posts (id) {
            id -> BigInt,
            author_id -> BigInt,
            category_id -> BigInt,
            title -> Text,
            published_at -> BigInt,
            views -> BigInt,
        }
    }

    diesel::table! {
        comments (id) {
            id -> BigInt,
            post_id -> BigInt,
            author_id -> BigInt,
            content -> Text,
            created_at -> BigInt,
        }
    }

    diesel::table! {
        tags (id) {
            id -> BigInt,
            name -> Text,
        }
    }

    diesel::table! {
        post_tags (post_id, tag_id) {
            post_id -> BigInt,
            tag_id -> BigInt,
        }
    }

    diesel::table! {
        followers (follower_id, followee_id) {
            follower_id -> BigInt,
            followee_id -> BigInt,
            created_at -> BigInt,
        }
    }

    diesel::table! {
        likes (id) {
            id -> BigInt,
            user_id -> BigInt,
            post_id -> BigInt,
            created_at -> BigInt,
        }
    }

    diesel::table! {
        bench_bulk (id) {
            id -> BigInt,
            name -> Text,
            n -> BigInt,
        }
    }

    diesel::allow_tables_to_appear_in_same_query!(
        users,
        categories,
        posts,
        comments,
        tags,
        post_tags,
        followers,
        likes,
        bench_bulk
    );
}

use schema::*;

#[derive(Queryable, Debug, Clone)]
#[allow(dead_code)]
struct User {
    id: i64,
    email: String,
    age: i64,
    name: String,
    created_at: i64,
}

#[derive(Queryable, Debug, Clone)]
#[allow(dead_code)]
struct Category {
    id: i64,
    name: String,
}

#[derive(Queryable, Debug, Clone)]
#[allow(dead_code)]
struct Post {
    id: i64,
    author_id: i64,
    category_id: i64,
    title: String,
    published_at: i64,
    views: i64,
}

#[derive(Queryable, Debug, Clone)]
#[allow(dead_code)]
struct Comment {
    id: i64,
    post_id: i64,
    author_id: i64,
    content: String,
    created_at: i64,
}

#[derive(Queryable, Debug, Clone)]
#[allow(dead_code)]
struct Tag {
    id: i64,
    name: String,
}

#[derive(Queryable, Debug, Clone)]
#[allow(dead_code)]
struct PostTag {
    post_id: i64,
    tag_id: i64,
}

#[derive(Queryable, Debug, Clone)]
#[allow(dead_code)]
struct Follower {
    follower_id: i64,
    followee_id: i64,
    created_at: i64,
}

#[derive(Queryable, Debug, Clone)]
#[allow(dead_code)]
struct Like {
    id: i64,
    user_id: i64,
    post_id: i64,
    created_at: i64,
}

#[derive(Queryable, Insertable, Debug, Clone)]
#[diesel(table_name = bench_bulk)]
#[allow(dead_code)]
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
    let user = after.cpu_time_user.saturating_sub(before.cpu_time_user);
    let kernel = after.cpu_time_kernel.saturating_sub(before.cpu_time_kernel);
    let cpu = user + kernel;
    let cpu_time_ms = cpu.as_secs_f64() * 1000.0;
    let peak = before.memory_usage_bytes.max(after.memory_usage_bytes);
    let peak_rss_mb = peak as f64 / (1024.0 * 1024.0);
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

    // Query construction: SELECT ... WHERE id = 500 LIMIT 1
    results.push(bench_sync("to_sql_select_by_pk", 100_000, || {
        let query = users::table
            .filter(users::id.eq(500i64))
            .limit(1);
        let sql = diesel::debug_query::<Sqlite, _>(&query).to_string();
        std::hint::black_box(sql);
        BenchOutcome::default()
    }));

    // Query construction: filter + order + pagination with offset 0.
    results.push(bench_sync("to_sql_select_filter_order", 100_000, || {
        let query = users::table
            .filter(users::age.gt(18i64).and(users::email.like("%@example.com%")))
            .order_by(users::age.asc())
            .then_order_by(users::email.asc())
            .limit(1000)
            .offset(0);
        let sql = diesel::debug_query::<Sqlite, _>(&query).to_string();
        std::hint::black_box(sql);
        BenchOutcome::default()
    }));

    // Query construction: IN list with 50 ids.
    let ids_50: Vec<i64> = (1..=50).collect();
    results.push(bench_sync("to_sql_select_in_list", 100_000, || {
        let query = users::table
            .filter(users::id.eq_any(&ids_50))
            .order_by(users::id.asc())
            .limit(50);
        let sql = diesel::debug_query::<Sqlite, _>(&query).to_string();
        std::hint::black_box(sql);
        BenchOutcome::default()
    }));

    // Query construction: complex filter with BETWEEN.
    results.push(bench_sync("to_sql_select_complex_filter", 100_000, || {
        let query = users::table
            .filter(
                users::age
                    .gt(18i64)
                    .and(users::email.like("%example.com%"))
                    .and(users::id.between(100i64, 900i64)),
            )
            .order_by(users::age.asc())
            .then_order_by(users::email.asc())
            .limit(100);
        let sql = diesel::debug_query::<Sqlite, _>(&query).to_string();
        std::hint::black_box(sql);
        BenchOutcome::default()
    }));

    // Query construction: paginated.
    results.push(bench_sync("to_sql_select_paginated", 100_000, || {
        let query = users::table
            .filter(users::age.gt(18i64).and(users::email.like("%example.com%")))
            .order_by(users::age.asc())
            .then_order_by(users::email.asc())
            .limit(20)
            .offset(500);
        let sql = diesel::debug_query::<Sqlite, _>(&query).to_string();
        std::hint::black_box(sql);
        BenchOutcome::default()
    }));

    // Query-construction: prepared SELECT ... WHERE id = ? LIMIT 1 OFFSET 0
    let target_id = 500i64;
    results.push(bench_sync("to_sql_prepared_select_by_pk", 100_000, || {
        let query = users::table
            .filter(users::id.eq(target_id))
            .limit(1)
            .offset(0);
        let sql = diesel::debug_query::<Sqlite, _>(&query).to_string();
        std::hint::black_box(sql);
        BenchOutcome::default()
    }));

    // Query-construction: rebind a prepared raw SQL template with a new id.
    // Diesel does not expose a mutable prepared-statement rebind, so we clone
    // a base `SqlQuery` template and push a fresh bind value each iteration.
    {
        let prepared = diesel::sql_query(
            "SELECT id, email, age, name, created_at FROM users WHERE id = ? LIMIT 1 OFFSET 0",
        );
        let mut id = 500i64;
        results.push(bench_sync("prepared_rebind_select_by_pk", 100_000, || {
            let q = prepared.clone().bind::<BigInt, _>(id);
            let sql = diesel::debug_query::<Sqlite, _>(&q).to_string();
            std::hint::black_box(sql);
            id = (id % 1000) + 1;
            BenchOutcome::default()
        }));
    }

    // Query-construction: conditionally apply filter/order/limit to a boxed query.
    results.push(bench_sync("to_sql_conditional_filter", 100_000, || {
        let mut query = users::table.into_boxed::<Sqlite>();
        let maybe_age = Some(true);
        let maybe_order = Some(true);
        let maybe_limit: Option<i64> = Some(100);
        if maybe_age.is_some() {
            query = query.filter(users::age.gt(18i64));
        }
        if maybe_order.is_some() {
            query = query.order_by(users::age.asc());
        }
        if let Some(limit) = maybe_limit {
            query = query.limit(limit);
        }
        let sql = diesel::debug_query::<Sqlite, _>(&query).to_string();
        std::hint::black_box(sql);
        BenchOutcome::default()
    }));

    // Query-construction: non-recursive CTE.
    // Diesel has no high-level CTE query builder, so this falls back to `sql_query`.
    results.push(bench_sync("to_sql_select_with_cte", 100_000, || {
        let query = diesel::sql_query(
            "WITH active AS (SELECT id, email, age, name, created_at FROM users WHERE age > ?) \
             SELECT * FROM active WHERE id > ?",
        )
        .bind::<BigInt, _>(18i64)
        .bind::<BigInt, _>(0i64);
        let sql = diesel::debug_query::<Sqlite, _>(&query).to_string();
        std::hint::black_box(sql);
        BenchOutcome::default()
    }));

    // Query-construction: recursive CTE.
    // Diesel has no high-level CTE query builder, so this falls back to `sql_query`.
    results.push(bench_sync("to_sql_select_with_recursive_cte", 100_000, || {
        let query = diesel::sql_query(
            "WITH RECURSIVE nums(id, email, age, name, created_at) AS (\
                SELECT id, email, age, name, created_at FROM users WHERE id = ? \
                UNION ALL \
                SELECT id, email, age, name, created_at FROM nums WHERE id = ?\
            ) SELECT * FROM nums WHERE id > ?",
        )
        .bind::<BigInt, _>(1i64)
        .bind::<BigInt, _>(2i64)
        .bind::<BigInt, _>(0i64);
        let sql = diesel::debug_query::<Sqlite, _>(&query).to_string();
        std::hint::black_box(sql);
        BenchOutcome::default()
    }));

    // Query-construction: set union.
    results.push(bench_sync("to_sql_set_union", 100_000, || {
        let query = users::table
            .filter(users::age.gt(18i64))
            .union_all(users::table.filter(users::age.le(18i64)));
        let sql = diesel::debug_query::<Sqlite, _>(&query).to_string();
        std::hint::black_box(sql);
        BenchOutcome::default()
    }));

    // Query-construction: inner join.
    results.push(bench_sync("to_sql_select_with_join", 100_000, || {
        let query = posts::table
            .inner_join(users::table.on(posts::author_id.eq(users::id)))
            .select((posts::all_columns, users::all_columns));
        let sql = diesel::debug_query::<Sqlite, _>(&query).to_string();
        std::hint::black_box(sql);
        BenchOutcome::default()
    }));

    // Query-construction: EXISTS subquery.
    results.push(bench_sync("to_sql_select_exists_subquery", 100_000, || {
        let query = users::table.filter(exists(
            posts::table.filter(posts::author_id.eq(users::id)),
        ));
        let sql = diesel::debug_query::<Sqlite, _>(&query).to_string();
        std::hint::black_box(sql);
        BenchOutcome::default()
    }));

    // Query-construction: IN subquery.
    results.push(bench_sync("to_sql_select_in_subquery", 100_000, || {
        let sub = posts::table
            .select(posts::author_id)
            .filter(posts::author_id.gt(0i64));
        let query = users::table.filter(users::id.eq_any(sub));
        let sql = diesel::debug_query::<Sqlite, _>(&query).to_string();
        std::hint::black_box(sql);
        BenchOutcome::default()
    }));

    // Query-construction: nested insert (parent + child).
    // Diesel has no native nested-insert API, so this falls back to a single raw
    // CTE statement built with `sql_query` and `bind`.
    results.push(bench_sync("to_sql_nested_insert", 100_000, || {
        let query = diesel::sql_query(
            "WITH new_user AS (\
                INSERT INTO users (id, email, age, name, created_at) VALUES (?, ?, ?, ?, ?) \
                RETURNING id\
            ) INSERT INTO posts (id, author_id, category_id, title, published_at, views) \
              SELECT ?, new_user.id, ?, ?, ?, ? FROM new_user",
        )
        .bind::<BigInt, _>(9999i64)
        .bind::<Text, _>("nested@example.com")
        .bind::<BigInt, _>(30i64)
        .bind::<Text, _>("Nested")
        .bind::<BigInt, _>(0i64)
        .bind::<BigInt, _>(10_001i64)
        .bind::<BigInt, _>(1i64)
        .bind::<Text, _>("nested post")
        .bind::<BigInt, _>(0i64)
        .bind::<BigInt, _>(0i64);
        let sql = diesel::debug_query::<Sqlite, _>(&query).to_string();
        std::hint::black_box(sql);
        BenchOutcome::default()
    }));

    // Query-construction: nested update (parent + related rows).
    // Diesel has no native nested-update API, so this falls back to a single raw
    // CTE statement built with `sql_query` and `bind`.
    results.push(bench_sync("to_sql_nested_update", 100_000, || {
        let query = diesel::sql_query(
            "WITH updated_user AS (\
                UPDATE users SET name = ? WHERE id = ? RETURNING id\
            ) UPDATE posts SET author_id = (SELECT id FROM updated_user) WHERE id IN (?, ?)",
        )
        .bind::<Text, _>("updated")
        .bind::<BigInt, _>(1i64)
        .bind::<BigInt, _>(10_001i64)
        .bind::<BigInt, _>(10_002i64);
        let sql = diesel::debug_query::<Sqlite, _>(&query).to_string();
        std::hint::black_box(sql);
        BenchOutcome::default()
    }));

    // End-to-end: select by PK.
    {
        let conn = &mut conn;
        results.push(bench_sync("select_by_pk", 1_000, || {
            let user: User = users::table
                .filter(users::id.eq(500i64))
                .limit(1)
                .get_result(conn)
                .expect("failed to fetch user by pk");
            assert_eq!(user.id, 500);
            std::hint::black_box(user);
            BenchOutcome { rows: 1, queries: 1 }
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
            let rows_returned = rows.len();
            std::hint::black_box(rows);
            BenchOutcome {
                rows: rows_returned,
                queries: 1,
            }
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
            let rows_returned = rows.len();
            std::hint::black_box(rows);
            BenchOutcome {
                rows: rows_returned,
                queries: 1,
            }
        }));
    }

    // End-to-end: filtered + ordered + paginated.
    {
        let conn = &mut conn;
        results.push(bench_sync("find_filtered_paginated", 50, || {
            let rows: Vec<User> = users::table
                .filter(users::age.gt(18i64))
                .order_by(users::age.asc())
                .then_order_by(users::email.asc())
                .limit(20)
                .offset(500)
                .load(conn)
                .expect("failed to fetch paginated users");
            assert_eq!(rows.len(), 20);
            std::hint::black_box(rows);
            BenchOutcome { rows: 20, queries: 1 }
        }));
    }

    // End-to-end: IN list with 50 ids.
    {
        let conn = &mut conn;
        results.push(bench_sync("find_in_list", 100, || {
            let rows: Vec<User> = users::table
                .filter(users::id.eq_any(&ids_50))
                .order_by(users::id.asc())
                .limit(50)
                .load(conn)
                .expect("failed to fetch users by id list");
            assert_eq!(rows.len(), 50);
            std::hint::black_box(rows);
            BenchOutcome { rows: 50, queries: 1 }
        }));
    }

    // End-to-end: complex filter with multiple parameters.
    {
        let conn = &mut conn;
        results.push(bench_sync("find_complex_filter", 50, || {
            let rows: Vec<User> = users::table
                .filter(
                    users::age
                        .gt(18i64)
                        .and(users::email.like("%example.com%"))
                        .and(users::id.between(100i64, 900i64)),
                )
                .order_by(users::age.asc())
                .then_order_by(users::email.asc())
                .limit(100)
                .load(conn)
                .expect("failed to fetch complex filtered users");
            assert_eq!(rows.len(), 100);
            std::hint::black_box(rows);
            BenchOutcome {
                rows: 100,
                queries: 1,
            }
        }));
    }

    // End-to-end: count with filter.
    {
        let conn = &mut conn;
        results.push(bench_sync("count_filtered", 100, || {
            let count: i64 = users::table
                .filter(users::age.gt(18i64))
                .count()
                .get_result(conn)
                .expect("failed to count users");
            assert!(count >= 980, "expected ~1000 users, got {}", count);
            BenchOutcome {
                rows: count as usize,
                queries: 1,
            }
        }));
    }

    // End-to-end: exists with filter.
    {
        let conn = &mut conn;
        results.push(bench_sync("exists_filtered", 100, || {
            let exists: bool = diesel::select(exists(
                users::table.filter(users::age.gt(18i64)),
            ))
            .get_result(conn)
            .expect("failed to check existence");
            assert!(exists);
            BenchOutcome {
                rows: if exists { 1 } else { 0 },
                queries: 1,
            }
        }));
    }

    // End-to-end: include posts for all users (manual two-query include).
    {
        let conn = &mut conn;
        results.push(bench_sync("include_posts", 10, || {
            let users: Vec<User> = users::table
                .load(conn)
                .expect("failed to fetch users");
            let posts: Vec<Post> = posts::table
                .load(conn)
                .expect("failed to fetch posts");
            assert_eq!(users.len(), 1000);
            assert_eq!(posts.len(), 10_000);

            let mut posts_by_author: HashMap<i64, Vec<Post>> = HashMap::new();
            for post in posts {
                posts_by_author
                    .entry(post.author_id)
                    .or_default()
                    .push(post);
            }
            let rows_returned = users.len();
            std::hint::black_box((users, posts_by_author));
            BenchOutcome {
                rows: rows_returned,
                queries: 2,
            }
        }));
    }

    // End-to-end: include author for all posts (manual two-query include).
    {
        let conn = &mut conn;
        results.push(bench_sync("include_author", 10, || {
            let posts: Vec<Post> = posts::table
                .load(conn)
                .expect("failed to fetch posts");
            let authors: Vec<User> = users::table
                .load(conn)
                .expect("failed to fetch authors");
            assert_eq!(posts.len(), 10_000);

            let author_map: HashMap<i64, User> =
                authors.into_iter().map(|u| (u.id, u)).collect();
            let attached: Vec<Option<&User>> = posts
                .iter()
                .map(|p| author_map.get(&p.author_id))
                .collect();
            let rows_returned = posts.len();
            std::hint::black_box((posts, attached));
            BenchOutcome {
                rows: rows_returned,
                queries: 2,
            }
        }));
    }

    // End-to-end: include posts and their comments (manual three-query include).
    {
        let conn = &mut conn;
        results.push(bench_sync("include_posts_and_comments", 10, || {
            let users: Vec<User> = users::table
                .load(conn)
                .expect("failed to fetch users");
            let posts: Vec<Post> = posts::table
                .load(conn)
                .expect("failed to fetch posts");
            let comments: Vec<Comment> = comments::table
                .load(conn)
                .expect("failed to fetch comments");
            assert_eq!(users.len(), 1000);
            assert_eq!(posts.len(), 10_000);
            assert_eq!(comments.len(), 50_000);

            let mut posts_by_author: HashMap<i64, Vec<Post>> = HashMap::new();
            for post in posts {
                posts_by_author
                    .entry(post.author_id)
                    .or_default()
                    .push(post);
            }
            let mut comments_by_post: HashMap<i64, Vec<Comment>> = HashMap::new();
            for comment in comments {
                comments_by_post
                    .entry(comment.post_id)
                    .or_default()
                    .push(comment);
            }
            let rows_returned = users.len();
            std::hint::black_box((users, posts_by_author, comments_by_post));
            BenchOutcome {
                rows: rows_returned,
                queries: 3,
            }
        }));
    }

    // End-to-end: posts with tags (many-to-many through post_tags).
    {
        let conn = &mut conn;
        results.push(bench_sync("include_posts_with_tags", 10, || {
            let posts: Vec<Post> = posts::table
                .load(conn)
                .expect("failed to fetch posts");
            let post_tags: Vec<PostTag> = post_tags::table
                .load(conn)
                .expect("failed to fetch post_tags");
            let tags: Vec<Tag> = tags::table
                .load(conn)
                .expect("failed to fetch tags");
            assert_eq!(posts.len(), 10_000);
            assert_eq!(post_tags.len(), 30_000);
            assert_eq!(tags.len(), 100);

            let tag_map: HashMap<i64, Tag> = tags.into_iter().map(|t| (t.id, t)).collect();
            let mut tags_by_post: HashMap<i64, Vec<Tag>> = HashMap::new();
            for pt in post_tags {
                if let Some(tag) = tag_map.get(&pt.tag_id) {
                    tags_by_post.entry(pt.post_id).or_default().push(tag.clone());
                }
            }
            let rows_returned = posts.len();
            std::hint::black_box((posts, tags_by_post));
            BenchOutcome {
                rows: rows_returned,
                queries: 3,
            }
        }));
    }

    // End-to-end: find popular posts and attach their authors.
    {
        let conn = &mut conn;
        results.push(bench_sync("find_popular_posts", 50, || {
            let posts: Vec<Post> = posts::table
                .filter(posts::views.gt(1000i64))
                .order_by(posts::views.desc())
                .limit(100)
                .load(conn)
                .expect("failed to fetch popular posts");
            assert_eq!(posts.len(), 100);

            let author_ids: Vec<i64> = posts.iter().map(|p| p.author_id).collect();
            let authors: Vec<User> = users::table
                .filter(users::id.eq_any(author_ids))
                .load(conn)
                .expect("failed to fetch authors");
            let author_map: HashMap<i64, User> =
                authors.into_iter().map(|u| (u.id, u)).collect();
            let attached: Vec<Option<&User>> = posts
                .iter()
                .map(|p| author_map.get(&p.author_id))
                .collect();
            let rows_returned = posts.len();
            std::hint::black_box((posts, attached));
            BenchOutcome {
                rows: rows_returned,
                queries: 2,
            }
        }));
    }

    // End-to-end: prepared select by PK with bound parameter.
    {
        let conn = &mut conn;
        results.push(bench_sync("prepared_select_by_pk", 1_000, || {
            let user: User = users::table
                .filter(users::id.eq(500i64))
                .limit(1)
                .offset(0)
                .get_result(conn)
                .expect("failed to fetch prepared user by pk");
            assert_eq!(user.id, 500);
            std::hint::black_box(user);
            BenchOutcome { rows: 1, queries: 1 }
        }));
    }

    // End-to-end: unbuffered stream of all users.
    {
        let conn = &mut conn;
        results.push(bench_sync("stream_find_many_1000", 50, || {
            let mut rows = 0;
            let iter = users::table
                .load_iter::<User, _>(conn)
                .expect("failed to create user stream");
            for r in iter {
                let _ = r.expect("failed to read streamed user");
                rows += 1;
            }
            assert_eq!(rows, 1000);
            BenchOutcome { rows, queries: 1 }
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
            let inserted: Vec<BenchBulk> = diesel::insert_into(bench_bulk::table)
                .values(&bulk_rows)
                .get_results(conn)
                .expect("failed to bulk insert");
            assert_eq!(inserted.len(), 1_000);
            let rows_returned = inserted.len();
            std::hint::black_box(inserted);
            BenchOutcome {
                rows: rows_returned,
                queries: 2,
            }
        }));
    }

    println!("\n{}", serde_json::to_string_pretty(&results).unwrap());

    let output_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("diesel-results.json");
    std::fs::write(
        &output_path,
        serde_json::to_string_pretty(&results).unwrap(),
    )
    .expect("failed to write results");
    println!("Wrote {}", output_path.display());
}
