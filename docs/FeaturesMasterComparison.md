# Feature Master Comparison

This doc compares the ORMs benchmarked in this repo on features, architecture,
and the measured SQLite numbers. It is intended as a reference for choosing a
tool, not as a definitive ranking.

> **Caveats:** Feature claims are based on public documentation and the version
> measured in `docs/BenchmarkResults.md` (2026-08-12). Maturity and exact feature
> availability can change quickly, especially for alpha/early projects. Always
> verify against the upstream docs for your specific use case.

## Legend

- **Yes** — first-class, documented, supported.
- **Partial** — possible with caveats, extra tooling, or limited scope.
- **No** — not supported or not the intended workflow.
- **N/A** — not applicable to the tool's design.

**ruprizzle (rusqlite)** is the same runtime and query builder as
**ruprizzle (sqlx)**; it only swaps the SQLite driver from `sqlx::Any` to the
native `rusqlite` crate. PostgreSQL connections still go through `sqlx` in both
variants.

## High-level architecture

| Feature | ruprizzle (sqlx) | ruprizzle (rusqlite) | prax | sea-orm | diesel | prisma | drizzle |
|---|---|---|---|---|---|---|---|
| Language | Rust | Rust | Rust | Rust | Rust | TypeScript | TypeScript |
| Measured version | 0.1.1-beta.1 | 0.1.1-beta.1 | 0.11 | 1.1 | 2.2 | 6.19.3 | 0.43.0 |
| Primary driver | sqlx (Any) | sqlx for Postgres, rusqlite for SQLite | tokio-postgres / sqlx / mysql_async / tokio-rusqlite | sqlx | libsqlite3-sys / mysqlclient / libpq | Prisma query engine + driver adapters | Node database drivers |
| Async API | Yes | Yes (sync driver called on Tokio task) | Yes | Yes | Sync (blocking) | Yes | Yes / sync driver option |
| Query style | Schema-first typed builder | Same as sqlx variant | Prisma-like fluent builder | ActiveRecord / Entity + builder | Type-safe DSL | Generated fluent client | SQL-like typed builder |
| Schema source of truth | `schema.ruprizzle` | Same | `.prax` schema | Entity files or DB first | `table!` macros / `schema.rs` | `schema.prisma` | TypeScript schema files |
| No hidden query engine / sidecar binary | Yes | Yes | Yes | Yes | Yes | No | Yes |
| SQL transparency (`.to_sql()` on every builder) | Yes | Yes | Partial | No | Partial | Partial | Yes (SQL-first) |
| CLI for generate / migrate | `ruprizzle` | Same | `prax` | `sea-orm-cli` | `diesel_cli` | `prisma` | `drizzle-kit` |
| Multi-tenancy | No | No | Yes | Partial | No | Partial | Partial |
| Vector / pgvector search | No | No | Yes | Partial | No | No | No |
| Framework integrations | Any async runtime | Same | Axum, Actix, Armature | Axum, Actix, Loco, Salvo, Poem | Any | Nest, Next, etc. | Any TS framework |

## Database support

| Database | ruprizzle (sqlx) | ruprizzle (rusqlite) | prax | sea-orm | diesel | prisma | drizzle |
|---|---|---|---|---|---|---|---|
| PostgreSQL | Yes | Yes | Yes | Yes | Yes | Yes | Yes |
| MySQL / MariaDB | Yes | Yes | Yes | Yes | Yes | Yes | Yes |
| SQLite | Yes | Yes (native) | Yes | Yes | Yes | Yes | Yes |
| Microsoft SQL Server | No | No | Yes | Partial | No | Yes | Yes |
| MongoDB | No | No | Yes | No | No | Yes | No |
| CockroachDB | No | No | Partial | Partial | No | Yes | Yes |
| DuckDB | No | No | Yes | No | No | No | No |
| ScyllaDB / Cassandra | No | No | Yes | No | No | No | No |
| SingleStore | No | No | No | No | No | No | Yes |
| Serverless / edge (Neon, Turso, D1, PlanetScale) | Partial | Partial | Partial | Partial | No | Yes | Yes |

## Schema & code generation

