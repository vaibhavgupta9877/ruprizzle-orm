# Product Marketing Context: ruprizzle-orm

*Last updated: 2026-08-11*

## Product Overview

**One-liner:** A schema-first ORM for Rust that combines Prisma's declarative schema with Drizzle's SQL transparency — pure Rust, no sidecar, built on `sqlx`.

**What it does:** `ruprizzle` lets you write a single `schema.ruprizzle` file and generates a type-safe Rust client, automatic migration diffs, and nested relation loaders. Every query builder exposes `.to_sql()`, so you always see the SQL that will run. The parser and code generator live in build-time crates; only a thin `sqlx`-based runtime ships with your application.

**Product category:** Rust database access layer / ORM (competes with Diesel, SeaORM, sqlx, and a wave of new schema-first ORMs).

**Product type:** Open-source Rust library + CLI (`ruprizzle`, `ruprizzle-cli`).

**Business model:** Free, MIT/Apache-2.0 dual-licensed. Future optional commercial support, managed-cloud migrations, or IDE tooling are possible but not current.

## Target Audience

**Target companies:**
- Seed/Series A startups choosing Rust for a new backend.
- Developer-tool or SaaS companies with existing Rust services that need predictable database access.
- Agencies and indie hackers moving from TypeScript/Node to Rust.

**Decision-makers / users:**
- Tech lead / staff engineer deciding on the DB stack.
- Solo founder writing the first backend.
- Backend engineer migrating an MVP from SeaORM, Diesel, or hand-written SQLx.

**Primary use case:** Build async Rust web/API backends against PostgreSQL and SQLite without hand-writing SQL for CRUD, migrations, and relation loading.

**Jobs to be done:**
1. "I want a single source of truth for my schema; I don't want model files and migration files to drift."
2. "I want the compiler to catch database mistakes, but I also want to see the SQL I'm sending."
3. "I want nested relations loaded in a bounded number of queries, not N+1s."
4. "I want to add a new database backend without rewriting my application code."

**Use cases / scenarios:**
- Prototyping a CRUD API (Axum/Actix) with Postgres.
- SQLite-backed CLI tools or desktop apps that still need type-safe queries.
- Teams migrating from Prisma Client Rust who want to eliminate the Node sidecar.
- Projects where `sqlx` macros have become too verbose for dynamic queries.

## Personas

| Persona | Role | Cares about | Challenge | Value we promise |
|---------|------|-------------|-----------|------------------|
| **Solo Founder Sam** | Full-stack founder, Rust beginner-to-intermediate | Velocity, simple onboarding, no hidden magic | Spends too much time fighting boilerplate and migration drift | Schema-first codegen that just works; `.to_sql()` for confidence |
| **Backend Lead Lin** | Staff/principal engineer in a Rust team | Compile-time safety, performance, observability, vendor risk | Diesel DSL is sync; SeaORM is too opinionated; SQLx is too low-level for most CRUD | Type-safe, async, transparent SQL with an escape hatch to raw SQL |
| **TypeScript Convert Tom** | Backend dev moving from Node/Prisma or Drizzle | Familiar mental model, schema as source of truth | Rust ORMs feel alien or require too much manual boilerplate | Prisma/Drizzle-like ergonomics with no sidecar binary |
| **Systems Engineer Priya** | Rust veteran, performance sensitive | No hidden runtime, no WASM engines, minimal dependency graph | Existing tools pull in query engines or heavy runtimes | Thin runtime on `sqlx`; parser/codegen are build-time only |

## Problems & Pain Points

**Core problem:** Rust developers currently have to choose between:
1. Macro-heavy DSLs that hide SQL and compile slowly (Diesel).
2. String-based SQL helpers that are transparent but not type-safe and require `FromRow` boilerplate (sqlx).
3. Active-record style ORMs where the schema is not the source of truth and generated code drifts (SeaORM).

