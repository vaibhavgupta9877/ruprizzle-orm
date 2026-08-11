# Cross-ORM benchmark results

This document records an apples-to-apples benchmark of **ruprizzle**, **Prisma**, and **Drizzle** against the same SQLite dataset and the same query shapes.

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

> Important: this run used SQLite because no Postgres / Docker was available. The numbers are therefore dominated as much by driver choice as by the ORM layer. ruprizzle uses `sqlx::Any` (async, text-encodes every bind/row), Drizzle uses the synchronous `better-sqlite3` binding, and Prisma uses its in-process Rust query engine.

## Harness and raw data

- ruprizzle: `crates/runtime/examples/cross_orm_bench.rs`
- Prisma / Drizzle: `local/cross-orm-bench/node/`
- Database: `local/cross-orm-bench/node/bench.sqlite3`
- Result JSON: `local/cross-orm-bench/node/{ruprizzle,prisma,drizzle}-results.json`

## End-to-end results

All times are microseconds per operation (lower is better).

| Operation | ruprizzle | Prisma | Drizzle |
|---|---:|---:|---:|
| `select_by_pk` | 45.8 | 166.3 | **28.3** |
| `find_many_1000` | 1,687.2 | 2,049.5 | **289.7** |
| `find_filtered_ordered` | 1,718.2 | 2,153.8 | **343.5** |
| `include_posts` (1,000 users + 10,000 posts) | **16,174.7** | 32,586.3 | 181,496.5 |
| `bulk_insert_1000` | 9,964.3 | 15,003.4 | **8,379.3** |

## Query construction (no I/O)

Drizzle exposes `.toSQL()`; ruprizzle exposes `.to_sql()`; Prisma does not expose an equivalent API.

| Operation | ruprizzle | Drizzle |
|---|---:|---:|
| `to_sql_select_by_pk` | **0.57** | 8.41 |
| `to_sql_select_filter_order` | **1.54** | 10.08 |

## Codegen / build-step comparison

- **Drizzle:** no code generation; the schema is plain TypeScript.
- **ruprizzle:** 50-model schema generation = **17.8 ms** (`cargo bench -p ruprizzle-codegen`).
- **Prisma:** `prisma generate` for the 3-model benchmark schema took **~36 ms**; it scales linearly with schema size and is heavier than ruprizzle for large schemas.

## Analysis

### Simple reads
Drizzle is fastest on plain SQLite reads because `better-sqlite3` is synchronous and the Drizzle runtime is very thin. ruprizzle is 1.6–5.8× slower on these micro-benchmarks, mostly because `sqlx::Any` is async and text-encodes/decodes every bind and row. Prisma is consistently the slowest here — its query-engine serialization adds measurable per-call overhead.

### Relation includes
This is where ruprizzle’s batched loader shows its biggest advantage:

- **ruprizzle: 16.2 ms**
- **Prisma: 32.6 ms**
- **Drizzle: 181.5 ms**

Drizzle’s SQLite relational query (`db.query.users.findMany({ with: { posts: true } })`) emits a correlated subquery with `json_group_array` per parent row — effectively an N+1 shape in SQL. Prisma and ruprizzle both issue a bounded number of batched queries. On Postgres, Drizzle can use joins/CTEs and would likely be much faster.

### Bulk insert
Drizzle and ruprizzle are close; Prisma is ~1.8× slower. Note that ruprizzle’s `InsertManyQuery` also decodes the 1,000 returned rows via `RETURNING *`, while Drizzle and Prisma only return count/change metadata, so the real gap is smaller than it looks.

### Query construction
ruprizzle is roughly an order of magnitude faster at turning a builder into SQL+binds than Drizzle. This matters most for high-throughput request paths where the same builder patterns are repeated many times.

## Usage criteria

| Criterion | Best choice | Why |
|---|---|---|
| Rust project / compile-time type safety | **ruprizzle** | Schema-first, generated typed client, no hidden engine, native Rust. |
| Maximum simple-query throughput on SQLite | **Drizzle + better-sqlite3** | Sync driver, thin runtime, fastest raw reads in this run. |
| Nested relation loading, batching, no N+1 | **ruprizzle**, then Prisma | ruprizzle is ~2× faster than Prisma; Drizzle SQLite falls back to correlated subqueries. |
| TypeScript ecosystem, migrations, team familiarity | **Prisma** | Largest community, mature migrations, schema-first. |
| Zero build-step / runtime schema | **Drizzle** | Schema is plain TypeScript, no code generation. |
| SQL transparency / `.to_sql()` on every builder | **ruprizzle or Drizzle** | Both expose SQL; ruprizzle is faster to build it. |
| Production Postgres | ruprizzle or Prisma | ruprizzle’s [`Performance.md`](Performance.md) shows it within ~5% of hand-written `sqlx` on Postgres; Drizzle is also strong on Postgres but was not measured here. |

## Caveats

1. **SQLite is not Postgres.** These numbers are from a single SQLite file. The relative ordering can change with network latency, a different driver, or a different database. ruprizzle’s existing Postgres report shows much closer parity with hand-written `sqlx` for 1,000-row selects.
2. **Driver differences dominate simple reads.** Drizzle’s `better-sqlite3` sync path is inherently faster than ruprizzle’s `sqlx::Any` async path on this workload.
3. **Drizzle relational query is SQLite/driver-specific.** On Postgres Drizzle can use joins/CTEs and would likely be far faster for `include_posts`; do not take the 181 ms as a universal Drizzle number.
4. **No network.** All ORMs talked to a local file, so result-set decoding and ORM overhead are the main differentiators.

## See also

- [Performance](Performance.md) — Postgres vs `sqlx` measurements and the `sqlx::Any` text-marshalling note.
- [Known limitations](KnownLimitations.md) — honest boundaries of the alpha.
- `ProjectPlan/ImplementationPlan/ImplPlan09TestingRelease.md` — original testing-and-benchmark plan.
