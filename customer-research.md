# Customer & Market Research: Rust ORM Pain Points

*Generated: 2026-08-11*  
*Sources: Hacker News, Reddit r/rust, Rust users forum, GitHub issues/discussions, project READMEs.*  
*Recency window: 2024–2026.*

## Method

| Source | What it tells us | Bias to weight |
|--------|------------------|----------------|
| Hacker News / Reddit | Raw, unfiltered language; trigger events; switching stories | Skews technical and skeptical; louder voices |
| Rust users forum | Deeper architectural questions; migration scenarios | Smaller sample; power users |
| GitHub issues / discussions | Actual failure modes and feature gaps | Problem-skewed; not representative of happy users |
| Project READMEs | How competitors position themselves | Marketing, not verbatim customer language |

## Top themes (ranked by frequency × intensity)

### Theme 1: sqlx is great until you need a query builder

**Summary:** Developers respect `sqlx` for compile-time checked raw SQL, but dynamic/conditional queries, unnameable macro types, and boilerplate push them away.

**Frequency:** High  
**Intensity:** High  
**Confidence:** High

**Representative quotes:**
- "SQLx is just providing some 'core' foundations, which means yes there is no good query builder or anything like that." — Hacker News
- "SQLx sucks at dynamic queries. Dynamic predicates, WHERE IN clauses, etc." — Hacker News
- "The structs sqlx creates through the macros are unnameable types, they are created on the fly at the invocation site. You can't return them without transforming them into a type you own." — Hacker News
- "Sqlx is completely lacking in the query composability department, and leads to a very large amount of boilerplate." — Hacker News
- "SQLx can handle complicated queries as long as they're completely static strings... SQLx sucks at dynamic queries." — Hacker News

**Implications for ruprizzle:**
- Position as the generated client on top of sqlx that adds a type-safe query builder and migrations.
- Show `.to_sql()` and filter combinators as the answer to composability.
- Emphasize that `raw!` / `sqlx::query!` interop is a first-class escape hatch, not a defeat.

---

### Theme 2: Diesel is powerful but heavy and not async-first

**Summary:** Diesel's type system is admired, but its sync core, macro load, hand-written migrations, and intimidating compiler errors are real friction.

**Frequency:** High  
**Intensity:** Medium-High  
**Confidence:** High

**Representative quotes:**
- "Diesel was ok, but I never use it anymore since rocket moved to async." — Hacker News
- "I found their query building catastrophically bad the moment the query isn't built all in one place." — Hacker News
- "Diesel's macro-heavy approach... compile times that punish rapid iteration." — The Editorial, 2026
- "Diesel is arguably the most mature and widely adopted ORM... The trade-off: compile times that punish rapid iteration." — Rust infinity, 2026

**Implications for ruprizzle:**
- Lead with async-first, tokio-native, and no hand-written migration files.
- Compare compile times where possible; highlight build-time parser/codegen not in runtime graph.
- Stress the schema file as the single source of truth.

---

### Theme 3: SeaORM is too opinionated and its schema is not the source of truth

**Summary:** SeaORM is the default async ORM, but developers hit limited joins, migration DSL friction, entity drift, and `sqlx` breakage.

**Frequency:** High  
**Intensity:** High  
**Confidence:** High

**Representative quotes:**
- "Sea ORM is too opinionated in my experience. Even making migration is not trivial with their own DSL." — Hacker News
- "I tried sea-orm, but I find its ORM API way too limited (it can't even do multiple joins)." — Hacker News
- "Sqlx 0.8.4 breaks sea-orm" — GitHub issue, 2025
- "It is insane that we can't even select three models of different tables with multiple joins by one query." — SeaORM GitHub discussion
- "The underlying query builder's API is just downright odd. The ActiveRecord pattern is fine for SeaORM, but it's just... weird." — Hacker News

**Implications for ruprizzle:**
- Make "schema as the single source of truth" the central pitch.
- Demonstrate multi-level `include` and `.to_sql()` to prove no hidden magic.
- Note the `sqlx-core` SemVer-exempt risk and how ruprizzle stays on the public crate.

---

### Theme 4: The dream is schema-first, zero boilerplate, and clean domain models

**Summary:** There is strong demand for one schema file that generates clients, migrations, and query builders. Developers also want domain structs free of ORM attributes.

**Frequency:** Medium-High  
**Intensity:** High  
**Confidence:** Medium-High

**Representative quotes:**
- "I just want to keep my business logic clean and don't want it to know about database or ORM related details." — Rust users forum
- "Existing Rust ORMs require manually defining structs, writing migrations, and wiring everything together. ferriorm takes the Prisma approach: define your schema once." — ferriorm README
- "Most Rust ORMs require you to manually define structs, derive traits, write migrations, and wire it all together." — ferriorm docs
- "Kosame was born out of a desire to have this level of developer ergonomics in Rust, using macro magic." — Kosame README

**Implications for ruprizzle:**
- "One `schema.ruprizzle` file, one generated client, no derive macros in your app."
- Show the quickstart: `init → edit schema → generate → migrate dev`.
- Compare to entity-first and ActiveRecord as the old way.

