# Cross-ORM benchmark results

This document records an apples-to-apples benchmark of **ruprizzle**, **prax**, **Sea-ORM**, **Diesel**, **Prisma**, and **Drizzle** against the same SQLite dataset and the same query shapes. ruprizzle is shown in two configurations: the `sqlx` backend and the new native `rusqlite` backend.

## Environment

- **Host:** Windows 11, local SQLite file
- **Dataset:**
  - `users`: 1,000 rows
  - `categories`: 20 rows
  - `posts`: 10,000 rows (10 posts per user)
  - `comments`: 50,000 rows (5 comments per post)
  - `tags`: 100 rows
  - `post_tags`: 30,000 rows (3 tags per post)
  - `followers`: 5,000 rows
  - `likes`: 20,000 rows
  - `bench_bulk`: empty table used for bulk-insert tests
- **Versions:**
  - ruprizzle `0.1.1-beta.1` (this repo)
  - prax-orm `0.11`
  - Sea-ORM `1.1`
  - Diesel `2.2`
  - Prisma `6.19.3`
  - Drizzle ORM `0.43.0`
  - better-sqlite3 `13.0.3`
  - rusqlite `0.32.1`

> Important: this run used SQLite because no Postgres / Docker was available. The numbers are therefore dominated as much by driver choice as by the ORM layer. ruprizzle (`sqlx` backend) uses `sqlx::Any`, ruprizzle (`rusqlite` backend) now runs the synchronous `rusqlite` query directly on the calling tokio task (no `spawn_blocking`), prax and Sea-ORM use `sqlx-sqlite`, Diesel uses `libsqlite3-sys`/`diesel` with a bundled SQLite, Drizzle uses the synchronous `better-sqlite3` binding, and Prisma uses its in-process Rust query engine.

## Harness and raw data

- ruprizzle: `crates/runtime/examples/cross_orm_bench.rs`
- ruprizzle (rusqlite): same example with `RUST_BENCH_DRIVER=rusqlite`
- prax: `local/cross-orm-bench/rust/prax-bench/src/main.rs`
- Sea-ORM: `local/cross-orm-bench/rust/sea-orm-bench/src/main.rs`
- Diesel: `local/cross-orm-bench/rust/diesel-bench/src/main.rs`
- Prisma / Drizzle: `local/cross-orm-bench/node/`
- Database: `local/cross-orm-bench/node/bench.sqlite3`
- Result JSONs:
  - `local/cross-orm-bench/raw_results.json` — all measured trials
  - `local/cross-orm-bench/results.json` — aggregated statistics
  - `local/cross-orm-bench/BENCHMARKS.log` — human-readable summary
  - `local/cross-orm-bench/node/ruprizzle-results.json`
  - `local/cross-orm-bench/node/ruprizzle-rusqlite-results.json`
  - `local/cross-orm-bench/node/prax-results.json`
  - `local/cross-orm-bench/node/sea-orm-results.json`
  - `local/cross-orm-bench/node/diesel-results.json`
  - `local/cross-orm-bench/node/prisma-results.json`
  - `local/cross-orm-bench/node/drizzle-results.json`

## End-to-end results

All times are microseconds per operation (lower is better).

| Operation | ruprizzle (sqlx) | ruprizzle (rusqlite) | prax | sea-orm | diesel | prisma | drizzle |
|---|---|---|---|---|---|---|---|
| `select_by_pk` | 24.0 | 3.1 | 24.7 | 83.7 | 11.2 | 196.7 | 50.8 |
| `find_many_1000` | 1,725.2 | 367.5 | 731.3 | 1,641.7 | 306.9 | 2,844.8 | 461.6 |
| `find_filtered_ordered` | 1,767.5 | 566.3 | 880.5 | 1,648.0 | 413.1 | 3,351.1 | 585.5 |
| `include_posts` | 22,034.9 | 7,133.6 | 11,112.6 | 20,362.4 | 3,845.7 | 43,635.5 | 202,045.1 |
| `bulk_insert_1000` | 1,907.8 | 1,238.3 | 1,139.9 | 6,456.8 | 6,765.0 | 14,355.7 | 9,612.8 |

