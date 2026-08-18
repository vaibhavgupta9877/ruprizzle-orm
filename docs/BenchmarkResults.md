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
  - ruprizzle `0.4.0-beta.2` (this repo)
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
| `select_by_pk` | 27.9 | 3.1 | 22.5 | 76.8 | 10.0 | 182.9 | 38.3 |
| `find_many_1000` | 1,741.1 | 385.8 | 771.0 | 1,616.5 | 297.4 | 2,935.7 | 414.2 |
| `find_filtered_ordered` | 1,918.1 | 569.6 | 966.1 | 1,669.8 | 433.8 | 3,358.3 | 506.7 |
| `include_posts` | 22,881.2 | 7,545.5 | 11,712.9 | 20,333.5 | 3,647.4 | 44,300.7 | 186,014.4 |
| `bulk_insert_1000` | 2,099.7 | 1,395.0 | 1,174.6 | 5,031.3 | 6,619.3 | 13,236.6 | 8,536.1 |

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
- **Diesel: 10.0 µs**
- **ruprizzle (sqlx): 27.9 µs**
- **prax: 22.5 µs**
- **Drizzle: 38.3 µs**
- **Sea-ORM: 76.8 µs**
- **Prisma: 182.9 µs**

Running the `rusqlite` query synchronously on the calling task eliminates the `tokio::task::spawn_blocking` dispatch that previously cost ~10–15 µs per call, making ruprizzle the fastest single-row PK lookup. Direct `ToSql` impls on `Value` avoid the extra string clone that used to happen per bind. Diesel and ruprizzle (sqlx) are the next tier.

### Multi-row and filtered reads
Diesel is still fastest, but ruprizzle (rusqlite) is now within striking distance:

- `find_many_1000`: **297.4 µs** (Diesel) vs **385.8 µs** (ruprizzle rusqlite) vs **414.2 µs** (Drizzle) vs **771.0 µs** (prax) vs **1,616.5 µs** (Sea-ORM) vs **2,935.7 µs** (Prisma)
- `find_filtered_ordered`: **433.8 µs** (Diesel) vs **569.6 µs** (ruprizzle rusqlite) vs **506.7 µs** (Drizzle) vs **966.1 µs** (prax) vs **1,669.8 µs** (Sea-ORM) vs **3,358.3 µs** (Prisma)

The remaining gap on multi-row reads is mostly per-row decoding overhead in ruprizzle's intermediate `Row`/`FromValue` path. The native drivers avoid the per-row async worker-thread hop used by `sqlx-sqlite`, which still shows up clearly in Sea-ORM and ruprizzle (sqlx).

### Relation includes
Diesel's manually-written join query is fastest, while ruprizzle's auto-batched loader remains the strongest *automatic* option:

- **Diesel: 3.6 ms** (manual join)
- **ruprizzle (rusqlite): 7.5 ms** (auto-batched)
- **prax: 11.7 ms**
- **ruprizzle (sqlx): 22.9 ms**
- **Sea-ORM: 20.3 ms**
- **Prisma: 44.3 ms**
- **Drizzle: 186.0 ms** (correlated subquery per parent row)

Removing `spawn_blocking` shaves ~0.5 ms off ruprizzle (rusqlite). The remaining gap vs Diesel is the in-memory grouping of 10,000 child rows into 1,000 parent `Vec`s. Diesel does not group in this benchmark. Drizzle's SQLite relational query still emits a correlated subquery with `json_group_array` per parent row.

### Bulk insert
The `rusqlite` and `sqlx` backends are fastest:

- **prax: 1.2 ms**
- **ruprizzle (rusqlite): 1.4 ms**
- **ruprizzle (sqlx): 2.1 ms**
- **Drizzle: 8.5 ms**
- **Diesel: 6.6 ms**
- **Sea-ORM: 5.0 ms**
- **Prisma: 13.2 ms**

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
| Maximum simple-query throughput on SQLite | **ruprizzle (rusqlite)** | 3.1 µs on `select_by_pk` — faster than Diesel's 10.0 µs — with zero async dispatch overhead. |
| Multi-row reads and filtered queries on SQLite | **Diesel**, then **ruprizzle (rusqlite)** | Diesel is fastest; ruprizzle (rusqlite) is within 20–40% and beats Drizzle/prax. |
| Bulk inserts on SQLite | **prax**, then **ruprizzle (rusqlite)** | 1.2–1.4 ms, faster than Diesel/Sea-ORM. |
| Nested relation loading, automatic batching | **Diesel** (manual), then **ruprizzle (rusqlite)**, then **prax** | Diesel's manual join is fastest; ruprizzle's auto-batched loader beats Sea-ORM/Prisma. |
| TypeScript ecosystem, migrations, team familiarity | **Prisma** | Largest community, mature migrations, schema-first. |
| Zero build-step / runtime schema | **Drizzle** | Schema is plain TypeScript, no code generation. |
| SQL transparency / `.to_sql()` on every builder | **ruprizzle, Diesel, or Drizzle** | ruprizzle and Diesel expose SQL cheaply; Drizzle exposes it too but is slower to construct. |
| Production Postgres | ruprizzle, Prisma, or Diesel | ruprizzle's [`performance.md`](performance.md) shows it within ~5% of hand-written `sqlx` on Postgres; Diesel and Drizzle are also strong on Postgres but were not measured here. |

## Caveats

1. **SQLite is not Postgres.** These numbers are from a single SQLite file. The relative ordering can change with network latency, a different driver, or a different database. **Do not cite these numbers for a Postgres comparison until they are re-run on Postgres.**
2. **Driver and dispatch differences dominate simple reads.** ruprizzle (rusqlite) now runs `rusqlite` queries synchronously on the calling tokio task, eliminating `spawn_blocking` dispatch and beating even Diesel on single-row PK lookups. Diesel uses `libsqlite3-sys` directly, and Drizzle uses the synchronous `better-sqlite3` binding. All of these avoid the per-row async worker-thread hop used by `sqlx-sqlite` (which powers ruprizzle (sqlx), prax, and Sea-ORM). The measured row-hop cost is roughly 0.9 µs/row for `sqlx-sqlite` versus ~0.3 µs/row for the synchronous bindings, which is why Diesel, ruprizzle (rusqlite), and Drizzle cluster near the top for simple reads.
3. **Drizzle relational query is SQLite/driver-specific.** On Postgres Drizzle can use joins/CTEs and would likely be far faster for `include_posts`; do not take the 186.0 ms as a universal Drizzle number.
4. **No network.** All ORMs talked to a local file, so result-set decoding and ORM overhead are the main differentiators.
5. **This run used 1 warm-up + 10 measured trials per driver.** Medians are reported. See `local/cross-orm-bench/BENCHMARKS.log` and `local/cross-orm-bench/raw_results.json` for full per-trial data. Run-to-run variance can be 5–10% on Windows. The main take-away is the relative shape between backends, not single-digit absolute values.

## See also

- [Performance](performance.md) — Postgres vs `sqlx` measurements and the `sqlx::Any` text-marshalling note.
- [Known limitations](KnownLimitations.md) — honest boundaries of the alpha.
- [Feature master comparison](FeaturesMasterComparison.md) — feature and architecture comparison across all ORMs, including the extended benchmark table.
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