---

### Theme 5: No hidden engine / no sidecar is a real advantage

**Summary:** Developers are wary of Prisma's Node sidecar, hidden query engines, and runtime bloat. Pure Rust, thin runtime is valued.

**Frequency:** Medium  
**Intensity:** Medium  
**Confidence:** Medium

**Representative quotes:**
- "No bells and whistles, no Rust binaries, no serverless adapters, everything just works out of the box." — Drizzle marketing
- "Prisma Client Rust ... ships a Rust query engine and uses a Node sidecar for migration generation. ruprizzle is pure Rust with no sidecar binary." — ruprizzle README
- "When the abstraction inevitably leaks, it leaks towards the user using raw SQL." — Hacker News (about Drizzle)

**Implications for ruprizzle:**
- Lead with "no sidecar, no hidden engine, no WASM runtime" in every comparison.
- Explain the workspace split: parser and codegen are build-time only.
- Use `.to_sql()` as proof that there is no hidden query planner.

---

### Theme 6: Migrations should be generated, not hand-written

**Summary:** Teams want schema diffing and automatic migration generation. Manual migration files are seen as a source of drift and errors.

**Frequency:** Medium  
**Intensity:** Medium  
**Confidence:** Medium

**Representative quotes:**
- "ferriorm diffs your schema against the database and generates SQL migrations for you." — ferriorm docs
- "Prisma's declarative schema as the single source of truth, with a generated typed client, automatic migration diffing." — ruprizzle README
- "Automatic migrations — keep your database schema in sync with your models effortlessly." — rusql-alchemy README

**Implications for ruprizzle:**
- Highlight `migrate dev` and the deliberate `migrate deploy` safety split.
- Explain destructive-change prompts and `migrate reset` / `resolve` / `status`.
- Position auto-diff as a first-class feature, not an add-on.

---

### Theme 7: New ORMs are multiplying — skepticism is high

**Summary:** Because many new Rust ORMs appear similar, developers are skeptical about which will survive. Trust signals (community, release cadence, transparency) matter.

**Frequency:** Medium  
**Intensity:** Medium  
**Confidence:** Medium

**Representative quotes:**
- "Very interested in exploring how this will compare to Diesel and SeaORM." — Hacker News (on Toasty)
- "Kosame is currently a prototype and not recommended for production use." — Kosame README
- "This project is still evolving. Expect breaking changes." — drizzle-rs README
- "Work in Progress - Prax is currently under active development." — prax-orm README

**Implications for ruprizzle:**
- Be aggressively honest about the alpha and the roadmap.
- Ship small, frequent releases and publicize them.
- Build a visible community (Discord, GitHub Discussions) before asking for trust.

---

## Customer language cheat sheet

Use these exact phrases and concepts in copy, headlines, comparisons, and README:

- "schema-first"
- "type-safe"
- "no sidecar"
- "no hidden query engine"
- "SQL transparency"
- ".to_sql()"
- "generated client"
- "automatic migrations"
- "no N+1"
- "pure Rust"
- "built on sqlx"
- "honest alpha"
- "single source of truth"
- "bounded include"
- "additive dialects"
- "migrate dev vs migrate deploy"

## Anti-language (avoid)

- "production-ready" (until 0.2+)
- "zero-config" (there is a CLI and schema to learn)
- "magic" (we are anti-magic)
- Generic superlatives: "fastest", "best", "world-class"

## Persona snapshots

| Persona | Trigger event | Top pain | Desired outcome | Where they hang out |
|---------|---------------|----------|-----------------|---------------------|
| **TypeScript convert** | Moving a startup backend from Node to Rust | Rust ORMs feel alien or boilerplate-heavy | Familiar schema-first workflow, no sidecar | Hacker News, Reddit r/rust, Prisma/Drizzle communities |
| **Rust web lead** | Starting a new service; evaluating DB layer | SeaORM limits / Diesel sync / SQLx verbosity | Async, type-safe, migration-capable ORM | TWIR, Rust users forum, GitHub |
| **Indie hacker** | Building an MVP fast | Wants CRUD + migrations without DBA work | One schema file, generated client, quick deploy | Product Hunt, Twitter/X, YouTube |
| **Rust tooling dev** | SQLite + Postgres in a CLI or desktop app | Heavy runtimes or hidden engines | Thin runtime, transparent SQL, multi-backend | lib.rs, docs.rs, Rust meetups |

## Research gaps

1. **No primary research yet.** We have not run a survey or 1:1 interviews with real users. The first 20 people who try ruprizzle should be interviewed.
2. **Conversion funnel unknown.** We do not know the rate at which README visitors become installers and then active users.
3. **Headline resonance untested.** We do not know which message — "no sidecar," "schema-first," or "SQL transparency" — converts best with each persona.
4. **Competitor user sentiment thin.** Most new ORMs have no public user discussion; their README claims may not match reality. Watchlist as they mature.

---

*This document feeds directly into `.agents/product-marketing.md` and `MARKETING_PLAN.md`.*
