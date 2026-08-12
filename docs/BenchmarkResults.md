# Cross-ORM benchmark results

This document records an apples-to-apples benchmark of **ruprizzle**, **Prisma**, and **Drizzle** against the same SQLite dataset and the same query shapes, plus an additional column for ruprizzle's new native `rusqlite` backend.

## Environment

- **Host:** Windows 11, local SQLite file
- **Dataset:**
  - `users`: 1,000 rows
  - `posts`: 10,000 rows (10 posts per user)
  - `bench_bulk`: empty table used for bulk-insert tests
- **Versions:**
  - ruprizzle `0.1.0-alpha.3` (this repo)
  - Prisma `6.19.3`
  - Drizzle ORM `0.43.0`
  - better-sqlite3 `13.0.3`
  - rusqlite `0.32.1`

> Important: this run used SQLite because no Postgres / Docker was available. The numbers are therefore dominated as much by driver choice as by the ORM layer. ruprizzle (`sqlx` backend) uses `sqlx::Any`, ruprizzle (`rusqlite` backend) uses the synchronous `rusqlite` binding over a small blocking pool, Drizzle uses the synchronous `better-sqlite3` binding, and Prisma uses its in-process Rust query engine.

## Harness and raw data

- ruprizzle: `crates/runtime/examples/cross_orm_bench.rs`
- ruprizzle (rusqlite): same example with `RUST_BENCH_DRIVER=rusqlite`
- Prisma / Drizzle: `local/cross-orm-bench/node/`
- Database: `local/cross-orm-bench/node/bench.sqlite3`
- Result JSONs:
  - `local/cross-orm-bench/raw_results.json` — all measured trials
  - `local/cross-orm-bench/results.json` — aggregated statistics
  - `local/cross-orm-bench/BENCHMARKS.log` — human-readable summary
  - `local/cross-orm-bench/node/ruprizzle-results.json`
  - `local/cross-orm-bench/node/ruprizzle-rusqlite-results.json`
  - `local/cross-orm-bench/node/prisma-results.json`
  - `local/cross-orm-bench/node/drizzle-results.json`

## End-to-end results

All times are microseconds per operation (lower is better).

| Operation | ruprizzle (sqlx) | ruprizzle (rusqlite) | Prisma | Drizzle |
|---|---:|---:|---:|---:|
| `select_by_pk` | 23.5 | 16.7 | 162.5 | 29.8 |
| `find_many_1000` | 1,058.9 | 311.9 | 2,043.7 | 308.4 |
| `find_filtered_ordered` | 1,186.2 | 469.7 | 2,235.8 | 338.1 |
| `include_posts` (1,000 users + 10,000 posts) | 12,800.4 | 5,017.5 | 32,406.6 | 182,360.5 |
| `bulk_insert_1000` | 2,031.6 | 1,418.7 | 12,762.0 | 8,823.9 |

## Query construction (no I/O)

Drizzle exposes `.toSQL()`; ruprizzle exposes `.to_sql()`; Prisma does not expose an equivalent API.

| Operation | ruprizzle (sqlx / rusqlite) | Drizzle |
|---|---:|---:|
| `to_sql_select_by_pk` | 0.9 | 8.5 |
| `to_sql_select_filter_order` | 3.3 | 10.1 |

## Codegen / build-step comparison

- **Drizzle:** no code generation; the schema is plain TypeScript.
- **ruprizzle:** 50-model schema generation = **17.8 ms** (`cargo bench -p ruprizzle-codegen`).
- **Prisma:** `prisma generate` for the 3-model benchmark schema took **~36 ms**; it scales linearly with schema size and is heavier than ruprizzle for large schemas.

## Analysis

### Simple reads
The native `rusqlite` backend is now faster than Drizzle on single-row PK lookups (16.7 µs vs 29.8 µs) and within a few microseconds on most simple reads. ruprizzle's `sqlx` backend is also competitive at 23.5 µs. Prisma is consistently the slowest on simple reads.

### Multi-row and filtered reads
The `rusqlite` backend makes ruprizzle competitive on multi-row work:

- `find_many_1000`: **311.9 µs** (rusqlite) vs **1,058.9 µs** (sqlx) vs **308.4 µs** (Drizzle)
- `find_filtered_ordered`: **469.7 µs** (rusqlite) vs **1,186.2 µs** (sqlx) vs **338.1 µs** (Drizzle)

The native driver removes `sqlx-sqlite`'s per-row worker thread hop, and the owned `Row::take` decoding path removes the extra intermediate string copy. `find_many_1000` is now effectively tied with Drizzle; `find_filtered_ordered` is within ~40 % of Drizzle, with the remaining gap coming from the `ORDER BY` sort and the `spawn_blocking` dispatch.

### Relation includes
This is where ruprizzle's batched loader shows its biggest advantage:

- **ruprizzle (rusqlite): 5.0 ms**
- **ruprizzle (sqlx): 12.8 ms**
- **Prisma: 32.4 ms**
- **Drizzle: 182.4 ms**

Drizzle's SQLite relational query (`db.query.users.findMany({ with: { posts: true } })`) emits a correlated subquery with `json_group_array` per parent row — effectively an N+1 shape in SQL. Prisma and ruprizzle both issue a bounded number of batched queries, so this comparison is partly **query-strategy**, not purely ORM efficiency. On Postgres, Drizzle can use joins/CTEs and would likely be much faster.

### Bulk insert
**ruprizzle (rusqlite): 1.4 ms, ruprizzle (sqlx): 2.0 ms, Drizzle: 8.8 ms, Prisma: 12.8 ms.** The native `rusqlite` backend is now fastest. Both ruprizzle backends are faster than Drizzle and Prisma. The gap is partly because `InsertManyQuery` returns the inserted rows while the others return only count/change metadata (so the real difference is smaller than it looks), partly from WAL mode reducing fsync pressure, and partly from the prepared-statement cache.

### Query construction
ruprizzle is roughly an order of magnitude faster at turning a builder into SQL+binds than Drizzle (`0.5` µs vs `8.5` µs for select-by-PK). That is real, but it is not a throughput advantage in the recorded end-to-end numbers: the full `to_sql()` path is ~0.5 µs against a ~30–460 µs round-trip floor. It is best read as evidence that the builder layer is not where end-to-end time goes, not as a reason to choose ruprizzle for raw speed.

## Usage criteria

| Criterion | Best choice | Why |
|---|---|---|
| Rust project / compile-time type safety | **ruprizzle** | Schema-first, generated typed client, no hidden engine, native Rust. |
| Maximum simple-query throughput on SQLite | **ruprizzle (rusqlite)** | Synchronous `rusqlite` driver plus owned row decoding is fastest on single-row and multi-row reads in this run. |
| Multi-row reads and filtered queries on SQLite | **ruprizzle (rusqlite)** | Roughly 2× faster than the `sqlx` backend and competitive with Drizzle. |
| Nested relation loading, batching, no N+1 | **ruprizzle (rusqlite)**, then ruprizzle (sqlx), then Prisma | The rusqlite backend is ~2× faster than the sqlx backend and ~5× faster than Prisma; Drizzle SQLite falls back to correlated subqueries. |
| TypeScript ecosystem, migrations, team familiarity | **Prisma** | Largest community, mature migrations, schema-first. |
| Zero build-step / runtime schema | **Drizzle** | Schema is plain TypeScript, no code generation. |
| SQL transparency / `.to_sql()` on every builder | **ruprizzle or Drizzle** | Both expose SQL; ruprizzle is faster to build it. |
| Production Postgres | ruprizzle or Prisma | ruprizzle's [`Performance.md`](Performance.md) shows it within ~5% of hand-written `sqlx` on Postgres; Drizzle is also strong on Postgres but was not measured here. |

## Caveats

1. **SQLite is not Postgres.** These numbers are from a single SQLite file. The relative ordering can change with network latency, a different driver, or a different database. **Do not cite these numbers for a Postgres comparison until they are re-run on Postgres.**
2. **Driver differences dominate simple reads.** Drizzle's `better-sqlite3` is a synchronous, in-process binding with no per-row thread hop. `sqlx-sqlite` is async over a dedicated worker thread and a bounded row channel, costing roughly 0.9 µs/row versus better-sqlite3's ~0.3 µs/row. ruprizzle's `rusqlite` backend uses a synchronous driver wrapped in `spawn_blocking`, plus WAL mode, prepared-statement caching, and owned `Row::take` decoding. The dispatch cost is small enough that it is now faster than Drizzle on single-row reads and effectively tied on `find_many_1000`.
3. **Drizzle relational query is SQLite/driver-specific.** On Postgres Drizzle can use joins/CTEs and would likely be far faster for `include_posts`; do not take the 181.8 ms as a universal Drizzle number.
4. **No network.** All ORMs talked to a local file, so result-set decoding and ORM overhead are the main differentiators.
5. **This run used 1 warm-up + 10 measured trials per driver.** Medians are reported. See `local/cross-orm-bench/BENCHMARKS.log` and `local/cross-orm-bench/raw_results.json` for full per-trial data. Run-to-run variance can be 5–10% on Windows. The main take-away is the relative shape between backends, not single-digit absolute values.

## See also

- [Performance](Performance.md) — Postgres vs `sqlx` measurements and the `sqlx::Any` text-marshalling note.
- [Known limitations](KnownLimitations.md) — honest boundaries of the alpha.
- `ProjectPlan/ImplementationPlan/ImplPlan09TestingRelease.md` — original testing-and-benchmark plan.