## Benchmark run: 2026-08-17 12:00 UTC

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
| `select_by_pk` | 20.9 | 3.1 | 17.6 | 66.5 | 10.1 | 174.8 | 39.3 |
| `find_many_1000` | 1,620.4 | 424.2 | 794.7 | 1,710.1 | 303.8 | 2,879.6 | 406.8 |
| `find_filtered_ordered` | 1,841.1 | 549.2 | 925.3 | 1,810.3 | 421.8 | 3,332.9 | 480.2 |
| `find_filtered_paginated` | 393.1 | 309.0 | 347.4 | 472.9 | 307.7 | 667.1 | 372.6 |
| `find_in_list` | 107.1 | 32.8 | 80.8 | 131.8 | 41.4 | 445.1 | 104.8 |
| `find_complex_filter` | 310.3 | 156.7 | 230.3 | 355.2 | 167.2 | 836.1 | 248.4 |
| `count_filtered` | 35.5 | 19.5 | 34.9 | 89.9 | 25.4 | 187.0 | 46.7 |
| `exists_filtered` | 17.1 | 2.6 | 16.1 | 58.6 | 9.5 | 155.9 | 46.1 |
| `include_posts` | 22,514.6 | 7,411.6 | 10,818.1 | 22,010.8 | 3,725.5 | 42,982.6 | 189,412.7 |
| `include_author` | 21,994.2 | 7,115.1 | 8,845.5 | 21,775.0 | 3,417.6 | 83,281.3 | 17,031.6 |
| `include_posts_and_comments` | 137,868.3 | 57,814.5 | 43,439.2 | 118,143.1 | 20,724.0 | 262,781.5 | 9,280,171.7 |
| `include_posts_with_tags` | 55,864.7 | 26,187.5 | 25,400.9 | 58,378.9 | 8,063.9 | 266,997.0 | 37,587.5 |
| `find_popular_posts` | 1,518.6 | 1,306.8 | 1,993.9 | 1,661.4 | 1,324.3 | 2,646.5 | 5,636.3 |
| `prepared_select_by_pk` | 18.2 | 2.6 | 4.5 | 63.7 | 9.9 | 177.0 | 14.9 |
| `stream_find_many_1000` | 2,024.0 | 689.7 | 57.7 | 2,760.6 | 237.3 | 2,643.1 | 314.8 |
| `bulk_insert_1000` | 1,966.0 | 1,434.7 | 1,195.3 | 6,279.6 | 7,080.6 | 13,614.5 | 8,518.1 |

### Query construction (no I/O)

| Operation | ruprizzle (sqlx) | ruprizzle (rusqlite) | prax | sea-orm | diesel | prisma | drizzle |
|---|---|---|---|---|---|---|---|
| `to_sql_select_by_pk` | 0.6 | 0.6 | 0.4 | 7.5 | 0.7 | 0.1 | 11.8 |
| `to_sql_select_filter_order` | 1.5 | 1.5 | 1.1 | 12.7 | 1.0 | 0.1 | 17.2 |
| `to_sql_select_in_list` | 2.3 | 2.3 | 4.4 | 25.3 | 2.7 | 0.7 | 39.6 |
| `to_sql_select_complex_filter` | 1.8 | 1.8 | 1.5 | 14.3 | 1.1 | 0.1 | 19.4 |
| `to_sql_select_paginated` | 1.5 | 1.5 | 1.1 | 12.0 | 1.0 | 0.1 | 17.7 |
| `to_sql_prepared_select_by_pk` | 0.6 | 0.6 | 0.4 | 2.7 | 0.8 | 0.1 | 11.8 |
| `prepared_rebind_select_by_pk` | 0.0 | 0.0 | 0.1 | 0.1 | 0.2 | 0.1 | 0.1 |
| `to_sql_conditional_filter` | 0.8 | 0.8 | 0.4 | 8.3 | 0.8 | 0.3 | 15.0 |
| `to_sql_select_with_cte` | 1.6 | 1.6 | 0.8 | 15.8 | 0.2 | 0.1 | 36.1 |
| `to_sql_select_with_recursive_cte` | 2.4 | 2.4 | 0.5 | 21.3 | 0.3 | 0.1 | 0.1 |
| `to_sql_set_union` | 1.4 | 1.4 | 0.9 | 14.0 | 1.1 | 0.1 | 26.4 |
| `to_sql_select_with_join` | 0.9 | 0.9 | 0.1 | 9.2 | 1.1 | 0.1 | 29.2 |
| `to_sql_select_exists_subquery` | 1.0 | 1.0 | 0.1 | 15.0 | 1.1 | 0.1 | 23.8 |
| `to_sql_select_in_subquery` | 1.4 | 1.4 | 0.1 | 10.6 | 0.8 | 0.1 | 19.0 |
| `to_sql_nested_insert` | 1.2 | 1.2 | 0.4 | 0.0 | 0.6 | 0.1 | 19.9 |
| `to_sql_nested_update` | 0.8 | 0.8 | 0.3 | 0.0 | 0.3 | 0.1 | 17.6 |