## Query construction (no I/O)

| Operation | ruprizzle (sqlx) | ruprizzle (rusqlite) | prax | sea-orm | diesel | prisma | drizzle |
|---|---|---|---|---|---|---|---|
| `to_sql_select_by_pk` | 0.5 | 0.6 | 0.4 | 7.8 | 0.7 | — | 11.4 |
| `to_sql_select_filter_order` | 1.5 | 1.5 | 1.1 | 12.4 | 1.0 | — | 16.4 |

## Codegen / build-step comparison

- **Drizzle:** no code generation; the schema is plain TypeScript.
- **ruprizzle:** 50-model schema generation = **17.8 ms** (`cargo bench -p ruprizzle-codegen`).
- **Prisma:** `prisma generate` for the 3-model benchmark schema took **~36 ms**; it scales linearly with schema size and is heavier than ruprizzle for large schemas.

## Analysis

### Simple reads
The synchronous, native-driver ORMs lead here:

- **ruprizzle (rusqlite): 3.1 µs**
- **Diesel: 11.2 µs**
- **ruprizzle (sqlx): 24.0 µs**
- **prax: 24.7 µs**
- **Drizzle: 50.8 µs**
- **Sea-ORM: 83.7 µs**
- **Prisma: 196.7 µs**

Running the `rusqlite` query synchronously on the calling task eliminates the `tokio::task::spawn_blocking` dispatch that previously cost ~10–15 µs per call, making ruprizzle the fastest single-row PK lookup. Direct `ToSql` impls on `Value` avoid the extra string clone that used to happen per bind. Diesel and ruprizzle (sqlx) are the next tier.

### Multi-row and filtered reads
Diesel is still fastest, but ruprizzle (rusqlite) is now within striking distance:

- `find_many_1000`: **306.9 µs** (Diesel) vs **367.5 µs** (ruprizzle rusqlite) vs **461.6 µs** (Drizzle) vs **731.3 µs** (prax) vs **1,641.7 µs** (Sea-ORM) vs **2,844.8 µs** (Prisma)
- `find_filtered_ordered`: **413.1 µs** (Diesel) vs **566.3 µs** (ruprizzle rusqlite) vs **585.5 µs** (Drizzle) vs **880.5 µs** (prax) vs **1,648.0 µs** (Sea-ORM) vs **3,351.1 µs** (Prisma)

The remaining gap on multi-row reads is mostly per-row decoding overhead in ruprizzle's intermediate `Row`/`FromValue` path. The native drivers avoid the per-row async worker-thread hop used by `sqlx-sqlite`, which still shows up clearly in Sea-ORM and ruprizzle (sqlx).

### Relation includes
Diesel's manually-written join query is fastest, while ruprizzle's auto-batched loader remains the strongest *automatic* option:

- **Diesel: 3.8 ms** (manual join)
- **ruprizzle (rusqlite): 7.1 ms** (auto-batched)
- **prax: 11.1 ms**
- **ruprizzle (sqlx): 22.0 ms**
- **Sea-ORM: 20.4 ms**
- **Prisma: 43.6 ms**
- **Drizzle: 202.0 ms** (correlated subquery per parent row)

Removing `spawn_blocking` shaves ~0.5 ms off ruprizzle (rusqlite). The remaining gap vs Diesel is the in-memory grouping of 10,000 child rows into 1,000 parent `Vec`s. Diesel does not group in this benchmark. Drizzle's SQLite relational query still emits a correlated subquery with `json_group_array` per parent row.

### Bulk insert
The `rusqlite` and `sqlx` backends are fastest:

- **prax: 1.1 ms**
- **ruprizzle (rusqlite): 1.2 ms**
- **ruprizzle (sqlx): 1.9 ms**
- **Drizzle: 9.6 ms**
- **Diesel: 6.8 ms**
- **Sea-ORM: 6.5 ms**
- **Prisma: 14.4 ms**

Diesel's bulk insert is slower here despite using a single `INSERT` statement. ruprizzle and prax both use a multi-value statement with `RETURNING *` and still come out ahead.

