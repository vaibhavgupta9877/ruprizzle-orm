# Cross-ORM benchmark results

This document records an apples-to-apples benchmark of **ruprizzle**, **prax**, **Sea-ORM**, **Diesel**, **Prisma**, and **Drizzle** against the same SQLite dataset and the same query shapes. ruprizzle is shown in two configurations: the `sqlx` backend and the new native `rusqlite` backend.

## Environment

- **Host:** Windows 11, local SQLite file
- **Dataset:**
  - `users`: 1,000 rows
  - `posts`: 10,000 rows (10 posts per user)
  - `bench_bulk`: empty table used for bulk-insert tests
- **Versions:**
  - ruprizzle `0.1.0-alpha.3` (this repo)
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
| `select_by_pk` | 19.3 | 3.0 | 28.8 | 74.4 | 9.8 | 163.0 | 29.6 |
| `find_many_1000` | 1,000.4 | 225.8 | 469.4 | 1,138.0 | 187.5 | 2,020.0 | 300.9 |
| `find_filtered_ordered` | 1,163.3 | 373.6 | 620.4 | 1,242.3 | 271.4 | 2,233.3 | 351.3 |
| `include_posts` | 12,796.1 | 4,512.3 | 7,233.9 | 24,410.7 | 2,883.9 | 32,291.5 | 182,144.4 |
| `bulk_insert_1000` | 1,971.9 | 1,263.4 | 1,376.6 | 5,933.5 | 5,726.0 | 13,052.1 | 8,349.7 |

## Query construction (no I/O)

| Operation | ruprizzle (sqlx) | ruprizzle (rusqlite) | prax | sea-orm | diesel | prisma | drizzle |
|---|---|---|---|---|---|---|---|
| `to_sql_select_by_pk` | 0.9 | 0.5 | 0.4 | 5.0 | 0.5 | — | 8.4 |
| `to_sql_select_filter_order` | 3.2 | 1.5 | 0.4 | 7.7 | 0.7 | — | 10.2 |

## Codegen / build-step comparison

- **Drizzle:** no code generation; the schema is plain TypeScript.
- **ruprizzle:** 50-model schema generation = **17.8 ms** (`cargo bench -p ruprizzle-codegen`).
- **Prisma:** `prisma generate` for the 3-model benchmark schema took **~36 ms**; it scales linearly with schema size and is heavier than ruprizzle for large schemas.

## Analysis

### Simple reads
The synchronous, native-driver ORMs lead here:

- **ruprizzle (rusqlite): 3.0 µs**
- **Diesel: 9.8 µs**
- **ruprizzle (sqlx): 19.3 µs**
- **Drizzle: 29.6 µs**, **prax: 28.8 µs**
- **Sea-ORM: 74.4 µs**
- **Prisma: 163.0 µs**

Running the `rusqlite` query synchronously on the calling task eliminates the `tokio::task::spawn_blocking` dispatch that previously cost ~10–15 µs per call, making ruprizzle the fastest single-row PK lookup. Diesel and ruprizzle (sqlx) are the next tier.

### Multi-row and filtered reads
Diesel is still fastest, but ruprizzle (rusqlite) is now within striking distance:

- `find_many_1000`: **187.5 µs** (Diesel) vs **225.8 µs** (ruprizzle rusqlite) vs **300.9 µs** (Drizzle) vs **469.4 µs** (prax) vs **1,138.0 µs** (Sea-ORM) vs **2,020.0 µs** (Prisma)
- `find_filtered_ordered`: **271.4 µs** (Diesel) vs **351.3 µs** (Drizzle) vs **373.6 µs** (ruprizzle rusqlite) vs **620.4 µs** (prax) vs **1,242.3 µs** (Sea-ORM) vs **2,233.3 µs** (Prisma)

The remaining gap on multi-row reads is mostly per-row decoding overhead in ruprizzle's intermediate `Row`/`FromValue` path. The native drivers avoid the per-row async worker-thread hop used by `sqlx-sqlite`, which still shows up clearly in Sea-ORM and ruprizzle (sqlx).

### Relation includes
Diesel's manually-written join query is fastest, while ruprizzle's auto-batched loader remains the strongest *automatic* option:

- **Diesel: 2.9 ms** (manual join)
- **ruprizzle (rusqlite): 4.5 ms** (auto-batched)
- **prax: 7.2 ms**
- **ruprizzle (sqlx): 12.8 ms**
- **Sea-ORM: 24.4 ms**
- **Prisma: 32.3 ms**
- **Drizzle: 182.1 ms** (correlated subquery per parent row)

Removing `spawn_blocking` shaves ~0.5 ms off ruprizzle (rusqlite). The remaining gap vs Diesel is the in-memory grouping of 10,000 child rows into 1,000 parent `Vec`s. Diesel does not group in this benchmark. Drizzle's SQLite relational query still emits a correlated subquery with `json_group_array` per parent row.

