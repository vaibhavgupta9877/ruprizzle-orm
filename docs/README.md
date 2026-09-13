# ruprizzle

[![Crates.io](https://img.shields.io/crates/v/ruprizzle.svg)](https://crates.io/crates/ruprizzle)
[![docs.rs](https://docs.rs/ruprizzle/badge.svg)](https://docs.rs/ruprizzle)
[![MSRV](https://img.shields.io/badge/MSRV-1.85-blue.svg)](https://github.com/vaibhavgupta9877/ruprizzle-orm/blob/main/Cargo.toml)
[![License](https://img.shields.io/crates/l/ruprizzle.svg)](https://github.com/vaibhavgupta9877/ruprizzle-orm/blob/main/LICENSE-MIT)
[![CI](https://img.shields.io/badge/CI-cargo%20xtask%20ci-success)](https://github.com/vaibhavgupta9877/ruprizzle-orm/actions/workflows/ci.yml)

A schema-first ORM for Rust that combines the best parts of Prisma and Drizzle:

- **Prisma's** declarative schema as the single source of truth, with a generated
  typed client, automatic migration diffing, and nested relation loading.
- **Drizzle's** SQL transparency — no hidden query engine, no sidecar binary, and
  `.to_sql()` on every builder so you always know what is being sent to the
  database.

PostgreSQL, MySQL/MariaDB, and SQLite 3+ are supported behind a dialect trait, so more
backends are additive. Built on [`sqlx`](https://github.com/launchbadge/sqlx) for
the wire protocol and pooling; we do not write a driver. Native driver features
are also available: `sqlite-rusqlite` for synchronous SQLite and an experimental
`postgres-tokio-postgres` for PostgreSQL.

## Status

**The only version available on crates.io is `1.0.0-rc.1`**, for every crate in the
workspace. `ruprizzle-turso` and `ruprizzle-d1` have never been published at all.
Run `scripts/check-release-state.sh` to confirm this against the registry rather
than taking this page's word for it.

`1.0.1` and `1.5.0` are git tags, not releases. The `v1.5.0` publish run failed in
the pre-publish gate and uploaded nothing; the tag is kept as evidence of the
attempt rather than moved. Until a publish run succeeds, treat `1.0.0-rc.1` as the
only installable version and build from source for anything newer.

**Upgrading from `1.0.0-rc.1`?** Most applications need no code changes. See
[Upgrading from 1.0.0-rc.1 to 1.5.1](UpgradingFromRc1.md), and pin
`"=1.0.0-rc.1"` until you are ready, because `"1.0.0-rc.1"` also matches `1.5.1`.

The core P0–P8 implementation is complete and MySQL/MariaDB support is shipped.
The public API is covered by semantic versioning from the first stable release
onward — which has not happened yet.

The v1.1–v1.5 feature line is complete in the repository as `1.5.0` — array
filters, full-text search, soft deletes, offline query checking, nested writes,
tree hierarchies, OpenTelemetry, read-replica routing, query caching, PostGIS and
Ruprizzle Studio. See [What's new in v1.1–v1.5](WhatsNewV1_1ToV1_5.md). It was
assessed and blocked; the current blockers and the remediation plan are in
[`ProjectPlan/ProductionReadinessSolPlan.md`](https://github.com/vaibhavgupta9877/ruprizzle-orm/blob/main/ProjectPlan/ProductionReadinessSolPlan.md).

Two things were waived on the way here, both in writing rather than by omission:
the W4-02 48-hour `rusqlite` soak, accepted on 15.56 h / 1.46 B ops / 0 errors
(see [SoakReport.md](SoakReport.md)), and the two-week RC feedback window, because
no external consumer existed to provide the feedback (see
[Stability](Stability.md#waiver-the-100-rc1-feedback-window-2026-08-21)).

See [Stability](Stability.md) for the semver policy and the public-dependency list
— notably, the 1.0 line is pinned to `sqlx 0.8` — and
[Known limitations](KnownLimitations.md) for deliberate boundaries.

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
cargo install ruprizzle-cli --version 1.0.0-rc.1  # the `ruprizzle` command
cargo add ruprizzle@1.0.0-rc.1                   # the runtime crate your app uses
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
| Scaffold a project | `ruprizzle init --provider postgres\|postgresql\|sqlite\|mysql\|mariadb` |
| Generate the client | `ruprizzle generate` |
| Auto-watch in dev | `ruprizzle generate --watch` |
| Create & apply a migration | `ruprizzle migrate dev --name <name>` |
| Apply migrations in CI/prod | `ruprizzle migrate deploy` |
| Check migration status | `ruprizzle migrate status` |
| Mark a migration applied | `ruprizzle migrate resolve <id>` |
| Reset and replay migrations | `ruprizzle migrate reset --force` |
| Squash migration history | `ruprizzle migrate squash --force` |
| Validate for CI | `ruprizzle validate` |
| Canonicalise schema | `ruprizzle format` |
| Introspect an existing database | `ruprizzle db pull` |
| Seed fixture data | `ruprizzle db seed` |
| Prototype schema push | `ruprizzle db push` |
| Run the language server | `ruprizzle lsp` |
| Offline query check | `ruprizzle check --manifest <path>` |

`migrate dev` and `migrate deploy` are deliberately separate: the production
command never diffs or writes migration files, so habit cannot carry a dangerous
prototyping invocation into CI.

## Why another Rust ORM?

| Feature | ruprizzle | Diesel | SeaORM | sqlx | prax | Prisma | Drizzle |
|---|---|---|---|---|---|---|---|
| Schema-first code generation | ✅ | partial | ❌ | ❌ | ✅ | ✅ | ❌ |
| Type-safe nested `include` | ✅ | ❌ | partial | ❌ | ✅ | ✅ | ✅ |
| SQL-first query API | ✅ | ❌ | ❌ | ✅ | partial | partial | ✅ |
| Migrations from schema diff | ✅ | ❌ | partial | ❌ | ✅ | ✅ | partial |
| Compile-time query checking | ✅ | ✅ | ❌ | ✅ | ✅ | N/A | ❌ |
| Native driver backends (no `sqlx::Any`) | ✅ | ✅ | ❌ | N/A | ✅ | ❌ | ❌ |
| Advanced SQL (CTEs, subqueries, set ops) | ✅ | partial | partial | ✅ | partial | partial | partial |

The trade-off is intentional: ruprizzle targets teams that want a single source
of truth in the schema file, compile-time type safety across relations, and the
ability to drop down to raw SQL without leaving the query builder.

## Repository layout

| Crate | Role | Phase |
|---|---|---|
| `crates/core`    | IR, spans, diagnostics | ✅ P0 |
| `crates/parser`  | Schema DSL → validated IR | ✅ P1 |
| `crates/dialect` | `DbDialect` trait, Postgres + MySQL + SQLite | ✅ P2 |
| `crates/codegen` | IR → Rust source | ✅ P3 |
| `crates/runtime` | `ruprizzle`, the crate your app depends on | ✅ P4 |
| `crates/migrate` | Snapshot, diff, plan, apply | ✅ P6 |
| `crates/cli`     | The `ruprizzle` binary | ✅ P7 |
| `crates/lsp`     | Language server for `schema.ruprizzle` | ✅ P8 |
| `crates/check`   | Offline / compile-time query checking | ✅ P8 |
| `crates/testkit` | Dual-database test harness | ✅ P0 |
