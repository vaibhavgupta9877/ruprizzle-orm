# ruprizzle

[![Crates.io](https://img.shields.io/crates/v/ruprizzle.svg)](https://crates.io/crates/ruprizzle)
[![docs.rs](https://docs.rs/ruprizzle/badge.svg)](https://docs.rs/ruprizzle)
[![MSRV](https://img.shields.io/badge/MSRV-1.85-blue.svg)](./Cargo.toml)
[![License](https://img.shields.io/crates/l/ruprizzle.svg)](./LICENSE-MIT)
[![CI](https://img.shields.io/badge/CI-cargo%20xtask%20ci-success)](./xtask/src/main.rs)

**ruprizzle is a schema-first ORM for Rust — a Prisma-style schema file that generates a typed client, with Drizzle-style SQL transparency and no sidecar binary.**

It combines the best parts of Prisma and Drizzle:

- **Prisma's** declarative schema as the single source of truth, with a generated typed client, automatic migration diffing, and nested relation loading.
- **Drizzle's** SQL transparency — no hidden query engine, no sidecar binary, and `.to_sql()` on every builder so you always know what is being sent to the database.

Postgres, SQLite, and MySQL/MariaDB are supported from day one behind a `DbDialect` trait, so more backends are additive. Built on [`sqlx`](https://github.com/launchbadge/sqlx) for the wire protocol and pooling; ruprizzle does not write its own driver. A native `rusqlite` backend is also available for SQLite via the `sqlite-rusqlite` Cargo feature.

> **Status:** `1.0.0-rc.1` is **published on crates.io** (2026-08-21, tag `v1.0.0-rc.1`). P0–P8 feature work is complete, MySQL/MariaDB support is shipped, and the public API is frozen for the 1.0 line. The 48-hour `rusqlite` soak has been **waived** after 15.56 h / 1.46 B ops / 0 errors (see `docs/SoakReport.md`). The two-week RC feedback window is now open; the W6-05 production-readiness rescore against the published RC is the remaining gate before `1.0.0`. See [Known limitations](#known-limitations) for deliberate boundaries and [Stability](docs/Stability.md) for the semver policy.

---

## Table of contents

- [Quick example](#quick-example)
- [Why ruprizzle?](#why-ruprizzle)
- [Detailed features](#detailed-features)
- [Installation](#installation)
- [Quickstart](#quickstart)
- [Query examples](#query-examples)
- [CLI workflow](#cli-workflow)
- [Comparison with other ORMs](#comparison-with-other-orms)
- [Architecture and repository layout](#architecture-and-repository-layout)
- [Performance](#performance)
- [Status and roadmap](#status-and-roadmap)
- [Known limitations](#known-limitations)
- [Development](#development)
- [Planning documents](#planning-documents)
- [Changelog](#changelog)
- [Licence](#licence)

---

## Quick example

Define your schema:

```prisma
// schema.ruprizzle
datasource db {
  provider = "postgres"
  url      = env("DATABASE_URL")
}

generator client {
  output      = "src/db"
  module_name = "db"
}

model User {
  id        Uuid     @id @default(uuid7())
  email     String   @unique
  name      String?
  posts     Post[]
  createdAt DateTime @default(now()) @map("created_at")

  @@map("users")
}

model Post {
  id        Uuid     @id @default(uuid7())
  title     String
  published Boolean  @default(false)
  authorId  Uuid     @map("author_id")
  author    User     @relation(fields: [authorId], references: [id], onDelete: Cascade)

  @@map("posts")
}
```

Run `ruprizzle generate`, add `mod db;` to your crate, and write queries that mirror the SQL they compile to:

```rust
// SQL-first, type-safe query builder
let admins = db
    .user()
    .find_many()
    .filter(user::EMAIL.ends_with("@acme.com"))
    .order_by(user::CREATED_AT.desc())
    .limit(20)
    .fetch_all()
    .await?;

// Prisma-style relation include, batched — never N+1
let users = db
    .user()
    .find_many()
    .include(user::posts().filter(post::PUBLISHED.eq(true)).take(5))
    .fetch_all()
    .await?;

// Inspect the SQL before it runs
let sql = db
    .user()
    .find_many()
    .filter(user::EMAIL.eq("alice@example.com"))
    .to_sql();
```

Wrong-typed and cross-model filters are compile errors, not runtime ones:

```rust
user::EMAIL.eq(42)                              // error: expected String, found i32
db.post().find_many().filter(user::EMAIL.eq(""))  // error: expected Filter<Post>, found Filter<User>
```

---

## Why ruprizzle?

Most Rust database tools force you to choose between three imperfect options:

1. **Macro-heavy query DSLs** that are powerful but compile slowly and hide the SQL.
2. **String-based SQL helpers** that are transparent but not type-safe.
3. **Active-record style ORMs** that feel ergonomic but drift away from the schema.

ruprizzle tries to give you all three at once:

- **Schema is the single source of truth.** Models, migrations, and the client are derived from `schema.ruprizzle`. You do not hand-edit generated code.
- **No hidden query engine.** No sidecar binary, no WASM engine, no hidden thread pool. The generated client is plain Rust calling `sqlx`.
- **Predictable SQL.** Every builder call maps to a visible SQL fragment. `.to_sql()` is available on every query.
- **Type errors, not runtime errors.** Column tokens are typed `Column<Model, T>`, so `user::email.eq(42)` fails to compile.
- **Escape hatch always present.** `sqlx::query_as!` interop and the `raw!` macro / `RawFragment` predicate are first-class, not a defeat.
- **Dialect differences are explicit.** If Postgres supports something SQLite or MySQL does not, the generator tells you at build time, not at runtime.

---

## Detailed features

### Schema DSL

- **Prisma-inspired `.ruprizzle` syntax** with `datasource`, `generator`, `model`, `enum`, and relation blocks.
- **Native type annotations** such as `@db.Uuid`, `@db.VarChar(255)`, `@db.Text`, `@db.Integer`, `@db.Real`, `@db.Decimal`, `@db.Json`, `@db.Bytes`, `@db.Timestamp`, and dialect-specific extensions.
- **Validation rules** for names, duplicate fields, missing IDs, broken relations, unsupported native types, and more — all reported with source spans, line numbers, and "did you mean?" suggestions.
- **Canonical formatter** (`ruprizzle format`) and watch mode (`ruprizzle generate --watch`) for rapid iteration.
- **Schema fingerprinting** so generated clients can be cheaply invalidated when the schema changes.

### Code generation

- **Generated Rust modules** for every model: entity struct, column tokens, insert/update setters, and relation helpers.
- **Generated `Db` client root** that exposes one accessor per model (`db.user()`, `db.post()`, etc.).
- **Compile-time type-safe column tokens** with model-scoped modules (`user::EMAIL`, `post::PUBLISHED`).
- **Enum code generation** for schema enums, mapped to a Rust enum with `#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]`.
- **No parser or codegen in your runtime dependency graph** — the runtime crate only depends on `sqlx`, `serde`, `chrono`, `uuid`, and `rust_decimal`, keeping application compile times low.

### Query builder

- **CRUD builders**: `select`, `find_many`, `find_by_id`, `find_unique`, `insert`, `insert_many`, `upsert`, `update`, `delete`.
- **Type-safe filters** for equality, inequality, ordering, `IN` sets, nullability, and string matching (`starts_with`, `ends_with`, `contains`).
- **Filter combinators**: `.and(...)`, `.or(...)`, `all([...])`, `any([...])`.
- **Projections** to select only the columns you need.
- **Ordering, pagination, and cursor helpers** (`limit`, `offset`, `after`, `before`, `paginate(Page::new(1, 20))`).
- **Distinct** selection support.
- **SQL transparency**: `.to_sql()` returns the compiled SQL with placeholders for every query.

### Relations and `include`

- **Nested relation loading** with `.include()`: `user::posts()`, `post::author()`, and arbitrary depth.
- **Per-relation filters and ordering**: `user::posts().filter(post::PUBLISHED.eq(true)).order_by(post::CREATED_AT.desc()).take(5)`.
- **Bounded query count**: a two-level `include` issues at most one query per level. The bound is asserted in the test suite, not just by inspection.
- **Foreign-key declarations** with `onDelete` and `onUpdate` actions (`Cascade`, `Restrict`, `SetNull`, `SetDefault`, `NoAction`).
- **One-to-many, many-to-one, and self-referential** relations are supported in this release.

### Migrations

- **Automatic migration diffing** from a declarative schema: `ruprizzle migrate dev` diffs the current schema against the last snapshot, generates `up.sql` / `down.sql`, applies the migration, and regenerates the client.
- **12 change classes** covering create/drop tables, add/drop/rename columns, add/drop indexes, add/drop uniques, add/drop foreign keys, create/alter enums, and table rebuilds on SQLite.
- **Snapshot-based history** stored as a serialised `Schema` in `migrations/.ruprizzle/snapshot.ruprizzle`.
- **Production-safe `migrate deploy`**: applies pending migrations but never diffs or writes new migration files, so the dangerous prototyping command cannot slip into CI.
- **Drift detection** and `migrate status`, `migrate resolve`, `migrate reset`, and `db push` for prototyping.
- **Destructive-change prompts**: `migrate dev` aborts on data-loss unless `--accept-data-loss` is passed.

### CLI

- `ruprizzle init --provider postgres|sqlite|mysql` — scaffold schema, `.env`, `.gitignore`, and `migrations/`.
- `ruprizzle generate` and `ruprizzle generate --watch` — generate the typed client.
- `ruprizzle validate` — CI-friendly schema validation.
- `ruprizzle format` — canonicalise the schema file.
- `ruprizzle migrate dev|deploy|status|resolve|reset` — see [CLI workflow](#cli-workflow).
- `ruprizzle db pull` — introspect an existing database into `schema.ruprizzle`.
- `ruprizzle db seed` — transactionally apply idempotent `seeds/main.json` data (legacy `main.sql` remains supported).
- `ruprizzle db push` — direct schema push without migration files.

A declarative seed file maps model or table names to row arrays:

```json
{"User": [{"id": 1, "email": "alice@example.com"}]}
```

Rows must include their primary key; repeated runs update the existing row instead of inserting a duplicate.

### Dialects

- **Postgres 17+**, **MySQL/MariaDB**, and **SQLite 3+** are supported. MySQL/MariaDB carries an unpatched dependency advisory in its authentication path ([RUSTSEC-2023-0071](https://rustsec.org/advisories/RUSTSEC-2023-0071) via `rsa` -> `sqlx-mysql`); connect over TLS or a unix socket, and see [Known limitations](docs/KnownLimitations.md) before using MySQL in production.
- **Dialect capabilities model**: native enums, native UUID, `RETURNING`, `ALTER COLUMN`, window functions, JSON support, partial indexes, and max bind parameters are explicitly modelled.
- **Portable MySQL DML**: inserts use a primary-key follow-up lookup because MySQL has no DML `RETURNING`; upserts use `ON DUPLICATE KEY UPDATE`.
- **SQLite table rebuilds** for destructive column changes are handled automatically.
- **UUID and JSON** are mapped idiomatically per dialect (`uuid`/`jsonb` on Postgres, `char(36)`/`json` on MySQL, and text on SQLite where native storage is unavailable).

### Transactions and escape hatches

- **First-class transactions**: `db.raw_pool().begin().await?`, `tx.commit().await?`, `tx.rollback().await?`. Builders take `&dyn Executor`, so the same query works against a pool or a transaction.
- **Isolation levels**: `ReadUncommitted`, `ReadCommitted`, `RepeatableRead`, `Serializable`.
- **Raw SQL execution**: `db.raw_pool().fetch_all_raw(sql, params).await?` and `db.raw_pool().execute_raw(sql, params).await?`.
- **Retry helpers**: `ruprizzle::is_retryable(&error)` for transient error handling.

---

## Installation

```bash
# The CLI
$ cargo install ruprizzle-cli

# The runtime crate your application uses
$ cargo add ruprizzle
```

MSRV: **Rust 1.85**.

---

## Quickstart

From an empty directory to a working query in five commands:

```bash
mkdir my-app && cd my-app
ruprizzle init --provider postgres
# edit .env with your DATABASE_URL
# edit schema.ruprizzle
ruprizzle migrate dev --name init
# add `mod db;` to src/main.rs
cargo run
```

A minimal `schema.ruprizzle`:

```prisma
datasource db {
  provider = "postgres"
  url      = env("DATABASE_URL")
}

generator client {
  output      = "src/db"
  module_name = "db"
}

model User {
  id    Int    @id @default(autoincrement())
  email String @unique
  name  String
}
```

And `src/main.rs`:

```rust
mod db;

#[tokio::main]
async fn main() -> Result<(), ruprizzle::Error> {
    let db = db::Db::connect(&std::env::var("DATABASE_URL")?).await?;

    db.user()
        .create(db::UserInsert {
            id: None,
            email: "alice@example.com".into(),
            name: Some("Alice".into()),
        })
        .exec()
        .await?;

    let users = db
        .user()
        .find_many()
        .filter(db::user::EMAIL.ends_with("@example.com"))
        .fetch_all()
        .await?;

    println!("{users:?}");
    Ok(())
}
```

See the full [quickstart](docs/quickstart.md) for a step-by-step walkthrough.

---

## Query examples

### Select

```rust
let users = db
    .user()
    .find_many()
    .filter(user::EMAIL.eq("alice@example.com"))
    .order_by(user::NAME.asc())
    .limit(10)
    .offset(20)
    .fetch_all()
    .await?;
```

### Projections

```rust
let names = db
    .user()
    .find_many()
    .columns(user::NAME)
    .fetch_all()
    .await?;
```

### Insert and upsert

```rust
let user = db
    .user()
    .create(db::UserInsert {
        id: None,
        email: "alice@example.com".into(),
        name: Some("Alice".into()),
    })
    .exec()
    .await?;

// Or build an insert directly:
db.insert::<User>()
    .set(user::EMAIL, "alice@example.com")
    .set_optional(user::NAME, Some("Alice"))
    .exec()
    .await?;

// Insert or update on conflict
db.insert::<User>()
    .set(user::EMAIL, "alice@example.com")
    .set(user::NAME, "Alice")
    .on_conflict(["email"])
    .do_update(["name"])
    .exec()
    .await?;
```

### Pagination

```rust
let page = db
    .user()
    .find_many()
    .order_by(user::ID.asc())
    .page(20)
    .await?;

for user in &page.items {
    println!("{}", user.email);
}
```

### Transactions

```rust
use ruprizzle::prelude::*;

let mut tx = db.raw_pool().begin().await?;

let user = InsertQuery::new(&tx)
    .set(db::user::EMAIL, "a@b.c")
    .exec()
    .await?;

if should_commit {
    tx.commit().await?;
} else {
    tx.rollback().await?;
}
```

### Raw SQL

```rust
use ruprizzle::prelude::*;

let rows = db
    .raw_pool()
    .fetch_all_raw(
        "SELECT * FROM users WHERE email LIKE ?".into(),
        vec![Value::Str("%@example.com".into())],
    )
    .await?;
```

See the [query guide](docs/QueryGuide.md) and [relations guide](docs/RelationsGuide.md) for more.

---

## CLI workflow

| Step | Command |
|---|---|
| Scaffold a project | `ruprizzle init --provider postgres\|sqlite` |
| Generate the client | `ruprizzle generate` |
| Auto-watch in dev | `ruprizzle generate --watch` |
| Create & apply a migration | `ruprizzle migrate dev --name <name>` |
| Apply migrations in CI/prod | `ruprizzle migrate deploy` |
| Check migration status | `ruprizzle migrate status` |
| Validate for CI | `ruprizzle validate` |
| Canonicalise schema | `ruprizzle format` |

`migrate dev` and `migrate deploy` are deliberately separate: the production command never diffs or writes migration files, so habit cannot carry a dangerous prototyping invocation into CI.

---

## Comparison with other ORMs

The table below focuses on the features that differentiate ruprizzle from the tools a Rust team is most likely to evaluate. A full feature, architecture, and benchmark comparison covering ruprizzle (sqlx), ruprizzle (rusqlite), prax, Sea-ORM, Diesel, Prisma, and Drizzle is in [`docs/FeaturesMasterComparison.md`](docs/FeaturesMasterComparison.md).

| Feature | ruprizzle | Diesel | SeaORM | sqlx | prax | Prisma Client Rust | Drizzle |
|---|---|---|---|---|---|---|---|
| **Schema-first code generation** | ✅ | partial | ❌ | ❌ | ✅ | ✅ | ❌ |
| **Declarative schema DSL** | ✅ | ❌ | ❌ | ❌ | ✅ | ✅ | ❌ |
| **Type-safe column tokens** | ✅ | ✅ | partial | ❌ | ✅ | ✅ | ✅ |
| **Type-safe nested `include`** | ✅ | ❌ | partial | ❌ | ✅ | ✅ | ✅ |
| **SQL-first, visible query builder** | ✅ | partial | ❌ | ✅ | partial | partial | ✅ |
| `.to_sql()` on every builder | ✅ | partial | ❌ | N/A | partial | partial | ✅ |
| **Migrations from schema diff** | ✅ | ❌ | partial | ❌ | ✅ | ✅ | partial |
| **Compile-time query checking** | ✅ | ✅ | ❌ | ✅ | ✅ | N/A | ❌ |
| **No sidecar / no hidden engine** | ✅ | ✅ | ✅ | ✅ | ✅ | ❌ | ✅ |
| **Native async / `await`** | ✅ | partial | ✅ | ✅ | ✅ | ✅ | ✅ |
| **Postgres + SQLite + MySQL** | ✅ | ✅ | ✅ | ✅ | ✅ | partial | ✅ |
| **Native `rusqlite` backend** | ✅ | ✅ | ❌ | N/A | partial | ❌ | N/A |
| **Advanced SQL (CTEs, subqueries, set ops)** | ✅ | partial | partial | ✅ | partial | partial | partial |

### What this means in practice

- **Diesel** gives you compile-time checked SQL and strong types, but migrations are hand-written and the DSL can feel macro-heavy. ruprizzle keeps the type safety and adds a declarative schema, generated client, and automatic migration diffing.
- **SeaORM** is ergonomic and active-record style, but the schema is not the single source of truth and the generated code can drift. ruprizzle inverts that: edit `schema.ruprizzle`, run `ruprizzle generate`.
- **sqlx** is the most transparent, but you write SQL and `FromRow` by hand. ruprizzle builds on sqlx and gives you a typed client while preserving the escape hatch.
- **prax** is the closest Rust alternative in philosophy and also uses a declarative schema, but it targets broader database support (MSSQL, MongoDB, DuckDB, ScyllaDB) and ships extra machinery for multi-tenancy and pgvector that ruprizzle intentionally leaves out.
- **Prisma Client Rust** is similar in philosophy, but it ships a Rust query engine and uses a Node sidecar for migration generation. ruprizzle is pure Rust with no sidecar binary.
- **Drizzle** is SQL-first and code-first by design; there is no separate declarative schema DSL, and the schema is plain TypeScript.

---

## Architecture and repository layout

The workspace is split so that parser and codegen never enter the user's runtime dependency graph. Every crate in the table below uses the shared workspace version (`1.0.0-rc.1` at the time of writing).

| Directory | Crate | Role | Ships to users? | Status |
|---|---|---|---|---|
| `crates/core` | `ruprizzle-core` | IR, spans, diagnostics | transitively | ✅ complete |
| `crates/parser` | `ruprizzle-parser` | Schema DSL → validated IR | no (build/CLI only) | ✅ complete |
| `crates/dialect` | `ruprizzle-dialect` | `DbDialect` trait, Postgres + SQLite + MySQL | transitively | ✅ complete |
| `crates/codegen` | `ruprizzle-codegen` | IR → Rust source | no | ✅ complete |
| `crates/runtime` | `ruprizzle` | Query builder, executor, transactions | **yes (published)** | ✅ complete |
| `crates/macros` | `ruprizzle-macros` | `#[derive(FromRow)]` passthrough, `raw!` | **yes (published)** | ✅ complete |
| `crates/migrate` | `ruprizzle-migrate` | Snapshot, diff, plan, apply | transitively | ✅ complete |
| `crates/cli` | `ruprizzle-cli` | The `ruprizzle` binary | **yes (published)** | ✅ complete |
| `crates/testkit` | `ruprizzle-testkit` | Dual-database test harness | no | ✅ complete |

Published crates are available on [crates.io](https://crates.io/crates/ruprizzle). `crates/testkit` is the only crate in the workspace marked `publish = false`; it is used by the integration suite and is not published.

The pipeline is:

```text
schema.ruprizzle
    → parser (Pest grammar + lowering + validation)
    → core IR (Schema)
    → dialect (capabilities, DDL, SQL fragments)
    → codegen (Rust modules)
    → runtime (query builder + sqlx executor)
```

Because the parser and codegen are build-time tools, the runtime crate is thin and application compile times stay low.

---

## Performance

Measured locally during development (no I/O for construction benchmarks):

| Benchmark | Result |
|---|---|
| Query construction (select by PK, no I/O) | ~600 ns |
| Query construction (filter + order, no I/O) | ~1.8 µs |
| Codegen, 50-model schema | ~16 ms |

The latest cross-ORM SQLite run (2026-08-18, 06:04 UTC, 1 warm-up + 10 measured trials, medians) is in [`docs/BenchmarkResults.md`](docs/BenchmarkResults.md) and the human-readable summary is in [`local/cross-orm-bench/BENCHMARKS.log`](local/cross-orm-bench/BENCHMARKS.log). Highlights:

| Operation | ruprizzle (sqlx) | ruprizzle (rusqlite) | fastest comparison |
|---|---|---|---|
| `select_by_pk` | 25.1 µs | **3.1 µs** | Diesel 9.9 µs, Drizzle 39.0 µs, Prisma 173.1 µs |
| `find_many_1000` | 1,634.4 µs | **386.3 µs** | Diesel 305.4 µs, Drizzle 409.9 µs, Sea-ORM 1,559.1 µs |
| `include_posts` | 21,139.9 µs | **7,553.3 µs** | Diesel 3,627.0 µs, prax 10,741.2 µs, Sea-ORM 20,856.5 µs |
| `bulk_insert_1000` | 1,912.4 µs | 1,383.1 µs | **prax 1,059.0 µs**, Drizzle 9,069.6 µs, Sea-ORM 6,027.3 µs |

The `rusqlite` backend swaps the SQLite driver from `sqlx::Any` to the synchronous native `rusqlite` crate and is enabled with the `sqlite-rusqlite` feature. Postgres still uses `sqlx` in both variants. For the Postgres-vs-sqlx overhead report and the `sqlx::Any` text-marshalling note, see [docs/performance.md](docs/performance.md). Generated-crate compile-time benchmarks are automated via `cargo xtask bench-compile`.

---

## Status and roadmap

`1.0.0-rc.1` is **published on crates.io** (2026-08-21, tag `v1.0.0-rc.1`); all ten publishable crates are live at that version. P0–P8 and W0–W5 are complete, including LSP and compile-time query checking; the W4-02 48-hour `rusqlite` soak is **waived** after 15.56 h / 1.46 B ops / 0 errors. The public API has been reviewed and is frozen for the 1.0 line. The remaining work before a stable `1.0.0` is release-process only:

- ~~Publish `1.0.0-rc.1` to crates.io~~ **done 2026-08-21**; the real feedback window is now running (`PathToStableV1.md` W6-04).
- Re-run production-readiness assessment against the RC and reach ≥ 92/100 (W6-05).
- Exercise the automated release workflow end-to-end.
- ~~Complete the clean 48-hour soak test (W4-02) after resolving the SQLite `rusqlite` lock-contention issue documented in `docs/SoakReport.md`.~~ **Waived** after 15.56 h / 1.46 B ops / 0 errors.

Long-term deferrals (v1.2+) such as implicit many-to-many join tables, full-text search, PostGIS, soft deletes, and polymorphic relations are documented in `docs/KnownLimitations.md`.

See the [implementation plan](ProjectPlan/ImplementationPlan/MasterPlan.md), the [production-readiness plan](ProjectPlan/ProductionReadinessPlan.md), and the [decisions log](ProjectPlan/ImplementationPlan/ImplPlan10AppendixDecisions.md) for the full phase-by-phase state, production assessment, and ADRs.

---

## Known limitations

This is an honest beta. The boundaries are documented so you can decide whether ruprizzle is right for your project today.

- **Heuristic renames** are suggested automatically; add `@renamedFrom` to confirm a data-preserving rename. The diff never guesses silently.
- **`db push`** does not write migration files and is only for prototyping.
- **LSP for `schema.ruprizzle`** is available via `ruprizzle-lsp` and the VS Code
  extension in `editor/`. Syntax highlighting is also available as a TextMate
  grammar.
- **Offline query checking** (`ruprizzle check`) is available using query
  manifests captured at test time.
- **`Decimal` on SQLite** is stored as text by the default `sqlx::Any` path.
  The `sqlite-rusqlite` feature parses it back from text at decode time. If
  you need exact decimal math on SQLite, use `Int` minor units or a PostgreSQL
  backend.
- **SQLite `Json`** is stored as TEXT, but JSON1 `json_extract`, `json_type`,
  and `json_set` are supported; the `sqlite-rusqlite` feature also decodes `Json`
  without the `sqlx::Any` text round-trip. JSON containment (`@>`) is
  approximated because JSON1 has no containment operator.
- **Polymorphic relations, recursive tree loading beyond the current depth-limited
  `include`, soft deletes, full-text search, and PostGIS types** are deferred to
  0.2+.

See [docs/KnownLimitations.md](docs/KnownLimitations.md) for the full list and [docs/MigratingFrom.md](docs/MigratingFrom.md) for cheat-sheets when moving from Diesel, SeaORM, or sqlx.

---

## Development

```bash
docker compose up -d      # Postgres for the integration suite
cargo xtask ci            # everything CI runs: fmt, clippy, test, docs
```

Without Docker, `cargo test` still passes: the Postgres half of each dual-database test skips with a printed notice. CI sets `RUPRIZZLE_REQUIRE_DB=1`, which turns that skip into a failure, so the skip can never hide real breakage.

- [Contributing](CONTRIBUTING.md) — how to build, test, and what CI enforces
- [Security policy](SECURITY.md) — how to report a vulnerability
- [Changelog](CHANGELOG.md)

---

## Planning documents

- [MasterPlan](ProjectPlan/ImplementationPlan/MasterPlan.md) — scope, timeline, progress tracker
- [Decisions and risks](ProjectPlan/ImplementationPlan/ImplPlan10AppendixDecisions.md) — ADRs, kill criteria, what is deferred to 0.2
- [Release notes](RELEASES.md) — what changed in each published version
- [Changelog](CHANGELOG.md) — the full, sectioned changelog

---

## Changelog

See [CHANGELOG.md](CHANGELOG.md) for the full, versioned list of additions, changes, fixes, and security notes.

---

## Licence

Dual-licensed under [MIT](LICENSE-MIT) or [Apache 2.0](LICENSE-APACHE), at your option.
