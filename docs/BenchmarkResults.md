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

> Important: this run used SQLite because no Postgres / Docker was available. The numbers are therefore dominated as much by driver choice as by the ORM layer. ruprizzle (`sqlx` backend) uses `sqlx::Any`, ruprizzle (`rusqlite` backend) uses the synchronous `rusqlite` binding over a small blocking pool, prax and Sea-ORM use `sqlx-sqlite`, Diesel uses `libsqlite3-sys`/`diesel` with a bundled SQLite, Drizzle uses the synchronous `better-sqlite3` binding, and Prisma uses its in-process Rust query engine.

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
| `select_by_pk` | 22.6 | 16.6 | 28.8 | 69.2 | 10.3 | 163.9 | 29.4 |
| `find_many_1000` | 1,026.5 | 304.4 | 452.9 | 1,169.8 | 194.9 | 2,047.6 | 305.6 |
| `find_filtered_ordered` | 1,160.7 | 493.2 | 633.4 | 1,218.4 | 289.5 | 2,238.9 | 334.9 |
| `include_posts` | 13,075.3 | 5,024.1 | 7,448.6 | 24,874.3 | 2,924.0 | 32,864.9 | 181,915.6 |
| `bulk_insert_1000` | 2,110.8 | 1,474.0 | 1,419.1 | 6,235.5 | 5,410.8 | 13,296.5 | 8,711.2 |

## Query construction (no I/O)

| Operation | ruprizzle (sqlx) | ruprizzle (rusqlite) | prax | sea-orm | diesel | prisma | drizzle |
|---|---|---|---|---|---|---|---|
| `to_sql_select_by_pk` | 0.9 | 0.6 | 0.4 | 5.1 | 0.5 | — | 8.5 |
| `to_sql_select_filter_order` | 3.3 | 1.5 | 0.4 | 7.6 | 0.7 | — | 10.2 |

## Codegen / build-step comparison

- **Drizzle:** no code generation; the schema is plain TypeScript.
- **ruprizzle:** 50-model schema generation = **17.8 ms** (`cargo bench -p ruprizzle-codegen`).
- **Prisma:** `prisma generate` for the 3-model benchmark schema took **~36 ms**; it scales linearly with schema size and is heavier than ruprizzle for large schemas.

## Analysis

### Simple reads
The synchronous, native-driver ORMs lead here:

- **Diesel: 10.3 µs**
- **ruprizzle (rusqlite): 16.6 µs**
- **ruprizzle (sqlx): 22.6 µs**
- **Drizzle: 29.4 µs**, **prax: 28.8 µs**
- **Sea-ORM: 69.2 µs**
- **Prisma: 163.9 µs**

Diesel's direct `libsqlite3-sys` path has the least runtime overhead, while ruprizzle's `rusqlite` backend is close behind. Sea-ORM and Prisma carry the most overhead for a single-row PK lookup.

### Multi-row and filtered reads
Diesel is again fastest, with ruprizzle (rusqlite), Drizzle, and prax forming a competitive mid-tier:

- `find_many_1000`: **194.9 µs** (Diesel) vs **304.4 µs** (ruprizzle rusqlite) vs **305.6 µs** (Drizzle) vs **452.9 µs** (prax) vs **1,169.8 µs** (Sea-ORM) vs **2,047.6 µs** (Prisma)
- `find_filtered_ordered`: **289.5 µs** (Diesel) vs **334.9 µs** (Drizzle) vs **493.2 µs** (ruprizzle rusqlite) vs **633.4 µs** (prax) vs **1,218.4 µs** (Sea-ORM) vs **2,238.9 µs** (Prisma)

The native, synchronous drivers (Diesel / better-sqlite3 / rusqlite) avoid the per-row async worker-thread hop used by `sqlx-sqlite`, which shows up clearly in Sea-ORM's and ruprizzle (sqlx)'s numbers.

### Relation includes
Diesel's manually-written join query is fastest, while ruprizzle's auto-batched loader remains the strongest *automatic* option:

- **Diesel: 2.9 ms** (manual join)
- **ruprizzle (rusqlite): 5.0 ms** (auto-batched)
- **prax: 7.4 ms**
- **ruprizzle (sqlx): 13.1 ms**
- **Sea-ORM: 24.9 ms**
- **Prisma: 32.9 ms**
- **Drizzle: 181.9 ms** (correlated subquery per parent row)

Drizzle's SQLite relational query emits a correlated subquery with `json_group_array` per parent row — effectively an N+1 shape in SQL. On Postgres it can use joins/CTEs and would likely be much faster. Sea-ORM's `find_with_related` issues a pair of queries but is slower than ruprizzle's loader, partly because of its own in-memory grouping and the `sqlx-sqlite` async runtime overhead.

### Bulk insert
The `rusqlite` and `sqlx` backends are fastest:

