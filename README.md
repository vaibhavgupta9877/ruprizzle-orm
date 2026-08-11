# ruprizzle

[![Crates.io](https://img.shields.io/crates/v/ruprizzle.svg)](https://crates.io/crates/ruprizzle)
[![docs.rs](https://docs.rs/ruprizzle/badge.svg)](https://docs.rs/ruprizzle)
[![MSRV](https://img.shields.io/badge/MSRV-1.85-blue.svg)](./Cargo.toml)
[![License](https://img.shields.io/crates/l/ruprizzle.svg)](./LICENSE-MIT)
[![CI](https://img.shields.io/badge/CI-cargo%20xtask%20ci-success)](./xtask/src/main.rs)

A **schema-first ORM for Rust** that combines the best parts of Prisma and Drizzle:

- **Prisma's** declarative schema as the single source of truth, with a generated typed client, automatic migration diffing, and nested relation loading.
- **Drizzle's** SQL transparency — no hidden query engine, no sidecar binary, and `.to_sql()` on every builder so you always know what is being sent to the database.

Postgres and SQLite are supported from day one behind a `DbDialect` trait, so more backends are additive. Built on [`sqlx`](https://github.com/launchbadge/sqlx) for the wire protocol and pooling; ruprizzle does not write its own driver.

> **Status:** `0.1.0-alpha.2` is published on crates.io. The core P0–P8 implementation is complete and the public API is now stabilising. See [Known limitations](#known-limitations) for the honest boundaries of the alpha.

---

## Table of contents

- [Quick example](#quick-example)
- [Why ruprizzle?](#why-ruprizzle)
- [Detailed features](#detailed-features)
- [Installation](#installation)
- [Quickstart](#quickstart)
- [Query examples](#query-examples)
- [CLI workflow](#cli-workflow)
- [Comparison with other Rust ORMs](#comparison-with-other-rust-orms)
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
db.post().select().filter(user::EMAIL.eq(""))  // error: expected Filter<Post>, found Filter<User>
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
- **Dialect differences are explicit.** If Postgres supports something SQLite does not, the generator tells you at build time, not at runtime.

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

- `ruprizzle init --provider postgres|sqlite` — scaffold schema, `.env`, `.gitignore`, and `migrations/`.
- `ruprizzle generate` and `ruprizzle generate --watch` — generate the typed client.
- `ruprizzle validate` — CI-friendly schema validation.
- `ruprizzle format` — canonicalise the schema file.
- `ruprizzle migrate dev|deploy|status|resolve|reset` — see [CLI workflow](#cli-workflow).
- `ruprizzle db push|seed` — direct schema push and seed scripts.

### Dialects

- **Postgres 17+** and **SQLite 3+** support from day one.
- **Dialect capabilities model**: native enums, native UUID, `RETURNING`, `ALTER COLUMN`, window functions, JSON support, partial indexes, and max bind parameters are explicitly modelled.
- **Additive dialect design**: adding MySQL/MariaDB means implementing `DbDialect` and a conformance suite; the runtime does not change.
- **SQLite table rebuilds** for destructive column changes are handled automatically.
- **UUID and JSON** are mapped idiomatically per dialect (`uuid`/`jsonb` on Postgres, text on SQLite where native storage is unavailable).

### Transactions and escape hatches

- **First-class transactions**: `db.begin().await?`, `tx.commit().await?`, `tx.rollback().await?`. All builders work unchanged against a transaction.
- **Isolation levels**: `ReadUncommitted`, `ReadCommitted`, `RepeatableRead`, `Serializable`.
- **Raw SQL execution**: `db.fetch_all_raw(sql, params).await?` and `db.execute_raw(sql, params).await?`.
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
            name: "Alice".into(),
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

See the full [quickstart](docs/Quickstart.md) for a step-by-step walkthrough.

---

## Query examples

### Select

```rust
let users = db
    .user()
    .select()
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
    .select()
    .project(user::NAME)
    .fetch_all()
    .await?;
```

### Insert and upsert

```rust
db.user()
    .insert()
    .set(user::EMAIL, "alice@example.com")
    .set(user::NAME, "Alice")
    .exec()
    .await?;

// Insert or update on conflict
db.user()
    .insert()
    .set(user::EMAIL, "alice@example.com")
    .set(user::NAME, "Alice")
    .on_conflict(["email"])
    .do_update(["name"])
    .exec()
    .await?;
```

### Pagination

```rust
use ruprizzle::Page;

let page = db
    .user()
    .select()
    .paginate(Page::new(1, 20))
    .fetch()
    .await?;

println!("page {} of {}, total {}", page.number, page.total, page.total_rows);
```

### Transactions

```rust
let mut tx = db.begin().await?;

let id = tx.user().insert().set(user::EMAIL, "a@b.c").exec().await?;

if should_commit {
    tx.commit().await?;
} else {
    tx.rollback().await?;
}
```

### Raw SQL

```rust
let rows = db
    .fetch_all_raw(
        "SELECT * FROM users WHERE email LIKE $1".to_owned(),
        vec![Value::from("%@example.com")],
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

## Comparison with other Rust ORMs

| Feature | ruprizzle | Diesel | SeaORM | sqlx | Prisma Client Rust |
|---|---|---|---|---|---|
| **Schema-first code generation** | ✅ | partial | ❌ | ❌ | ✅ |
| **Declarative schema DSL** | ✅ | ❌ | ❌ | ❌ | ✅ |
| **Type-safe column tokens** | ✅ | ✅ | partial | ❌ | ✅ |
| **Type-safe nested `include`** | ✅ | ❌ | partial | ❌ | ✅ |
| **SQL-first, visible query builder** | ✅ | partial | ❌ | ✅ | partial |
| `.to_sql()` on every builder | ✅ | ❌ | ❌ | N/A | partial |
| **Migrations from schema diff** | ✅ | ❌ | partial | ❌ | partial |
| **Compile-time query checking** | planned | ✅ | ❌ | ✅ | N/A |
| **No sidecar / no hidden engine** | ✅ | ✅ | ✅ | ✅ | ❌ |
| **Native async / `await`** | ✅ | partial | ✅ | ✅ | ✅ |
| **Postgres + SQLite** | ✅ | ✅ | ✅ | ✅ | partial |
| **Built on `sqlx`** | ✅ | ❌ | ✅ | N/A | ❌ |

### What this means in practice

- **Diesel** gives you compile-time checked SQL and strong types, but migrations are hand-written and the DSL can feel macro-heavy. ruprizzle keeps the type safety and adds a declarative schema, generated client, and automatic migration diffing.
- **SeaORM** is ergonomic and active-record style, but the schema is not the single source of truth and the generated code can drift. ruprizzle inverts that: edit `schema.ruprizzle`, run `ruprizzle generate`.
- **sqlx** is the most transparent, but you write SQL and `FromRow` by hand. ruprizzle builds on sqlx and gives you a typed client while preserving the escape hatch.
- **Prisma Client Rust** is closest in philosophy, but it ships a Rust query engine and uses a Node sidecar for migration generation. ruprizzle is pure Rust with no sidecar binary.

---

## Architecture and repository layout

The workspace is split so that parser and codegen never enter the user's runtime dependency graph:

| Crate | Role | Ships to users? | Phase |
|---|---|---|---|
| `crates/core` | IR, spans, diagnostics | transitively | P0 |
| `crates/parser` | Schema DSL → validated IR | no (build/CLI only) | P1 |
| `crates/dialect` | `DbDialect` trait, Postgres + SQLite | transitively | P2 |
| `crates/codegen` | IR → Rust source | no | P3 |
| `crates/runtime` (`ruprizzle`) | Query builder, executor, transactions | **yes** | P4 |
| `crates/macros` | `#[derive(FromRow)]` passthrough, `raw!` | **yes** | P4 |
| `crates/migrate` | Snapshot, diff, plan, apply | transitively | P6 |
| `crates/cli` | The `ruprizzle` binary | **yes** | P7 |
| `crates/testkit` | Dual-database test harness | no | P0 |

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

The end-to-end I/O benchmark is `cargo bench -p ruprizzle --bench end_to_end`; for the latest numbers and the text-marshalling note, see [docs/Performance.md](docs/Performance.md). Generated-crate compile-time benchmarks are not yet automated because they require a dedicated compile-time machine.

---

## Status and roadmap

P0–P8 are complete and `0.1.0-alpha.2` is on crates.io. Remaining work before a stable 0.2:

- MySQL / MariaDB dialect (additive via `DbDialect`).
- Many-to-many implicit join tables (explicit join model works today).
- Database introspection → schema (`db pull`).
- Raw-SQL compile-time verification (`sqlx::query!` style).
- Full LSP for the schema DSL.
- Migration squashing and connection pool metrics.

See the [implementation plan](ProjectPlan/ImplementationPlan/MasterPlan.md) and [decisions log](ProjectPlan/ImplementationPlan/ImplPlan10AppendixDecisions.md) for the full phase-by-phase state and ADRs.

---

## Known limitations

This is an honest alpha. The boundaries are documented so you can decide whether ruprizzle is right for your project today.

- **Migrations** do not handle mutual foreign-key cycles automatically. Cycles must be broken by hand across migrations.
- **Heuristic renames** (detecting a column was renamed rather than dropped + added) are not implemented. Use `@renamedFrom` to give the diff an explicit hint.
- **`db push`** does not write migration files and is only for prototyping.
- **Compile-time query checking** (`sqlx-data.json` / offline mode) is not implemented.
- **No LSP** yet; syntax highlighting is available as a TextMate grammar.
- **`Decimal` on SQLite** is stored as text.
- **SQLite `Json`** is stored as text and cannot be queried with JSON operators.
- **Polymorphic relations, recursive loading beyond depth 2, soft deletes, JSON path querying, full-text search, and PostGIS types** are deferred to 0.2+.

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