| Feature | ruprizzle (sqlx) | ruprizzle (rusqlite) | prax | sea-orm | diesel | prisma | drizzle |
|---|---|---|---|---|---|---|---|
| Declarative schema DSL | Yes | Yes | Yes | No | No | Yes | No |
| Code-first schema in source | No | No | No | Partial | Partial | No | Yes |
| Generated typed client | Yes | Yes | Yes | Partial | Partial | Yes | No |
| Schema-first migrations (diff from schema) | Yes | Yes | Yes | Partial | No | Yes | Partial |
| Introspection / codegen from existing DB | Planned | Planned | Partial | Yes | Partial (`print-schema`) | Yes | Yes |
| Compile-time query checking | Planned | Planned | Yes | No | Yes | N/A | No |
| Type-safe column tokens | Yes | Yes | Yes | Partial | Yes | Yes (generated types) | Yes (typed columns) |
| Type-safe nested `include` | Yes | Yes | Yes | Partial | No | Yes | Yes |
| Enum code generation | Yes | Yes | Yes | Partial | Partial | Yes | Yes |
| No runtime parser/codegen in dependency tree | Yes | Yes | Partial | Partial | Yes | No | Yes |

## Query builder & type safety

| Feature | ruprizzle (sqlx) | ruprizzle (rusqlite) | prax | sea-orm | diesel | prisma | drizzle |
|---|---|---|---|---|---|---|---|
| CRUD builders | Yes | Yes | Yes | Yes | Yes | Yes | Yes |
| Raw SQL escape hatch | Yes | Yes | Yes | Yes | Yes | Yes | Yes |
| Type-safe filters | Yes | Yes | Yes | Partial | Yes | Partial | Partial |
| Boolean filter combinators (`and` / `or`) | Yes | Yes | Yes | Yes | Yes | Yes | Yes |
| Projections (select subset) | Yes | Yes | Yes | Yes | Yes | Yes | Yes |
| Pagination (`limit` / `offset` / cursor) | Yes | Yes | Yes | Yes | Yes | Yes | Yes |
| Ordering | Yes | Yes | Yes | Yes | Yes | Yes | Yes |
| Upsert / on-conflict | Yes | Yes | Yes | Yes | Yes | Yes | Yes |
| Bulk insert many | Yes | Yes | Yes | Yes | Yes | Yes | Yes |
| Window functions / row numbering | Yes | Yes | Yes | Partial | Yes | Yes | Yes |
| Aggregates | Partial | Partial | Yes | Yes | Yes | Yes | Yes |
| JSON operators | Partial | Partial | Partial | Partial | Partial | Yes | Partial |
| Streaming / cursors | Buffered | Buffered | Yes | Yes | Yes | Yes | Yes |

## Relations & advanced loading

| Feature | ruprizzle (sqlx) | ruprizzle (rusqlite) | prax | sea-orm | diesel | prisma | drizzle |
|---|---|---|---|---|---|---|---|
| One-to-many / many-to-one | Yes | Yes | Yes | Yes | Yes | Yes | Yes |
| One-to-one | Yes | Yes | Yes | Yes | Yes | Yes | Yes |
| Many-to-many | Partial | Partial | Yes | Yes | Yes | Yes | Yes |
| Self-referential relations | Yes | Yes | Yes | Yes | Yes | Yes | Yes |
| Nested relation `include` | Yes | Yes | Yes | Partial | No | Yes | Yes |
| Batched / auto N+1 avoidance | Yes (bounded 1 query/level) | Same | Yes | Yes (data loader) | Manual join | Yes (join or query) | Yes |
| Per-relation filters and `take` | Yes | Yes | Yes | Partial | No | Yes | Yes |
| Lazy loading | No | No | Yes | Yes | No | Yes | No |

## Migrations & tooling

| Feature | ruprizzle (sqlx) | ruprizzle (rusqlite) | prax | sea-orm | diesel | prisma | drizzle |
|---|---|---|---|---|---|---|---|
| Automatic migration diffing from schema | Yes | Yes | Yes | Partial | No | Yes | Yes |
| Migration rollback (`down.sql`) | Yes | Yes | Yes | Yes | Yes | Yes | Yes |
| CLI for migrate / seed / status | Yes | Yes | Yes | Yes | Yes | Yes | Yes |
| DB push (prototyping without migration files) | Yes | Yes | Yes | No | No | Yes | Yes |
| Drift detection | Yes | Yes | Partial | No | No | Partial | No |
| Offline / embedded migrations | No | No | No | Partial | Yes | No | No |
| Transactional migrations | Yes | Yes | Yes | Yes | Yes | Yes | Yes |

## Measured SQLite benchmark (µs/op)

Numbers are from the latest `local/cross-orm-bench/BENCHMARKS.log`
(2026-08-12, 1 warm-up + 10 measured trials, medians reported; lower is better).