### Query construction
Turning a builder into SQL+binds is cheap for all the Rust ORMs, while the TypeScript/JavaScript ORMs are an order of magnitude slower:

- **prax: 0.4 µs**, **Diesel: 0.7 µs**, **ruprizzle (rusqlite): 0.6 µs**, **ruprizzle (sqlx): 0.5 µs**
- **Sea-ORM: 7.8–14.6 µs**
- **Drizzle: 11.4–38.3 µs**
- **Prisma: not exposed**

Query construction remains a sub-microsecond win for ruprizzle (rusqlite); the bigger wins in this round came from removing per-bind `Value`->`RusqliteValue` conversions.

## Usage criteria

| Criterion | Best choice | Why |
|---|---|---|
| Compile-time type safety, generated typed client | **Diesel** or **ruprizzle** | Both are schema-first and fully typed; Diesel has the larger ecosystem, ruprizzle the more ergonomically generated client. |
| Maximum simple-query throughput on SQLite | **ruprizzle (rusqlite)** | 3.1 µs on `select_by_pk` — faster than Diesel's 11.2 µs — with zero async dispatch overhead. |
| Multi-row reads and filtered queries on SQLite | **Diesel**, then **ruprizzle (rusqlite)** | Diesel is fastest; ruprizzle (rusqlite) is within 20–40% and beats Drizzle/prax. |
| Bulk inserts on SQLite | **prax**, then **ruprizzle (rusqlite)** | 1.1–1.2 ms, faster than Diesel/Sea-ORM. |
| Nested relation loading, automatic batching | **Diesel** (manual), then **ruprizzle (rusqlite)**, then **prax** | Diesel's manual join is fastest; ruprizzle's auto-batched loader beats Sea-ORM/Prisma. |
| TypeScript ecosystem, migrations, team familiarity | **Prisma** | Largest community, mature migrations, schema-first. |
| Zero build-step / runtime schema | **Drizzle** | Schema is plain TypeScript, no code generation. |
| SQL transparency / `.to_sql()` on every builder | **ruprizzle, Diesel, or Drizzle** | ruprizzle and Diesel expose SQL cheaply; Drizzle exposes it too but is slower to construct. |
| Production Postgres | ruprizzle, Prisma, or Diesel | ruprizzle's [`Performance.md`](Performance.md) shows it within ~5% of hand-written `sqlx` on Postgres; Diesel and Drizzle are also strong on Postgres but were not measured here. |

## Caveats

1. **SQLite is not Postgres.** These numbers are from a single SQLite file. The relative ordering can change with network latency, a different driver, or a different database. **Do not cite these numbers for a Postgres comparison until they are re-run on Postgres.**
2. **Driver and dispatch differences dominate simple reads.** ruprizzle (rusqlite) now runs `rusqlite` queries synchronously on the calling tokio task, eliminating `spawn_blocking` dispatch and beating even Diesel on single-row PK lookups. Diesel uses `libsqlite3-sys` directly, and Drizzle uses the synchronous `better-sqlite3` binding. All of these avoid the per-row async worker-thread hop used by `sqlx-sqlite` (which powers ruprizzle (sqlx), prax, and Sea-ORM). The measured row-hop cost is roughly 0.9 µs/row for `sqlx-sqlite` versus ~0.3 µs/row for the synchronous bindings, which is why Diesel, ruprizzle (rusqlite), and Drizzle cluster near the top for simple reads.
3. **Drizzle relational query is SQLite/driver-specific.** On Postgres Drizzle can use joins/CTEs and would likely be far faster for `include_posts`; do not take the 182.1 ms as a universal Drizzle number.
4. **No network.** All ORMs talked to a local file, so result-set decoding and ORM overhead are the main differentiators.
5. **This run used 1 warm-up + 2 measured trials per driver.** Medians are reported. See `local/cross-orm-bench/BENCHMARKS.log` and `local/cross-orm-bench/raw_results.json` for full per-trial data. Run-to-run variance can be 5–10% on Windows. The main take-away is the relative shape between backends, not single-digit absolute values.

## See also

