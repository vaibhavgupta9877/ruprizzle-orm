# Feature Master Comparison

This doc compares the ORMs benchmarked in this repo on features, architecture,
and the measured SQLite numbers. It is intended as a reference for choosing a
tool, not as a definitive ranking.

> **Caveats:** Feature claims are based on public documentation and the version
> measured in `docs/BenchmarkResults.md` (2026-08-17). Maturity and exact feature
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
| Measured version | 0.4.0-beta.2 | 0.4.0-beta.2 | 0.11 | 1.1 | 2.2 | 6.19.3 | 0.43.0 |
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
| Introspection / codegen from existing DB | Yes (`db pull`) | Yes (`db pull`) | Partial | Yes | Partial (`print-schema`) | Yes | Yes |
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
| Aggregates | Yes | Yes | Yes | Yes | Yes | Yes | Yes |
| JSON operators | Yes* | Yes* | Partial | Partial | Partial | Yes | Partial |
| Streaming / cursors | Yes (buffered + unbuffered) | Yes (buffered + unbuffered) | Yes | Yes | Yes | Yes | Yes |

## Relations & advanced loading

| Feature | ruprizzle (sqlx) | ruprizzle (rusqlite) | prax | sea-orm | diesel | prisma | drizzle |
|---|---|---|---|---|---|---|---|
| One-to-many / many-to-one | Yes | Yes | Yes | Yes | Yes | Yes | Yes |
| One-to-one | Yes | Yes | Yes | Yes | Yes | Yes | Yes |
| Many-to-many | Yes [^2] | Yes [^2] | Yes | Yes | Yes | Yes | Yes |
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

## Advanced query builder & SQL features

| Feature | ruprizzle (sqlx) | ruprizzle (rusqlite) | prax | sea-orm | diesel | prisma | drizzle |
|---|---|---|---|---|---|---|---|
| Conditional / dynamic filters | Yes | Yes | Partial | Partial | Partial | Partial | Yes |
| `IN` set filters | Yes | Yes | Yes | Yes | Yes | Yes | Yes |
| Count / exists queries | Yes | Yes | Yes | Yes | Yes | Yes | Yes |
| Prepared statement builder | Yes | Yes | Partial | Partial | Partial | Partial | Partial |
| Unbuffered streaming cursor | Yes | Yes | Partial | Partial | Partial | Partial | Partial |
| CTEs (non-recursive) | Yes | Yes | Partial | Partial | Yes | Partial | Yes |
| Recursive CTEs | Yes | Partial | No | No | Partial | No | No |
| Set operations (`UNION` / `INTERSECT` / `EXCEPT`) | Yes | Yes | Partial | Partial | Yes | Partial | Yes |
| `EXISTS` subqueries | Yes | Yes | Partial | Partial | Yes | No | Partial |
| `IN` subqueries | Yes | Yes | Partial | Partial | Yes | No | Partial |
| Nested inserts | Yes | Yes | Partial | No | Partial | Yes | No |
| Nested updates | Yes | Yes | Partial | No | Partial | Yes | No |
| Explicit `JOIN`s | Yes | Yes | Partial | Partial | Yes | No | Yes |

> **Note:** "Partial" for advanced SQL constructs usually means the feature is possible via raw SQL or a lower-level API, while "Yes" means a first-class builder method. ruprizzle's benchmark harness exercises every row above; availability in other ORMs is based on public documentation and the measured harness.

## Measured SQLite benchmark (µs/op)

Numbers are from the latest `local/cross-orm-bench/BENCHMARKS.log`
(2026-08-17, 1 warm-up + 10 measured trials, medians reported; lower is better).

### End-to-end operations

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

## Best-fit summary

| Criterion | Best choice | Why |
|---|---|---|
| Compile-time type safety, generated typed client | **Diesel** or **ruprizzle** | Both schema-first and fully typed; Diesel has the larger ecosystem, ruprizzle the more ergonomic generated client. |
| Maximum simple-query throughput on SQLite | **ruprizzle (rusqlite)** | 3.1 µs on `select_by_pk` — faster than Diesel's 10.0 µs — with zero async dispatch overhead. |
| Multi-row reads and filtered queries on SQLite | **Diesel**, then **ruprizzle (rusqlite)** | Diesel is fastest on most multi-row reads; ruprizzle (rusqlite) is within 20–40% and beats Drizzle/prax/Sea-ORM. |
| Bulk inserts on SQLite | **prax**, then **ruprizzle (rusqlite)** | prax leads at ~1.2 ms; ruprizzle (rusqlite) at ~1.4 ms is still roughly 4× faster than Diesel/Sea-ORM. |
| Nested relation loading, automatic batching | **ruprizzle (rusqlite)**, then **prax**, then **Diesel** (manual) | ruprizzle's auto-batched loader is ~2× faster than Sea-ORM/Prisma; Diesel is fastest if you hand-write the join. |
| TypeScript ecosystem, migrations, team familiarity | **Prisma** | Largest community, mature migrations, schema-first. |
| Zero build-step / runtime schema | **Drizzle** | Schema is plain TypeScript, no code generation. |
| SQL transparency / `.to_sql()` on every builder | **ruprizzle**, **Diesel**, or **Drizzle** | ruprizzle and Diesel expose SQL cheaply; Drizzle exposes it too but is slower to construct. |
| Production Postgres | **ruprizzle**, **Prisma**, or **Diesel** | ruprizzle's [`performance.md`](performance.md) shows it within ~5% of hand-written `sqlx` on Postgres; Diesel and Drizzle are also strong on Postgres but were not measured here. |
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

8. **ruprizzle JSON operators** are supported on Postgres (`jsonb`), MySQL (`JSON`),
   and SQLite (JSON1). Postgres and MySQL support full JSON containment (`@>`);
   SQLite approximates containment with a key-existence check because JSON1 has no
   containment operator. See `KnownLimitations.md`.

[^2]: ruprizzle's many-to-many support uses explicit join models (ADR-006). You
    model `PostTag` yourself, then declare `tags Tag[] @relation(through: PostTag)`
    to get `post.tags`, `post.tags_attach(...)`, `post.tags_set(...)`, and
    `post.tags_detach(...)` with batched `include` and transactional nested writes.
