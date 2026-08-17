# Cross-ORM benchmark expansion plan (implementation)

This is the source-of-truth spec for all harnesses. It is more detailed than `docs/superpowers/specs/2026-08-12-cross-orm-bench-expansion-design.md`.

## Database

`local/cross-orm-bench/node/bench.sqlite3` (created by `npm run seed` from `node/seed.js`).

Table schema in `node/schema.js` / `prisma/schema.prisma` / `seed.js`:

- `users(id, email, age, name, created_at)` – 1,000 rows
- `categories(id, name)` – 20 rows
- `posts(id, author_id, category_id, title, published_at, views)` – 10,000 rows
- `comments(id, post_id, author_id, content, created_at)` – 50,000 rows
- `tags(id, name)` – 100 rows
- `post_tags(post_id, tag_id)` – 30,000 rows
- `followers(follower_id, followee_id, created_at)` – 5,000 rows
- `likes(id, user_id, post_id, created_at)` – 20,000 rows
- `bench_bulk(id, name, n)` – empty at start

All harnesses read from the same file. The `BENCH_SQLITE_PATH` env var is set to an absolute path by `run_bench.py`.

## Result JSON

Each harness writes `*results.json` in its own output directory (`node/` for Node, `rust/<harness>/` for Rust). The file contains a JSON array of `BenchResult` objects:

```json
{
  "orm": "ruprizzle",
  "operation": "include_posts",
  "iters": 10,
  "total_ms": 123.4,
  "avg_ms": 12.34,
  "qps": 81.0,
  "rows_returned": 1000,
  "queries_issued": 2,
  "peak_rss_mb": 42.5,
  "cpu_time_ms": 45.2
}
```

- `iters`: number of measured iterations (warm-ups not counted).
- `total_ms`: wall time for all `iters`.
- `avg_ms`: `total_ms / iters`.
- `qps`: `1000 / avg_ms` (queries per second / operations per second).
- `rows_returned`: number of top-level rows returned by the operation.
- `queries_issued`: number of SQL round-trips for the operation. For ruprizzle this is exact via `CountingExecutor`; other harnesses report best-guess constants.
- `peak_rss_mb`: peak resident memory in MB.
- `cpu_time_ms`: process user+system CPU time consumed during the measured loop, in milliseconds.

For query-construction operations (`to_sql_*`) `rows_returned` and `queries_issued` are `0`. For `to_sql_*` operations in Prisma, harnesses may omit the operation entirely because Prisma does not expose SQL text.

## Operation list

### Query construction

All use 100,000 iterations unless noted.

- `to_sql_select_by_pk` – `SELECT * FROM users WHERE id = 500 LIMIT 1` (or equivalent builder).
- `to_sql_select_filter_order` – `SELECT * FROM users WHERE age > 18 AND email LIKE '%@example.com%' ORDER BY age, email LIMIT 1000 OFFSET 0`.
- `to_sql_select_in_list` – `SELECT * FROM users WHERE id IN (1..50) ORDER BY id LIMIT 50`.
- `to_sql_select_complex_filter` – `SELECT * FROM users WHERE age > 18 AND email LIKE '%example.com%' AND id BETWEEN 100 AND 900 ORDER BY age, email LIMIT 100`.
- `to_sql_select_paginated` – `SELECT * FROM users WHERE age > 18 AND email LIKE '%example.com%' ORDER BY age, email LIMIT 20 OFFSET 500`.

### End-to-end reads

- `select_by_pk`, 1000 iters, 1 row, 1 query.
- `find_many_1000`, 50 iters, 1000 rows, 1 query.
- `find_filtered_ordered`, 50 iters, ~980 rows (age > 18), 1 query.
- `find_filtered_paginated`, 50 iters, 20 rows, 1 query.
- `find_in_list`, 100 iters, 50 rows, 1 query.
- `find_complex_filter`, 50 iters, 100 rows, 1 query.
- `count_filtered`, 100 iters, ~980, 1 query.
- `exists_filtered`, 100 iters, 1, 1 query.
- `include_posts`, 10 iters, 1000 users, 2 queries (users + posts).
- `include_author`, 10 iters, 10000 posts, 2 queries (posts + authors).
- `include_posts_and_comments`, 10 iters, 1000 users, 3 queries (users + posts + comments).
- `include_posts_with_tags`, 10 iters, 10000 posts, 3 queries (posts + post_tags + tags).
- `find_popular_posts`, 50 iters, 100 posts, filter `views > 1000` + `include author` + `ORDER BY views DESC LIMIT 100`; 2 queries.

### End-to-end writes

- `bulk_insert_1000`, 10 iters, 1000 rows, 2 queries (`DELETE FROM bench_bulk` + `INSERT 1000 rows RETURNING *`).

## Notes per harness

### ruprizzle (done)

Reference implementation in `crates/runtime/examples/cross_orm_bench.rs`.

### Node

- `bench-drizzle.js` uses `drizzle-orm` with `better-sqlite3`.
- `bench-prisma.js` uses the Prisma Client.
- Both should measure CPU with `process.cpuUsage()` and peak RSS with `process.memoryUsage().rss` (sample before/after, take larger).
- Query/row counts can be set as constants matching the operation semantics.
- Output files: `drizzle-results.json`, `prisma-results.json`.
- For Prisma, skip `to_sql_*` operations.

### Prax

- `local/cross-orm-bench/rust/prax-bench/src/main.rs`.
- Add `simple-process-stats` to `Cargo.toml`.
- Use `simple_process_stats::ProcessStats` for resource metrics.
- Update models to the 8 tables and implement as many operations as possible.
- Output file: `prax-results.json` in `rust/prax-bench/`.

### Sea-ORM

- `local/cross-orm-bench/rust/sea-orm-bench/` with entity files under `src/entities/`.
- Add `simple-process-stats` to `Cargo.toml`.
- Create entity files for new tables; relations for includes.
- `src/main.rs` runs the operations.
- Output file: `sea-orm-results.json` in `rust/sea-orm-bench/`.

### Diesel

- `local/cross-orm-bench/rust/diesel-bench/src/main.rs` with table macro schema.
- Add `simple-process-stats`.
- Update schema and models, implement includes via manual joins where Diesel does not have a convenient relational loader.
- Output file: `diesel-results.json` in `rust/diesel-bench/`.

## Runner

`local/cross-orm-bench/run_bench.py`:

- Build ruprizzle with default and `rusqlite` drivers.
- Build `prax-bench`, `sea-orm-bench`, `diesel-bench`.
- Run `npm run seed` and, for Prisma, re-run `prisma generate`.
- Run each harness, collect all `*results.json`.
- Compute per-operation statistics: `mean`, `median`, `min`, `max`, `stdev`, `cv` (`stdev/mean`), `p95` for `avg_ms`.
- Write `BENCHMARKS.log` with end-to-end, query-construction, and query/row/resource tables.
- Append a new `## Benchmark run: <timestamp>` section to `docs/BenchmarkResults.md` while keeping previous runs.