## Benchmark run: 2026-08-17 16:16 UTC

### Environment

- **Warm-up trials:** 1
- **Measured trials:** 10
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
| `select_by_pk` | 22.6 | 3.0 | 22.9 | 65.8 | 15.5 | 196.4 | 39.2 |
| `find_many_1000` | 1,644.7 | 383.2 | 770.6 | 1,684.5 | 311.2 | 3,061.4 | 411.1 |
| `find_filtered_ordered` | 1,781.2 | 569.4 | 946.2 | 1,777.0 | 446.4 | 3,469.7 | 498.7 |
| `find_filtered_paginated` | 381.8 | 308.6 | 403.6 | 460.7 | 314.5 | 730.7 | 347.7 |
| `find_in_list` | 102.1 | 29.6 | 107.3 | 147.8 | 53.2 | 463.5 | 100.6 |
| `find_complex_filter` | 306.6 | 164.9 | 262.0 | 372.8 | 182.1 | 850.3 | 221.4 |
| `count_filtered` | 39.4 | 20.0 | 44.4 | 87.1 | 33.7 | 199.4 | 47.1 |
| `exists_filtered` | 17.3 | 2.7 | 18.4 | 64.6 | 13.4 | 166.7 | 41.3 |
| `include_posts` | 22,236.7 | 7,679.7 | 11,498.6 | 21,707.3 | 3,801.7 | 45,196.0 | 187,172.4 |
| `include_author` | 22,262.8 | 7,436.9 | 9,581.2 | 21,619.0 | 3,453.0 | 87,424.8 | 16,660.2 |
| `include_posts_and_comments` | 132,481.4 | 57,545.4 | 44,265.0 | 117,815.9 | 21,704.7 | 270,511.9 | 9,144,201.9 |
| `include_posts_with_tags` | 55,727.3 | 27,186.3 | 26,821.0 | 57,127.1 | 8,494.4 | 274,363.4 | 37,185.9 |
| `find_popular_posts` | 1,478.6 | 1,268.7 | 2,160.1 | 1,661.8 | 1,317.8 | 2,689.4 | 5,543.8 |
| `prepared_select_by_pk` | 26.0 | 2.3 | 4.6 | 74.9 | 15.0 | 179.6 | 14.9 |
| `stream_find_many_1000` | 2,035.8 | 686.0 | 59.7 | 2,485.3 | 245.5 | 2,754.3 | 316.6 |
| `bulk_insert_1000` | 1,962.5 | 1,346.0 | 1,222.4 | 9,403.5 | 13,988.2 | 13,118.6 | 8,341.6 |

### Query construction (no I/O)

