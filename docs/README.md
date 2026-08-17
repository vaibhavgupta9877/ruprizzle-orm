# ruprizzle

[![Crates.io](https://img.shields.io/crates/v/ruprizzle.svg)](https://crates.io/crates/ruprizzle)
[![docs.rs](https://docs.rs/ruprizzle/badge.svg)](https://docs.rs/ruprizzle)
[![MSRV](https://img.shields.io/badge/MSRV-1.85-blue.svg)](../Cargo.toml)
[![License](https://img.shields.io/crates/l/ruprizzle.svg)](../LICENSE-MIT)
[![CI](https://img.shields.io/badge/CI-cargo%20xtask%20ci-success)](../xtask/src/main.rs)

A schema-first ORM for Rust that combines the best parts of Prisma and Drizzle:

- **Prisma's** declarative schema as the single source of truth, with a generated
  typed client, automatic migration diffing, and nested relation loading.
- **Drizzle's** SQL transparency — no hidden query engine, no sidecar binary, and
  `.to_sql()` on every builder so you always know what is being sent to the
  database.

Postgres and SQLite are supported from day one behind a dialect trait, so more
backends are additive. Built on [`sqlx`](https://github.com/launchbadge/sqlx) for
the wire protocol and pooling; we do not write a driver.

## Status

`0.1.1-beta.1` is published on crates.io. The core P0–P8 implementation is
complete and the public API is now stabilising; the production-readiness assessment
has been refreshed for `0.1.1-beta.1`. See the
[implementation plan](../ProjectPlan/ImplementationPlan/MasterPlan.md) for the phase state
and the [production-readiness plan](../ProjectPlan/ProductionReadinessPlan.md) for the
assessment.

## Quick example

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
  posts     Post[]
  createdAt DateTime @default(now()) @map("created_at")

  @@map("users")
}

model Post {
  id       Uuid   @id @default(uuid7())
  title    String
  authorId Uuid   @map("author_id")
  author   User   @relation(fields: [authorId], references: [id], onDelete: Cascade)

  @@map("posts")
}
```

```rust
// Drizzle flavour: the call shape mirrors the SQL.
let admins = db
    .user()
    .find_many()
    .filter(user::EMAIL.ends_with("@acme.com"))
    .order_by(user::CREATED_AT.desc())
    .limit(20)
    .fetch_all()
    .await?;

// Prisma flavour: relation-aware, one query per level — never N+1.
let users = db
    .user()
    .find_many()
    .include(user::posts().filter(post::PUBLISHED.eq(true)).take(5))
    .fetch_all()
    .await?;
```

Wrong-typed and cross-model filters are compile errors, not runtime ones:

```rust
user::EMAIL.eq(42)                              // error: expected String, found i32
db.post().find_many().filter(user::EMAIL.eq(""))  // error: expected Filter<Post>, found Filter<User>
```

## Install

```bash
cargo install ruprizzle-cli    # the `ruprizzle` command
cargo add ruprizzle            # the runtime crate your app uses
```

In a new or existing project:

```bash
ruprizzle init --provider postgres
# Edit schema.ruprizzle, then:
ruprizzle migrate dev --name init
```

Add the generated module to `src/lib.rs` or `src/main.rs`:

```rust
mod db;
```

## Workflow

| Step | Command |
|---|---|
| Scaffold a project | `ruprizzle init --provider postgres\|sqlite` |
| Generate the client | `ruprizzle generate` |
| Auto-watch in dev | `ruprizzle generate --watch` |
| Create & apply a migration | `ruprizzle migrate dev --name <name>` |
| Apply migrations in CI/prod | `ruprizzle migrate deploy` |
| Validate for CI | `ruprizzle validate` |

`migrate dev` and `migrate deploy` are deliberately separate: the production
command never diffs or writes migration files, so habit cannot carry a dangerous
prototyping invocation into CI.

## Why another Rust ORM?

| Feature | ruprizzle | Diesel | SeaORM | sqlx |
|---|---|---|---|---|
| Schema-first code generation | ✅ | partial | ❌ | ❌ |
| Type-safe nested `include` | ✅ | ❌ | partial | ❌ |
| SQL-first query API | ✅ | ❌ | ❌ | ✅ |
| Migrations from schema diff | ✅ | ❌ | partial | ❌ |
| Compile-time query checking | planned | ✅ | ❌ | ✅ |

The trade-off is intentional: ruprizzle targets teams that want a single source
of truth in the schema file, compile-time type safety across relations, and the
ability to drop down to raw SQL without leaving the query builder.

## Repository layout

| Crate | Role | Phase |
|---|---|---|
| `crates/core`    | IR, spans, diagnostics | ✅ P0 |
| `crates/parser`  | Schema DSL → validated IR | ✅ P1 |
| `crates/dialect` | `DbDialect` trait, Postgres + SQLite | ✅ P2 |
| `crates/codegen` | IR → Rust source | ✅ P3 |
| `crates/runtime` | `ruprizzle`, the crate your app depends on | ✅ P4 |
| `crates/migrate` | Snapshot, diff, plan, apply | ✅ P6 |
| `crates/cli`     | The `ruprizzle` binary | ✅ P7 |
| `crates/testkit` | Dual-database test harness | ✅ P0 |