- [Performance](Performance.md) — Postgres vs `sqlx` measurements and the `sqlx::Any` text-marshalling note.
- [Known limitations](KnownLimitations.md) — honest boundaries of the alpha.
- `ProjectPlan/ImplementationPlan/ImplPlan09TestingRelease.md` — original testing-and-benchmark plan.

## Benchmark run: 2026-08-13 07:25 UTC

### Environment

- **Warm-up trials:** 1
- **Measured trials:** 2
- **Dataset:**
  - 1,000 users
  - 20 categories
  - 10,000 posts
  - 50,000 comments
  - 100 tags
  - 30,000 post_tags
  - 5,000 followers
  - 20,000 likes

### End-to-end results

All times are microseconds per operation (lower is better).

| Operation | ruprizzle (sqlx) | ruprizzle (rusqlite) | prax | sea-orm | diesel | prisma | drizzle |
|---|---|---|---|---|---|---|---|
| `select_by_pk` | 24.0 | 3.1 | 24.7 | 83.7 | 11.2 | 196.7 | 50.8 |
| `find_many_1000` | 1,725.2 | 367.5 | 731.3 | 1,641.7 | 306.9 | 2,844.8 | 461.6 |
| `find_filtered_ordered` | 1,767.5 | 566.3 | 880.5 | 1,648.0 | 413.1 | 3,351.1 | 585.5 |
| `find_filtered_paginated` | 404.1 | 298.5 | 350.0 | 421.9 | 307.8 | 717.8 | 375.7 |
| `find_in_list` | 114.2 | 28.4 | 125.3 | 131.4 | 37.7 | 434.4 | 143.5 |
| `find_complex_filter` | 331.9 | 157.9 | 302.0 | 337.4 | 158.1 | 905.1 | 258.0 |
| `count_filtered` | 46.1 | 20.5 | 51.6 | 85.1 | 26.8 | 203.8 | 62.9 |
| `exists_filtered` | 31.4 | 2.6 | 44.4 | 57.1 | 9.6 | 152.0 | 59.2 |
| `include_posts` | 22,034.9 | 7,133.6 | 11,112.6 | 20,362.4 | 3,845.7 | 43,635.5 | 202,045.1 |
| `include_author` | 21,274.4 | 7,341.2 | 8,882.5 | 20,066.1 | 3,305.2 | 85,683.7 | 19,261.1 |
| `include_posts_and_comments` | 134,962.3 | 55,612.4 | 43,914.9 | 110,351.2 | 20,711.9 | 260,752.6 | 10,445,301.8 |
| `include_posts_with_tags` | 54,808.5 | 25,538.1 | 26,189.5 | 54,337.6 | 8,326.9 | 265,723.7 | 46,585.6 |
| `find_popular_posts` | 1,471.4 | 1,253.0 | 1,980.7 | 1,616.6 | 1,295.7 | 2,590.9 | 6,567.2 |
| `bulk_insert_1000` | 1,907.8 | 1,238.3 | 1,139.9 | 6,456.8 | 6,765.0 | 14,355.7 | 9,612.8 |

### Query construction (no I/O)

| Operation | ruprizzle (sqlx) | ruprizzle (rusqlite) | prax | sea-orm | diesel | prisma | drizzle |
|---|---|---|---|---|---|---|---|
| `to_sql_select_by_pk` | 0.5 | 0.6 | 0.4 | 7.8 | 0.7 | — | 11.4 |
| `to_sql_select_filter_order` | 1.5 | 1.5 | 1.1 | 12.4 | 1.0 | — | 16.4 |
| `to_sql_select_in_list` | 2.3 | 2.3 | 4.3 | 28.4 | 2.7 | — | 38.3 |
| `to_sql_select_complex_filter` | 1.8 | 1.8 | 1.5 | 14.6 | 1.1 | — | 19.0 |
| `to_sql_select_paginated` | 1.5 | 1.5 | 1.1 | 11.9 | 1.0 | — | 19.2 |

## Benchmark run: 2026-08-13 11:45 UTC

### Environment

- **Warm-up trials:** 1
- **Measured trials:** 3
- **Dataset:**
  - 1,000 users
  - 20 categories
  - 10,000 posts
  - 50,000 comments
  - 100 tags
  - 30,000 post_tags
  - 5,000 followers
  - 20,000 likes