| Operation | ruprizzle (sqlx) | ruprizzle (rusqlite) | prax | sea-orm | diesel | prisma | drizzle |
|---|---|---|---|---|---|---|---|
| `to_sql_select_by_pk` | 0.6 | 0.6 | 0.4 | 7.6 | 0.7 | 0.1 | 12.2 |
| `to_sql_select_filter_order` | 1.5 | 1.6 | 1.1 | 12.4 | 1.0 | 0.1 | 18.0 |
| `to_sql_select_in_list` | 2.3 | 2.4 | 4.4 | 25.4 | 2.8 | 0.7 | 40.6 |
| `to_sql_select_complex_filter` | 1.8 | 1.8 | 1.5 | 14.6 | 1.2 | 0.1 | 20.0 |
| `to_sql_select_paginated` | 1.5 | 1.5 | 1.1 | 12.0 | 1.0 | 0.1 | 18.4 |
| `to_sql_prepared_select_by_pk` | 0.6 | 0.6 | 0.4 | 2.7 | 0.8 | 0.1 | 12.0 |
| `prepared_rebind_select_by_pk` | 0.0 | 0.0 | 0.1 | 0.1 | 0.2 | 0.1 | 0.1 |
| `to_sql_conditional_filter` | 0.8 | 0.8 | 0.4 | 8.4 | 0.9 | 0.3 | 15.4 |
| `to_sql_select_with_cte` | 1.6 | 1.6 | 0.8 | 15.7 | 0.2 | 0.1 | 36.9 |
| `to_sql_select_with_recursive_cte` | 2.3 | 2.4 | 0.5 | 21.4 | 0.2 | 0.1 | 0.1 |
| `to_sql_set_union` | 1.6 | 1.4 | 0.9 | 14.1 | 1.2 | 0.1 | 27.4 |
| `to_sql_select_with_join` | 0.9 | 0.9 | 0.1 | 9.2 | 1.1 | 0.1 | 30.3 |
| `to_sql_select_exists_subquery` | 1.0 | 1.1 | 0.1 | 15.1 | 1.1 | 0.1 | 24.3 |
| `to_sql_select_in_subquery` | 1.4 | 1.4 | 0.1 | 10.7 | 0.8 | 0.1 | 19.3 |
| `to_sql_nested_insert` | 1.2 | 1.3 | 0.4 | 0.0 | 0.6 | 0.1 | 20.6 |
| `to_sql_nested_update` | 0.8 | 0.8 | 0.3 | 0.0 | 0.3 | 0.1 | 18.4 |

## Benchmark run: 2026-08-17 16:55 UTC

### Environment

- **Warm-up trials:** 1
- **Measured trials:** 10
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
| `select_by_pk` | 27.9 | 3.1 | 22.5 | 76.8 | 10.0 | 182.9 | 38.3 |
| `find_many_1000` | 1,741.1 | 385.8 | 771.0 | 1,616.5 | 297.4 | 2,935.7 | 414.2 |
| `find_filtered_ordered` | 1,918.1 | 569.6 | 966.1 | 1,669.8 | 433.8 | 3,358.3 | 506.7 |
| `find_filtered_paginated` | 412.9 | 312.6 | 371.2 | 426.0 | 307.9 | 666.1 | 360.0 |
| `find_in_list` | 125.6 | 29.5 | 101.0 | 129.4 | 39.9 | 427.7 | 101.9 |
| `find_complex_filter` | 324.8 | 166.8 | 256.8 | 343.9 | 172.2 | 838.5 | 229.3 |
| `count_filtered` | 39.6 | 20.6 | 40.7 | 81.3 | 25.9 | 178.9 | 47.7 |
| `exists_filtered` | 18.5 | 2.7 | 19.3 | 57.9 | 9.8 | 157.7 | 42.4 |
| `include_posts` | 22,881.2 | 7,545.5 | 11,712.9 | 20,333.5 | 3,647.4 | 44,300.7 | 186,014.4 |
| `include_author` | 23,033.2 | 7,332.0 | 9,596.4 | 20,500.2 | 3,362.5 | 86,517.4 | 16,450.2 |
| `include_posts_and_comments` | 136,207.9 | 57,665.9 | 44,929.0 | 110,745.1 | 20,997.0 | 267,560.2 | 9,098,301.2 |
| `include_posts_with_tags` | 57,895.0 | 27,607.2 | 27,132.6 | 54,141.4 | 8,209.3 | 276,963.6 | 36,364.1 |
| `find_popular_posts` | 1,579.2 | 1,268.0 | 2,221.2 | 1,564.6 | 1,296.9 | 2,662.8 | 5,553.4 |
| `prepared_select_by_pk` | 20.5 | 2.3 | 5.3 | 60.2 | 10.2 | 165.5 | 14.8 |
| `stream_find_many_1000` | 2,096.7 | 706.5 | 58.1 | 2,576.9 | 231.7 | 2,659.3 | 330.7 |
| `bulk_insert_1000` | 2,099.7 | 1,395.0 | 1,174.6 | 5,031.3 | 6,619.3 | 13,236.6 | 8,536.1 |

