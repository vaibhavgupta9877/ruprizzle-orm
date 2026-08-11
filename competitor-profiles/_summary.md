# Rust ORM Competitive Landscape Summary

*Generated: 2026-08-11*  
*Scope: schema-first and new-generation Rust ORMs, plus established alternatives.*

## Executive takeaways

1. **A crowded wave of new ORMs.** At least 15 new Rust ORMs have appeared since 2024. Most pitch "Prisma-inspired" or "Drizzle-inspired" and use `sqlx` underneath.
2. **The winner is not decided.** Most projects are single-contributor, <100 GitHub stars, and in alpha. The project that ships consistently and builds community has the best chance.
3. **`toasty` is the awareness leader.** It is backed by tokio-rs and has 2,600+ stars. It is derive-macro based and targets SQL + NoSQL, so it is not a direct schema-first competitor, but it will soak up attention.
4. **`ferriorm` is the closest feature-for-feature clone.** Same workspace split, same Postgres/SQLite, schema-first, auto migrations, `sqlx` runtime. It is further ahead on some CRUD surface (aggregates, group-by, upsert, shadow DB).
5. **`prax-orm` is the overreach competitor.** Claims a huge matrix of databases (MongoDB, DuckDB, ScyllaDB, vector search, multi-tenancy, caching) but has only one contributor and 16 GitHub stars. It is unlikely to be credible in production soon.
6. **`vitrail` and `saola` are narrow threats.** Vitrail has a Cloudflare D1 wedge; Saola leverages real Prisma engines (but therefore has the sidecar ruprizzle rejects).
7. **Established tools are still the default.** Diesel, SeaORM, and sqlx dominate. New projects must give teams a reason to switch.

---

## Feature matrix (new-generation ORMs)

| Project | Repository | GH stars | First seen | Main crate downloads | DBs | Schema model | SQL visible | Migrations | Relations | Sidecar | Notes |
|---------|------------|----------|------------|----------------------|-----|--------------|-------------|------------|-----------|---------|-------|
| **ruprizzle** | vaibhavgupta9877/ruprizzle-orm | 0 | Aug 2026 | new | Postgres, SQLite | `schema.ruprizzle` (Prisma DSL) | ✅ `.to_sql()` | ✅ auto-diff | ✅ bounded `include` | ❌ | Alpha, pure Rust, build-time codegen |
| ferriorm | romanschejbal/ferriorm | 2 | Apr 2026 | ~305 | Postgres, SQLite | `schema.ferriorm` | partial | ✅ shadow-DB diff | ✅ | ❌ | 1 contributor; strong CRUD surface |
| prax-orm | quinnjr/prax | 16 | Dec 2025 | ~452 | Postgres, MySQL, SQLite, MSSQL, MongoDB, DuckDB, Scylla | `.prax` schema | partial | ✅ | ✅ | ❌ | Very broad, WIP, 1 contributor |
| vitrail | xJonathanLEI/vitrail | 1 | Mar 2026 | ~490 (pg) | Postgres, SQLite, Cloudflare D1 | `schema!` macro | partial | ✅ | ✅ | ❌ | D1 wedge, early |
| saola | saola-rs/saola | 1 | Apr 2026 | ~81 (core) | Postgres, MySQL, SQLite, MSSQL, MongoDB, Cockroach | `schema.prisma` | no | ✅ | ✅ | ✅ Prisma engine | Uses Prisma engines/PSL |
| kosame | kosame-orm/kosame | 89 | Sep 2025 | ~892 | Postgres only | `pg_table!` macro | no | planned | ✅ | ❌ | No build step, prototype |
| drizzle-rs | themixednuts/drizzle-rs | 41 | Jul 2025 | ~579 | SQLite, libsql, Turso, Postgres | Rust DSL | yes | manual + build.rs | partial | ❌ | Drizzle clone, git-only |
| tiny-orm | MattDelac/tiny_orm | 53 | Oct 2024 | ~8,768 | Postgres, MySQL, SQLite | derive macro | no | ❌ | ❌ | ❌ | CRUD-only, minimal |
| taitan-orm | thegenius/taitan-orm | 124 | Jan 2025 | ~9,057 | ? (sqlx) | sqlx-based | partial | ? | ? | ❌ | Active, Chinese community |
| georm | Phundrak/georm | 11 | Jan 2025 | ~1,995 | Postgres | derive macro | no | ❌ | ✅ | ❌ | SQLx ORM, Postgres-only |
| lume | Guru901/Lume | 3 | Aug 2025 | ~5,611 | MySQL, Postgres, SQLite | ? | partial | ? | ? | ❌ | Drizzle-inspired query builder |
| toasty | tokio-rs/toasty | 2,654 | Oct 2024 | new on crates.io | Postgres, MySQL, SQLite, Turso, DynamoDB | derive macro (`#[Model]`) | partial | ✅ | ✅ | ❌ | tokio-rs brand; SQL+NoSQL |

## Established alternatives

| Project | Repository | GH stars | Positioning | Strengths | Weaknesses vs. ruprizzle |
|---------|------------|----------|-------------|-----------|--------------------------|
| Diesel | diesel-rs/diesel | 14,071 | Sync-first, type-safe query DSL | Mature, fast, strong types, no hidden engine | Synchronous core; hand-written migrations; macro-heavy compile overhead |
| SeaORM | SeaQL/sea-orm | 9,856 | Async ActiveRecord | Large community, async, many integrations | Schema not source of truth; limited joins; `sqlx` SemVer breaks; no SQL transparency |
| sqlx | launchbadge/sqlx | 17,122 | Compile-time checked raw SQL | Massive ecosystem, raw SQL control, no magic | No query builder; dynamic queries are painful; unnameable macro types |
| Prisma Client Rust | prisma-client-rust | — | Prisma engine + Rust client | Feature parity with Prisma | Requires Node sidecar / Rust query engine; heavy runtime |

## Strategic implications for ruprizzle

1. **Differentiation must be concrete, not just branding.** "Schema-first" and "Prisma-inspired" are now table stakes. ruprizzle's concrete differentiators are: `.to_sql()` on every builder, bounded `include`, build-time-only parser/codegen, additive `DbDialect` model, and no sidecar.

2. **Move faster and more transparently than `ferriorm`.** ferriorm has the same pitch and more CRUD features. ruprizzle must win on SQL transparency, community, and release cadence.

3. **Do not try to out-feature `prax-orm`.** prax-orm is too broad. Let it overreach. ruprizzle should stay focused on Postgres/SQLite/MySQL, high-quality codegen, and clean migrations.

4. **Use `toasty` as a foil, not a target.** toasty has massive awareness but a different model (derive macro, SQL+NoSQL). ruprizzle should say: "If you want a schema file, a generated client, and transparent SQL, we are the choice."

5. **Own the SQLx upgrade-safety story.** Multiple ORMs have been broken by `sqlx` SemVer-exempt core API changes. ruprizzle should stress that it consumes only the stable public `sqlx` crate and keeps the runtime thin.

6. **Build trust before features.** In a market of single-contributor alpha projects, the winner will be the one that documents limitations, ships reliably, and grows contributors. GitHub stars and community activity matter more than download counts.

---

*Source data: GitHub public stats, crates.io listings via web search, lib.rs, docs.rs, project READMEs, and Hacker News / Reddit discussions. Stars and downloads are snapshots and will change daily.*