**Why alternatives fall short for this audience:**
- **Diesel:** synchronous at core, steep type-level DSL, hand-written migrations, intimidating compiler errors.
- **SeaORM:** dynamic query builder, ActiveRecord ergonomics, but the schema is not the source of truth, multi-join queries are awkward, and `sqlx` SemVer-exempt breaks have caused real outages.
- **sqlx:** excellent compile-time checked raw SQL, but dynamic/conditional queries are painful and the macro-generated result types are unnameable.
- **Prisma Client Rust:** closest philosophy but requires a Rust query engine and Node sidecar for migrations.
- **New schema-first clones (ferriorm, prax, saola, vitrail, etc.):** most are prototypes or single-contributor projects, several are not yet production-ready, and many copy Prisma's feature surface without Drizzle's SQL transparency.

**What it costs them:**
- Time: hours hand-syncing schema, models, and migrations.
- Bugs: runtime type mismatches and N+1 query bugs shipped to production.
- Cognitive load: explaining the ORM's magic to juniors and maintaining fragile query builders.
- CI time: heavy macro expansions and runtime dependency bloat.

**Emotional tension:**
- Frustrated that "the right abstraction" for Rust DB access doesn't seem to exist.
- Anxious about picking an ORM that will be abandoned or break on `sqlx` updates.
- Wary of magic — they want to trust the SQL without becoming a DBA.

## Competitive Landscape

**Direct competitors (same approach: schema-first + codegen + SQLx runtime):**
- **ferriorm** — 2 GitHub stars, 1 contributor, 6-crate workspace, `.ferriorm` schema, Postgres/SQLite, auto migrations, shadow DB, aggregates/upsert/group-by. *Threat: closest feature-for-feature clone, further along on CRUD surface, more active releases.*
- **prax-orm** — 16 GitHub stars, 452 crates.io downloads, very broad feature set (multi-DB including DuckDB, MongoDB, Scylla; vector search; multi-tenancy; caching). *Threat: ambitious scope may attract attention, but likely overreaching and single-contributor.*
- **vitrail** — 1 star, 490 `vitrail-pg` downloads, Prisma/Drizzle-inspired, Postgres/SQLite/Cloudflare D1. *Threat: D1 support is a wedge, but extremely early and low traction.*
- **saola** — 1 star, 81 `saola-core` downloads, uses actual Prisma engines/PSL. *Threat: can claim "battle-tested engine"; counter: still pulls in a sidecar/Prisma engine, which ruprizzle rejects.*
- **kosame** — 89 GitHub stars, macro-based, no build step, Postgres only, prototype. *Threat: highest star count in this cohort; counter: not schema-file-first, no migrations yet, not production.*
- **drizzle-rs** — 41 GitHub stars, 579 downloads, Drizzle clone, schema as Rust DSL. *Threat: familiar Drizzle naming; counter: still git-only, sync drivers, not a schema-first generator.*
- **rorm / taitan-orm / georm / lume / tiny-orm / elif-orm / zino-orm** — newer, smaller, or framework-coupled; not yet in the same conversation but signal a crowded space.

**Secondary competitors (mature, different approach):**
- **Diesel** — 14k stars, sync-by-default, mature. Missing: schema-first, auto migrations, native async.
- **SeaORM** — 9.8k stars, async, ActiveRecord. Missing: schema-first source of truth, SQL transparency.
- **sqlx** — 17k stars, raw SQL compile-time checking. Missing: query builder, schema-driven codegen.
- **Prisma Client Rust** — Prisma engine sidecar. Missing: pure Rust, no sidecar.
- **rbatis** — 2.5k stars, compile-time code generation, multiple databases. Missing: schema-first DSL, community momentum outside China.

**Indirect competitors:**
- Hand-written SQL with `tokio-postgres` / `rusqlite`.
- Choosing Go/TypeScript/Python for the backend because the Rust DB story feels immature.
- Sticking with an older ORM and patching around it.

## Differentiation