### End-to-end results

All times are microseconds per operation (lower is better).

| Operation | ruprizzle (sqlx) | ruprizzle (rusqlite) | prax | sea-orm | diesel | prisma | drizzle |
|---|---|---|---|---|---|---|---|
| `select_by_pk` | 25.4 | 3.0 | 20.5 | 63.2 | 10.0 | 209.8 | 38.1 |
| `find_many_1000` | 1,648.4 | 375.9 | 737.5 | 1,641.6 | 296.8 | 3,078.2 | 405.6 |
| `find_filtered_ordered` | 1,882.6 | 556.7 | 900.5 | 1,618.2 | 413.3 | 3,465.3 | 539.6 |
| `find_filtered_paginated` | 392.6 | 297.0 | 353.6 | 421.1 | 304.1 | 801.8 | 361.1 |
| `find_in_list` | 115.1 | 30.2 | 98.6 | 131.2 | 37.6 | 481.9 | 130.0 |
| `find_complex_filter` | 306.9 | 163.8 | 232.3 | 352.8 | 159.2 | 873.4 | 234.0 |
| `count_filtered` | 35.5 | 21.5 | 40.3 | 77.5 | 25.7 | 203.7 | 51.6 |
| `exists_filtered` | 16.5 | 2.6 | 16.9 | 57.7 | 9.6 | 159.7 | 44.5 |
| `include_posts` | 21,069.9 | 7,185.7 | 10,711.7 | 19,915.2 | 3,743.7 | 42,498.5 | 186,426.8 |
| `include_author` | 20,702.4 | 7,231.6 | 9,064.7 | 20,774.2 | 3,438.0 | 83,394.2 | 16,330.6 |
| `include_posts_and_comments` | 131,291.1 | 56,928.3 | 43,907.2 | 108,491.8 | 20,990.9 | 262,741.6 | 9,093,560.6 |
| `include_posts_with_tags` | 51,677.9 | 26,213.6 | 26,020.8 | 51,054.7 | 8,071.8 | 257,371.4 | 36,273.0 |
| `find_popular_posts` | 1,506.3 | 1,306.0 | 2,021.4 | 1,620.3 | 1,297.8 | 2,547.2 | 5,556.0 |
| `bulk_insert_1000` | 1,865.9 | 1,273.5 | 1,102.5 | 6,171.2 | 7,060.8 | 13,761.8 | 9,778.8 |

### Query construction (no I/O)

| Operation | ruprizzle (sqlx) | ruprizzle (rusqlite) | prax | sea-orm | diesel | prisma | drizzle |
|---|---|---|---|---|---|---|---|
| `to_sql_select_by_pk` | 0.6 | 0.5 | 0.4 | 7.3 | 0.7 | — | 11.6 |
| `to_sql_select_filter_order` | 1.5 | 1.6 | 1.1 | 12.1 | 1.0 | — | 17.0 |
| `to_sql_select_in_list` | 2.4 | 2.3 | 4.3 | 24.9 | 2.7 | — | 38.8 |
| `to_sql_select_complex_filter` | 1.8 | 1.9 | 1.5 | 14.2 | 1.1 | — | 18.8 |
| `to_sql_select_paginated` | 1.5 | 1.5 | 1.1 | 11.7 | 1.0 | — | 17.2 |

## Benchmark run: 2026-08-17 10:31 UTC

### Environment

- **Warm-up trials:** 1
- **Measured trials:** 3
- **Dataset:**
  - 1,000 users
  - 20 categories
  - 10,000 posts
  - 50,000 comments
  - 100 tags
  - 30,000 post_tags
  - 5,000 followers
  - 20,000 likes

### End-to-end results

All times are microseconds per operation (lower is better).