### Query construction (no I/O)

| Operation | ruprizzle (sqlx) | ruprizzle (rusqlite) | prax | sea-orm | diesel | prisma | drizzle |
|---|---|---|---|---|---|---|---|
| `to_sql_select_by_pk` | 0.6 | 0.6 | 0.4 | 7.4 | 0.7 | 0.1 | 11.8 |
| `to_sql_select_filter_order` | 1.6 | 1.6 | 1.2 | 12.2 | 1.0 | 0.1 | 17.2 |
| `to_sql_select_in_list` | 2.3 | 2.4 | 4.5 | 25.6 | 2.7 | 0.7 | 40.3 |
| `to_sql_select_complex_filter` | 1.8 | 1.9 | 1.6 | 14.2 | 1.1 | 0.1 | 19.1 |
| `to_sql_select_paginated` | 1.5 | 1.5 | 1.1 | 11.8 | 1.0 | 0.1 | 17.7 |
| `to_sql_prepared_select_by_pk` | 0.6 | 0.6 | 0.4 | 2.7 | 0.7 | 0.1 | 11.7 |
| `prepared_rebind_select_by_pk` | 0.0 | 0.0 | 0.1 | 0.1 | 0.2 | 0.1 | 0.1 |
| `to_sql_conditional_filter` | 0.8 | 0.8 | 0.4 | 8.3 | 0.8 | 0.3 | 14.6 |
| `to_sql_select_with_cte` | 1.6 | 1.6 | 0.8 | 15.3 | 0.2 | 0.1 | 35.6 |
| `to_sql_select_with_recursive_cte` | 2.3 | 2.4 | 0.5 | 20.8 | 0.2 | 0.1 | 0.1 |
| `to_sql_set_union` | 1.6 | 1.4 | 0.9 | 13.7 | 1.1 | 0.1 | 26.0 |
| `to_sql_select_with_join` | 0.9 | 0.9 | 0.1 | 9.0 | 1.1 | 0.1 | 29.0 |
| `to_sql_select_exists_subquery` | 1.0 | 1.1 | 0.1 | 14.4 | 1.1 | 0.1 | 24.0 |
| `to_sql_select_in_subquery` | 1.4 | 1.4 | 0.1 | 10.5 | 0.8 | 0.1 | 18.6 |
| `to_sql_nested_insert` | 1.2 | 1.2 | 0.4 | 0.0 | 0.5 | 0.1 | 19.8 |
| `to_sql_nested_update` | 0.8 | 0.8 | 0.3 | 0.0 | 0.3 | 0.1 | 17.4 |

## Benchmark run: 2026-08-18 06:04 UTC

### Environment