**Key differentiators:**
1. **Prisma schema + Drizzle transparency, but in pure Rust.** No sidecar binary, no WASM query engine, no Node dependency.
2. **SQL-first transparency on every builder.** `.to_sql()` is a first-class method, not an afterthought.
3. **Generated type-safe column tokens.** `user::EMAIL.eq(42)` is a compile error because the column token carries the Rust type.
4. **Bounded nested `include`.** Two-level includes issue at most one query per level; the bound is tested, not asserted by inspection.
5. **Build-time codegen, thin runtime.** Parser and codegen crates never enter the user's runtime graph; `ruprizzle` runtime only depends on `sqlx`, `serde`, `chrono`, `uuid`, `rust_decimal`.
6. **Additive dialect model.** PostgreSQL and SQLite from day one; adding MySQL means implementing `DbDialect`, not rewriting the runtime.
7. **Honest alpha posture.** Known limitations are published, `migrate dev` and `migrate deploy` are deliberately separated, and the public API is stabilizing toward 0.2.

**How we do it differently:**
- Schema is the single source of truth (`schema.ruprizzle`); you do not hand-edit generated Rust.
- The query builder is a transparent DSL that mirrors SQL, with a `raw!` macro and `sqlx::query!` interop as first-class escape hatches.
- Dialect capabilities are explicit in the generator, so unsupported features fail at build time, not at runtime.

**Why that's better:**
- Teams get the productivity of code generation and the confidence of compile-time types without the runtime bloat or hidden engines of Prisma-style tools.
- They can reason about SQL and optimize it; they are not at the mercy of a magical query planner.
- New database backends and language tooling are additive, protecting their investment.

**Why customers choose us over alternatives:**
- If they want schema-first but the Prisma Client Rust sidecar is a deal-breaker.
- If they like `sqlx` but are drowning in `FromRow`/query composition boilerplate.
- If they like Diesel's types but are tired of hand-written migrations and sync limitations.
- If they like SeaORM's ergonomics but need the schema to be the source of truth.

## Objections & Anti-Personas