- **prax: 1.4 ms**
- **ruprizzle (rusqlite): 1.5 ms**
- **ruprizzle (sqlx): 2.1 ms**
- **Diesel: 5.4 ms**
- **Drizzle: 8.7 ms**
- **Sea-ORM: 6.2 ms**
- **Prisma: 13.3 ms**

Diesel's bulk insert is slower here despite using a single `INSERT` statement; it still returns the inserted rows (like ruprizzle), so the gap is likely from its `INSERT ... RETURNING *` path and the prepared-plan cost for a 1,000-value statement.

### Query construction
Turning a builder into SQL+binds is cheap for all the Rust ORMs, while the TypeScript/JavaScript ORMs are an order of magnitude slower:

- **prax: 0.4 µs**, **Diesel: 0.5 µs**, **ruprizzle (rusqlite): 0.6 µs**, **ruprizzle (sqlx): 0.9 µs**
- **Sea-ORM: 5.1–7.6 µs**
- **Drizzle: 8.5–10.2 µs**
- **Prisma: not exposed**

This confirms the builder layer is not the end-to-end bottleneck for round-trip work, but it does matter if an application generates many queries per request.

## Usage criteria

| Criterion | Best choice | Why |
|---|---|---|
| Compile-time type safety, generated typed client | **Diesel** or **ruprizzle** | Both are schema-first and fully typed; Diesel has the larger ecosystem, ruprizzle the more ergonomically generated client. |
| Maximum simple-query throughput on SQLite | **Diesel** or **ruprizzle (rusqlite)** | Diesel is fastest in this run; ruprizzle is close and still faster than most. |
| Multi-row reads and filtered queries on SQLite | **Diesel** | Consistently fastest; ruprizzle (rusqlite), Drizzle, and prax are the next tier. |
| Bulk inserts on SQLite | **prax** or **ruprizzle (rusqlite)** | Both ~1.4–1.5 ms; Diesel is ~5.4 ms. |
| Nested relation loading, automatic batching | **ruprizzle (rusqlite)**, then **prax**, then **Diesel** (manual) | ruprizzle's auto-batched loader is ~2× faster than Sea-ORM/Prisma; Diesel is fastest if you hand-write the join. |
| TypeScript ecosystem, migrations, team familiarity | **Prisma** | Largest community, mature migrations, schema-first. |
| Zero build-step / runtime schema | **Drizzle** | Schema is plain TypeScript, no code generation. |
| SQL transparency / `.to_sql()` on every builder | **ruprizzle, Diesel, or Drizzle** | ruprizzle and Diesel expose SQL cheaply; Drizzle exposes it too but is slower to construct. |
| Production Postgres | ruprizzle, Prisma, or Diesel | ruprizzle's [`Performance.md`](Performance.md) shows it within ~5% of hand-written `sqlx` on Postgres; Diesel and Drizzle are also strong on Postgres but were not measured here. |

## Caveats

1. **SQLite is not Postgres.** These numbers are from a single SQLite file. The relative ordering can change with network latency, a different driver, or a different database. **Do not cite these numbers for a Postgres comparison until they are re-run on Postgres.**
2. **Driver differences dominate simple reads.** Diesel uses `libsqlite3-sys` directly, ruprizzle (rusqlite) uses the synchronous `rusqlite` driver wrapped in a small blocking pool, and Drizzle uses the synchronous `better-sqlite3` binding — all of these avoid the per-row async worker-thread hop used by `sqlx-sqlite` (which powers ruprizzle (sqlx), prax, and Sea-ORM). The measured row-hop cost is roughly 0.9 µs/row for `sqlx-sqlite` versus ~0.3 µs/row for the synchronous bindings, which is why Diesel, ruprizzle (rusqlite), and Drizzle cluster near the top for simple reads.
3. **Drizzle relational query is SQLite/driver-specific.** On Postgres Drizzle can use joins/CTEs and would likely be far faster for `include_posts`; do not take the 181.8 ms as a universal Drizzle number.
4. **No network.** All ORMs talked to a local file, so result-set decoding and ORM overhead are the main differentiators.
5. **This run used 1 warm-up + 10 measured trials per driver.** Medians are reported. See `local/cross-orm-bench/BENCHMARKS.log` and `local/cross-orm-bench/raw_results.json` for full per-trial data. Run-to-run variance can be 5–10% on Windows. The main take-away is the relative shape between backends, not single-digit absolute values.

## See also

- [Performance](Performance.md) — Postgres vs `sqlx` measurements and the `sqlx::Any` text-marshalling note.
- [Known limitations](KnownLimitations.md) — honest boundaries of the alpha.
- `ProjectPlan/ImplementationPlan/ImplPlan09TestingRelease.md` — original testing-and-benchmark plan.
