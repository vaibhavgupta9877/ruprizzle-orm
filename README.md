# ruprizzle-orm

A schema-first ORM for Rust, taking the best of Prisma and Drizzle:

- **Prisma's** declarative schema as the single source of truth, with a generated
  client, automatic migration diffing, and nested relation loading.
- **Drizzle's** SQL transparency — no hidden query engine, no sidecar binary, and
  `.to_sql()` on every query.

Postgres and SQLite from day one, behind a dialect trait so more are additive.
Built on [`sqlx`](https://github.com/launchbadge/sqlx) for the wire protocol and
pooling; we do not write a driver.

> **Status: pre-alpha, under construction.** The workspace foundation (phase P0)
> is complete. The parser, dialects, codegen, query builder, relations, and
> migration engine are phases P1 through P6 and are **not implemented yet**.
> Nothing here is usable as an ORM today. See the
> [implementation plan](ProjectPlan/ImplementationPlan/MasterPlan.md) for the
> phase-by-phase state.

## What it will look like

```prisma
// schema.ruprizzle
datasource db {
  provider = "postgres"
  url      = env("DATABASE_URL")
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
let admins = db.select::<User>()
    .filter(user::EMAIL.ends_with("@acme.com"))
    .order_by(user::CREATED_AT.desc())
    .limit(20)
    .fetch_all()
    .await?;

// Prisma flavour: relation-aware, one query per level — never N+1.
let users = db.user()
    .find_many()
    .include(user::posts().filter(post::PUBLISHED.eq(true)).take(5))
    .exec()
    .await?;
```

Wrong-typed and cross-model filters are compile errors, not runtime ones:

```rust
user::EMAIL.eq(42)                              // error: expected String, found i32
db.select::<Post>().filter(user::EMAIL.eq(""))  // error: expected Filter<Post>, found Filter<User>
```

## Repository layout

| Crate | Role | Phase |
|---|---|---|
| `crates/core` | IR, spans, diagnostics — the contract every crate speaks | ✅ P0 |
| `crates/parser` | Schema DSL → validated IR | P1 |
| `crates/dialect` | `DbDialect` trait, Postgres + SQLite | P2 |
| `crates/codegen` | IR → Rust source | P3 |
| `crates/runtime` | `ruprizzle`, the crate your app depends on | P4 |
| `crates/migrate` | Snapshot, diff, plan, apply | P6 |
| `crates/cli` | The `ruprizzle` binary | P7 |
| `crates/testkit` | Dual-database test harness | ✅ P0 |

The parser and code generator are **not** in your application's dependency graph.
They run in the CLI, so your builds never compile them.

## Development

```bash
docker compose up -d      # Postgres for the integration suite
cargo xtask ci            # everything CI runs: fmt, clippy, test, docs
```

Without Docker, `cargo test` still passes: the Postgres half of each dual-database
test skips with a printed notice. CI sets `RUPRIZZLE_REQUIRE_DB=1`, which turns
that skip into a failure, so the skip can never hide real breakage.

Integration tests are written once and run against every backend:

```rust
both_dbs! {
    setup = SMOKE_DDL;
    async fn insert_then_select(db: TestDb) {
        db.execute("INSERT INTO widget (id, name, price) VALUES (1, 'bolt', 250)").await?;
        assert_eq!(db.fetch_i64("SELECT count(*) FROM widget").await?, 1);
    }
}
```

## Planning documents

- [MasterPlan](ProjectPlan/ImplementationPlan/MasterPlan.md) — scope, timeline, progress tracker
- [Decisions and risks](ProjectPlan/ImplementationPlan/ImplPlan10AppendixDecisions.md) — ADRs, kill criteria, what is deferred to 0.2

## Licence

Dual-licensed under [MIT](LICENSE-MIT) or [Apache 2.0](LICENSE-APACHE), at your option.