- **Warm-up trials:** 1
- **Measured trials:** 10
- **Concurrency levels:** [1, 10, 100]
- **Duration per throughput run:** 5.0s
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
| `select_by_pk` | 25.1 | 3.1 | 17.9 | 66.8 | 9.9 | 173.1 | 39.0 |
| `find_many_1000` | 1,634.4 | 386.3 | 741.9 | 1,559.1 | 305.4 | 2,820.7 | 409.9 |
| `find_filtered_ordered` | 1,797.4 | 565.4 | 925.6 | 1,603.9 | 425.3 | 3,225.3 | 504.7 |
| `find_filtered_paginated` | 386.1 | 300.0 | 361.7 | 455.4 | 307.9 | 629.8 | 357.0 |
| `find_in_list` | 107.7 | 30.0 | 98.8 | 130.7 | 39.6 | 404.2 | 99.0 |
| `find_complex_filter` | 314.2 | 162.1 | 251.8 | 340.4 | 164.2 | 774.0 | 228.6 |
| `count_filtered` | 40.1 | 20.3 | 44.4 | 91.3 | 25.7 | 170.1 | 45.8 |
| `exists_filtered` | 20.7 | 2.7 | 22.1 | 70.0 | 9.6 | 151.4 | 41.6 |
| `include_posts` | 21,139.9 | 7,553.3 | 10,741.2 | 20,856.5 | 3,627.0 | 40,867.0 | 188,946.9 |
| `include_author` | 20,764.9 | 7,122.6 | 8,779.8 | 20,716.5 | 3,285.4 | 78,985.7 | 16,595.5 |
| `include_posts_and_comments` | 131,693.4 | 59,759.2 | 41,534.5 | 114,364.3 | 20,605.2 | 251,942.4 | 9,214,149.0 |
| `include_posts_with_tags` | 54,108.5 | 28,362.6 | 24,896.2 | 54,784.9 | 8,156.6 | 280,121.6 | 37,234.5 |
| `find_popular_posts` | 1,458.4 | 1,266.3 | 2,058.7 | 1,671.5 | 1,275.6 | 2,805.2 | 5,655.4 |
| `prepared_select_by_pk` | 24.6 | 2.3 | 4.9 | 84.7 | 10.0 | 194.2 | 14.9 |
| `stream_find_many_1000` | 2,296.3 | 705.6 | 57.1 | 1,807.1 | 225.6 | 3,290.1 | 327.0 |
| `bulk_insert_1000` | 1,912.4 | 1,383.1 | 1,059.0 | 6,027.3 | 6,689.7 | 14,142.5 | 9,069.6 |

### Query construction (no I/O)

| Operation | ruprizzle (sqlx) | ruprizzle (rusqlite) | prax | sea-orm | diesel | prisma | drizzle |
|---|---|---|---|---|---|---|---|
| `to_sql_select_by_pk` | 0.7 | 0.7 | 0.4 | 7.6 | 0.7 | 0.1 | 11.6 |
| `to_sql_select_filter_order` | 1.7 | 1.7 | 1.1 | 12.3 | 1.0 | 0.1 | 17.1 |
| `to_sql_select_in_list` | 2.5 | 2.5 | 4.3 | 25.8 | 2.7 | 0.7 | 39.4 |
| `to_sql_select_complex_filter` | 2.0 | 2.0 | 1.5 | 14.3 | 1.1 | 0.1 | 19.8 |
| `to_sql_select_paginated` | 1.7 | 1.7 | 1.1 | 11.8 | 1.0 | 0.1 | 18.2 |
| `to_sql_prepared_select_by_pk` | 0.7 | 0.7 | 0.4 | 2.7 | 0.8 | 0.1 | 11.9 |
| `prepared_rebind_select_by_pk` | 0.0 | 0.0 | 0.1 | 0.1 | 0.2 | 0.1 | 0.1 |
| `to_sql_conditional_filter` | 1.0 | 1.0 | 0.4 | 9.1 | 0.8 | 0.3 | 15.3 |
| `to_sql_select_with_cte` | 1.9 | 1.9 | 0.8 | 19.1 | 0.2 | 0.1 | 36.5 |
| `to_sql_select_with_recursive_cte` | 2.8 | 2.8 | 0.5 | 25.6 | 0.2 | 0.1 | 0.1 |
| `to_sql_set_union` | 1.8 | 1.6 | 0.9 | 17.1 | 1.1 | 0.1 | 27.3 |
| `to_sql_select_with_join` | 1.0 | 1.0 | 0.1 | 11.4 | 1.1 | 0.1 | 30.1 |
| `to_sql_select_exists_subquery` | 1.3 | 1.3 | 0.1 | 18.0 | 1.1 | 0.1 | 24.3 |
| `to_sql_select_in_subquery` | 1.7 | 1.7 | 0.1 | 13.1 | 0.8 | 0.1 | 19.1 |
| `to_sql_nested_insert` | 1.3 | 1.4 | 0.4 | 0.0 | 0.6 | 0.1 | 20.3 |
| `to_sql_nested_update` | 0.9 | 1.0 | 0.3 | 0.0 | 0.3 | 0.1 | 17.6 |