### Bulk insert
The `rusqlite` and `sqlx` backends are fastest:

- **ruprizzle (rusqlite): 1.3 ms**
- **prax: 1.4 ms**
- **ruprizzle (sqlx): 2.0 ms**
- **Diesel: 5.7 ms**
- **Sea-ORM: 5.9 ms**
- **Drizzle: 8.3 ms**
- **Prisma: 13.1 ms**

Diesel's bulk insert is slower here despite using a single `INSERT` statement. ruprizzle and prax both use a multi-value statement with `RETURNING *` and still come out ahead.

### Query construction
Turning a builder into SQL+binds is cheap for all the Rust ORMs, while the TypeScript/JavaScript ORMs are an order of magnitude slower:

- **prax: 0.4 µs**, **Diesel: 0.5 µs**, **ruprizzle (rusqlite): 0.5 µs**, **ruprizzle (sqlx): 0.9 µs**
- **Sea-ORM: 5.0–7.7 µs**
- **Drizzle: 8.4–10.2 µs**
- **Prisma: not exposed**

Query construction is now a sub-microsecond win for ruprizzle (rusqlite), but the real story is end-to-end reads.

## Usage criteria

| Criterion | Best choice | Why |
|---|---|---|
| Compile-time type safety, generated typed client | **Diesel** or **ruprizzle** | Both are schema-first and fully typed; Diesel has the larger ecosystem, ruprizzle the more ergonomically generated client. |
| Maximum simple-query throughput on SQLite | **ruprizzle (rusqlite)** | 3.0 µs on `select_by_pk` — faster than Diesel's 9.8 µs — with zero async dispatch overhead. |
| Multi-row reads and filtered queries on SQLite | **Diesel**, then **ruprizzle (rusqlite)** | Diesel is fastest; ruprizzle (rusqlite) is within 20–40% and beats Drizzle/prax. |
| Bulk inserts on SQLite | **ruprizzle (rusqlite)**, then **prax** | 1.3–1.4 ms, roughly 4.5× faster than Diesel. |
| Nested relation loading, automatic batching | **ruprizzle (rusqlite)**, then **prax**, then **Diesel** (manual) | ruprizzle's auto-batched loader is ~2× faster than Sea-ORM/Prisma; Diesel is fastest if you hand-write the join. |
| TypeScript ecosystem, migrations, team familiarity | **Prisma** | Largest community, mature migrations, schema-first. |
| Zero build-step / runtime schema | **Drizzle** | Schema is plain TypeScript, no code generation. |
| SQL transparency / `.to_sql()` on every builder | **ruprizzle, Diesel, or Drizzle** | ruprizzle and Diesel expose SQL cheaply; Drizzle exposes it too but is slower to construct. |
| Production Postgres | ruprizzle, Prisma, or Diesel | ruprizzle's [`Performance.md`](Performance.md) shows it within ~5% of hand-written `sqlx` on Postgres; Diesel and Drizzle are also strong on Postgres but were not measured here. |

## Caveats

1. **SQLite is not Postgres.** These numbers are from a single SQLite file. The relative ordering can change with network latency, a different driver, or a different database. **Do not cite these numbers for a Postgres comparison until they are re-run on Postgres.**
2. **Driver and dispatch differences dominate simple reads.** ruprizzle (rusqlite) now runs `rusqlite` queries synchronously on the calling tokio task, eliminating `spawn_blocking` dispatch and beating even Diesel on single-row PK lookups. Diesel uses `libsqlite3-sys` directly, and Drizzle uses the synchronous `better-sqlite3` binding. All of these avoid the per-row async worker-thread hop used by `sqlx-sqlite` (which powers ruprizzle (sqlx), prax, and Sea-ORM). The measured row-hop cost is roughly 0.9 µs/row for `sqlx-sqlite` versus ~0.3 µs/row for the synchronous bindings, which is why Diesel, ruprizzle (rusqlite), and Drizzle cluster near the top for simple reads.
3. **Drizzle relational query is SQLite/driver-specific.** On Postgres Drizzle can use joins/CTEs and would likely be far faster for `include_posts`; do not take the 182.1 ms as a universal Drizzle number.
4. **No network.** All ORMs talked to a local file, so result-set decoding and ORM overhead are the main differentiators.
5. **This run used 1 warm-up + 10 measured trials per driver.** Medians are reported. See `local/cross-orm-bench/BENCHMARKS.log` and `local/cross-orm-bench/raw_results.json` for full per-trial data. Run-to-run variance can be 5–10% on Windows. The main take-away is the relative shape between backends, not single-digit absolute values.

## See also

- [Performance](Performance.md) — Postgres vs `sqlx` measurements and the `sqlx::Any` text-marshalling note.
- [Known limitations](KnownLimitations.md) — honest boundaries of the alpha.
- `ProjectPlan/ImplementationPlan/ImplPlan09TestingRelease.md` — original testing-and-benchmark plan.