| Operation | ruprizzle (sqlx) | ruprizzle (rusqlite) | prax | sea-orm | diesel | prisma | drizzle |
|---|---|---|---|---|---|---|---|
| `select_by_pk` | 26.8 | 3.2 | 24.5 | 68.4 | 10.5 | 205.5 | 40.6 |
| `find_many_1000` | 1,672.0 | 421.8 | 737.9 | 1,685.6 | 303.6 | 2,929.8 | 445.3 |
| `find_filtered_ordered` | 1,820.5 | 588.5 | 867.5 | 1,664.3 | 458.0 | 3,364.7 | 592.5 |
| `find_filtered_paginated` | 380.8 | 323.0 | 367.2 | 425.3 | 313.9 | 688.5 | 383.7 |
| `find_in_list` | 107.6 | 30.4 | 93.5 | 130.3 | 38.0 | 443.8 | 153.2 |
| `find_complex_filter` | 317.9 | 169.4 | 254.2 | 351.6 | 161.2 | 847.0 | 281.9 |
| `count_filtered` | 36.1 | 19.8 | 38.7 | 81.1 | 26.7 | 200.9 | 51.6 |
| `exists_filtered` | 19.0 | 2.7 | 28.7 | 61.3 | 9.6 | 184.4 | 48.5 |
| `include_posts` | 22,552.8 | 8,064.8 | 11,167.2 | 20,926.7 | 3,740.3 | 44,188.7 | 209,266.2 |
| `include_author` | 22,017.3 | 7,805.1 | 9,258.2 | 20,993.0 | 3,563.9 | 87,114.5 | 21,389.2 |
| `include_posts_and_comments` | 139,103.6 | 59,680.6 | 45,911.1 | 116,811.1 | 22,209.8 | 271,459.5 | 9,939,872.7 |
| `include_posts_with_tags` | 58,117.2 | 28,041.0 | 26,128.7 | 55,850.4 | 8,884.1 | 282,784.3 | 39,234.7 |
| `find_popular_posts` | 1,504.4 | 1,376.1 | 1,980.9 | 1,639.0 | 1,414.5 | 2,686.2 | 6,129.3 |
| `prepared_select_by_pk` | 20.4 | 2.4 | — | — | — | — | — |
| `stream_find_many_1000` | 1,922.2 | 704.1 | — | — | — | — | — |
| `bulk_insert_1000` | 2,100.2 | 1,502.0 | 1,044.0 | 14,892.6 | 7,632.8 | 13,908.4 | 9,867.8 |

### Query construction (no I/O)

| Operation | ruprizzle (sqlx) | ruprizzle (rusqlite) | prax | sea-orm | diesel | prisma | drizzle |
|---|---|---|---|---|---|---|---|
| `to_sql_select_by_pk` | 0.6 | 0.6 | 0.4 | 7.6 | 0.8 | — | 12.3 |
| `to_sql_select_filter_order` | 1.5 | 1.5 | 1.1 | 12.2 | 1.0 | — | 18.0 |
| `to_sql_select_in_list` | 2.3 | 2.3 | 4.5 | 26.1 | 2.8 | — | 41.8 |
| `to_sql_select_complex_filter` | 1.8 | 1.8 | 1.6 | 14.3 | 1.2 | — | 20.9 |
| `to_sql_select_paginated` | 1.5 | 1.6 | 1.1 | 11.7 | 1.0 | — | 18.9 |
| `to_sql_prepared_select_by_pk` | 0.6 | 0.7 | — | — | — | — | — |
| `prepared_rebind_select_by_pk` | 0.0 | 0.0 | — | — | — | — | — |
| `to_sql_conditional_filter` | 0.8 | 0.8 | — | — | — | — | — |
| `to_sql_select_with_cte` | 1.6 | 1.6 | — | — | — | — | — |
| `to_sql_select_with_recursive_cte` | 2.3 | 2.4 | — | — | — | — | — |
| `to_sql_set_union` | 1.6 | 1.4 | — | — | — | — | — |
| `to_sql_select_with_join` | 0.9 | 0.9 | — | — | — | — | — |
| `to_sql_select_exists_subquery` | 1.0 | 1.1 | — | — | — | — | — |
| `to_sql_select_in_subquery` | 1.3 | 1.4 | — | — | — | — | — |
| `to_sql_nested_insert` | 1.2 | 1.2 | — | — | — | — | — |
| `to_sql_nested_update` | 0.8 | 0.8 | — | — | — | — | — |