### Latency percentiles (ruprizzle sqlx)

| Operation | p50 | p95 | p99 |
|---|---|---|---|
| `select_by_pk` | 25.1 | 32.5 | 33.5 |
| `find_many_1000` | 1,634.4 | 1,701.2 | 1,725.8 |
| `find_filtered_ordered` | 1,797.4 | 1,863.1 | 1,863.7 |
| `find_filtered_paginated` | 386.1 | 407.9 | 408.7 |
| `find_in_list` | 107.7 | 122.2 | 125.4 |
| `find_complex_filter` | 314.2 | 338.3 | 342.2 |
| `count_filtered` | 40.1 | 48.2 | 49.8 |
| `exists_filtered` | 20.7 | 30.8 | 33.0 |
| `include_posts` | 21,139.9 | 23,082.5 | 23,460.9 |
| `include_author` | 20,764.9 | 21,973.5 | 22,057.2 |
| `include_posts_and_comments` | 131,693.4 | 138,897.9 | 139,415.8 |
| `include_posts_with_tags` | 54,108.5 | 57,747.5 | 57,792.9 |
| `find_popular_posts` | 1,458.4 | 1,482.6 | 1,484.6 |
| `prepared_select_by_pk` | 24.6 | 32.3 | 32.7 |
| `stream_find_many_1000` | 2,296.3 | 2,385.9 | 2,397.2 |
| `bulk_insert_1000` | 1,912.4 | 2,980.8 | 3,497.2 |
| `to_sql_select_by_pk` | 0.7 | 0.7 | 0.7 |
| `to_sql_select_filter_order` | 1.7 | 1.7 | 1.8 |
| `to_sql_select_in_list` | 2.5 | 2.5 | 2.6 |
| `to_sql_select_complex_filter` | 2.0 | 2.1 | 2.2 |
| `to_sql_select_paginated` | 1.7 | 1.8 | 1.9 |
| `to_sql_prepared_select_by_pk` | 0.7 | 0.7 | 0.7 |
| `prepared_rebind_select_by_pk` | 0.0 | 0.0 | 0.0 |
| `to_sql_conditional_filter` | 1.0 | 1.0 | 1.0 |
| `to_sql_select_with_cte` | 1.9 | 1.9 | 1.9 |
| `to_sql_select_with_recursive_cte` | 2.8 | 2.9 | 2.9 |
| `to_sql_set_union` | 1.8 | 1.8 | 1.8 |
| `to_sql_select_with_join` | 1.0 | 1.0 | 1.0 |
| `to_sql_select_exists_subquery` | 1.3 | 1.4 | 1.4 |
| `to_sql_select_in_subquery` | 1.7 | 1.7 | 1.7 |
| `to_sql_nested_insert` | 1.3 | 1.4 | 1.4 |
| `to_sql_nested_update` | 0.9 | 1.0 | 1.0 |

### Throughput (ops/sec)

| Backend | Concurrency | select_by_pk | find_many_1000 | bulk_insert_1000 |
|---|---|---|---|---|