| Operation | ruprizzle (sqlx) | ruprizzle (rusqlite) | prax | sea-orm | diesel | prisma | drizzle |
|---|---|---|---|---|---|---|---|
| `select_by_pk` | 21.2 | 3.0 | 29.9 | 75.8 | 9.9 | 162.3 | 29.0 |
| `find_many_1000` | 1,035.7 | 229.6 | 508.0 | 1,101.5 | 180.1 | 2,038.0 | 300.0 |
| `find_filtered_ordered` | 1,171.5 | 375.1 | 619.1 | 1,191.4 | 272.9 | 2,253.4 | 343.1 |
| `include_posts` | 13,129.6 | 4,467.7 | 7,508.5 | 23,437.1 | 2,812.9 | 33,534.4 | 181,550.9 |
| `bulk_insert_1000` | 2,113.9 | 1,191.6 | 1,440.5 | 5,854.1 | 5,335.6 | 13,153.9 | 8,566.7 |
| `to_sql_select_by_pk` | 0.9 | 0.6 | 0.4 | 5.0 | 0.5 | — | 8.2 |
| `to_sql_select_filter_order` | 3.3 | 1.5 | 0.4 | 7.6 | 0.7 | — | 9.8 |

## Best-fit summary

| Criterion | Best choice | Why |
|---|---|---|
| Compile-time type safety, generated typed client | **Diesel** or **ruprizzle** | Both schema-first and fully typed; Diesel has the larger ecosystem, ruprizzle the more ergonomic generated client. |
| Maximum simple-query throughput on SQLite | **ruprizzle (rusqlite)** | 3.0 µs on `select_by_pk` — faster than Diesel's 9.9 µs — with zero async dispatch overhead. |
| Multi-row reads and filtered queries on SQLite | **Diesel**, then **ruprizzle (rusqlite)** | Diesel is fastest; ruprizzle (rusqlite) is within 20–40% and beats Drizzle/prax. |
| Bulk inserts on SQLite | **ruprizzle (rusqlite)**, then **prax** | 1.2–1.4 ms, roughly 4× faster than Diesel. |
| Nested relation loading, automatic batching | **ruprizzle (rusqlite)**, then **prax**, then **Diesel** (manual) | ruprizzle's auto-batched loader is ~2× faster than Sea-ORM/Prisma; Diesel is fastest if you hand-write the join. |
| TypeScript ecosystem, migrations, team familiarity | **Prisma** | Largest community, mature migrations, schema-first. |
| Zero build-step / runtime schema | **Drizzle** | Schema is plain TypeScript, no code generation. |
| SQL transparency / `.to_sql()` on every builder | **ruprizzle**, **Diesel**, or **Drizzle** | ruprizzle and Diesel expose SQL cheaply; Drizzle exposes it too but is slower to construct. |
| Production Postgres | **ruprizzle**, **Prisma**, or **Diesel** | ruprizzle's [`Performance.md`](Performance.md) shows it within ~5% of hand-written `sqlx` on Postgres; Diesel and Drizzle are also strong on Postgres but were not measured here. |
| Multi-tenancy or vector search out of the box | **prax** | Advertises row-level security, schema/database isolation, and pgvector integration. |

## Footnotes

1. **ruprizzle (rusqlite)** is a backend feature, not a separate ORM. It reuses the
   same runtime, query builder, and `schema.ruprizzle` as **ruprizzle (sqlx)**, but
   uses the synchronous `rusqlite` crate for SQLite connections instead of
   `sqlx::Any`. Postgres still uses `sqlx` in both variants.

2. **Prisma** ships a Rust query engine inside the client and uses a Node.js CLI
   for migration generation, so it is not "no hidden engine" in the same sense as
   the Rust-native libraries.

3. **Drizzle** is SQL-first and code-first by design; there is no separate
   declarative schema DSL. You write the schema in TypeScript and `drizzle-kit`
   can generate or push migrations from it.

4. **Diesel** migrations are file-based (`up.sql` / `down.sql`) and not
   auto-generated from a declarative schema diff, although `diesel print-schema`
   can generate a `schema.rs` from an existing database.

5. **Sea-ORM** is entity-first: you can generate entity files from an existing DB
   with `sea-orm-cli generate entity`, or write them by hand. It does not have a
   single declarative schema file as the source of truth by default.

6. **prax** advertises very broad database support (PostgreSQL, MySQL, SQLite,
   MSSQL, MongoDB, DuckDB, ScyllaDB) plus pgvector, multi-tenancy, and schema
   import, but it is a younger project and its implementation coverage should be
   verified for production workloads.

7. **SQLite-only benchmark:** These numbers are from a single local SQLite file.
   Driver choice dominates simple queries, and the relative ordering can change on
   Postgres or with network latency. See `docs/BenchmarkResults.md` for the full
   methodology.