| Objection | Response |
|-----------|----------|
| "It's still alpha." | True. `0.1.0-alpha.3` just shipped. P0–P8 are complete and the API is stabilizing. The [Known limitations](#) are published, so teams can make an informed decision. For greenfield MVPs and prototypes, it's ready to try today. |
| "Only Postgres and SQLite are supported." | MySQL/MariaDB is the next `DbDialect` implementation and is additive. Postgres and SQLite cover the majority of new Rust web and CLI projects. |
| "Why not just use ferriorm/prax/kosame?" | They are also early. ruprizzle differentiates on SQL transparency (`.to_sql()`), bounded `include`, and an honest alpha stance. Evaluate them side by side. |
| "One contributor / no community." | We're actively building the project in public. The first milestone is a small but engaged early-adopter cohort before a stable 0.2. |
| "What if `sqlx` breaks the API again?" | We intentionally consume only the stable `sqlx` public crate (currently 0.8) and keep the runtime thin to minimize exposure to `sqlx-core` SemVer-exempt internals. |

**Anti-persona:**
- Teams that need a battle-tested, multi-year production track record today (they should stay on Diesel/sqlx until 0.2+).
- Teams wedded to sync Rust or `async-std` (ruprizzle is tokio-first).
- Users who want a fully dynamic, runtime query builder without codegen (SeaORM or hand-rolled SQLx is a better fit).

## Switching Dynamics

**Push:** Frustration with current tools — Diesel's sync model, SeaORM's drift and `sqlx` breakage, SQLx verbosity, Prisma's sidecar, hidden query engines that make debugging hard.

**Pull:** The promise of a single `schema.ruprizzle` file, generated type-safe client, transparent SQL, and a thin `sqlx` runtime. "Finally a Rust ORM that feels like Prisma's schema but without the magic."

**Habit:** Teams are already invested in Diesel migrations, SeaORM entities, or SQLx query strings. Switching means a schema rewrite and retraining.

**Anxiety:** Will this project survive? Will it compile in CI? Will the alpha limits block us? Will the codegen slow down builds?

## Customer Language

**How they describe the problem:**
- "Writing dynamic/conditional queries just sucks and there isn't any good solution." (Hacker News, sqlx discussion)
- "SQLx is just providing some core foundations, which means yes there is no good query builder or anything like that." (Hacker News)
- "I found Diesel's query building catastrophically bad the moment the query isn't built all in one place." (Hacker News)
- "Sea ORM is too opinionated in my experience. Even making migration is not trivial with their own DSL." (Hacker News)
- "I want to keep my business logic clean and don't want it to know about database or ORM related details." (Rust users forum)
- "Most Rust ORMs require you to manually define structs, derive traits, write migrations, and wire it all together." (ferriorm README, echoed across the category)

**How they describe the ideal solution:**
- "Define your schema once, and everything else is generated."
- "No hidden query engine, no sidecar binary."
- "Type-safe, but I can still see the SQL."
- "Async-first, built on sqlx."

**Words to use:** schema-first, type-safe, SQL transparency, generated client, no sidecar, pure Rust, `sqlx`, migrations, relations, `include`, dialect, `.to_sql()`, compile-time, alpha, honest.

**Words to avoid:** magic, hidden engine, "production-ready" (until 0.2), "zero-config" (there is a CLI and schema to learn), generic superlatives.

**Glossary:**
| Term | Meaning |
|------|---------|
| `schema.ruprizzle` | The declarative schema file (Prisma-style DSL) that is the single source of truth. |
| `Db` client | The generated root database client with one accessor per model. |
| `Column<Model, T>` | A generated, type-safe column token; `user::EMAIL.eq(42)` is a compile error. |
| `include` | Nested relation loader with a bounded number of SQL queries. |
| `DbDialect` | The trait that models database capabilities; new DBs are additive. |
| `migrate dev` / `migrate deploy` | Development migration diffing vs. production-safe application. |

## Brand Voice

**Tone:** Technical, direct, honest, and pragmatic. Confident but not defensive about the alpha.

**Communication style:**
- Show the SQL and the code, not just promises.
- Prefer "why this trade-off" over "we're the best."
- Use concrete benchmarks and known limitations.

**Brand personality (adjectives):** transparent, type-obsessed, schema-first, fast, honest, additive, no-bullshit.

## Proof Points

**Metrics:**
- Query construction (select by PK, no I/O): ~600 ns.
- Query construction (filter + order, no I/O): ~1.8 µs.
- Codegen for a 50-model schema: ~16 ms.
- Runtime crate depends only on `sqlx`, `serde`, `chrono`, `uuid`, `rust_decimal`.

**Customers / adopters:**
- None yet publicly; early-adopter program and case studies are a 90-day priority.

**Testimonials:**
- None yet; collect from first 10 active users.

**Value themes:**
| Theme | Proof |
|-------|-------|
| SQL transparency | `.to_sql()` on every builder; README examples show generated SQL. |
| No sidecar / hidden engine | Pure Rust runtime; parser and codegen are build-time only. |
| Type safety | `user::EMAIL.eq(42)` and `Filter<Post>` vs `Filter<User>` are compile errors. |
| Bounded `include` | Test suite asserts query count per include level. |
| Additive dialects | `DbDialect` trait; Postgres and SQLite implemented, MySQL planned. |

## Goals

**Business goal (12 months):** Establish `ruprizzle` as the leading schema-first ORM for Rust among teams that value SQL transparency, with a stable 0.2 release and a self-sustaining open-source community.

**Key conversion action:** Star and try the project on GitHub; install `ruprizzle-cli`; run the quickstart; report an issue or join the Discord/community chat.

**Current metrics (as of 2026-08-11):**
- GitHub stars: 0.
- crates.io version: 0.1.0-alpha.3 (published today).
- Downloads: not yet significant.
- Contributors: 1 (the author).
- Docs.rs build: passing, 100% item coverage.

---

*Other marketing skills will now use this context automatically. Run `/product-marketing` to update it as the project and market evolve.*
