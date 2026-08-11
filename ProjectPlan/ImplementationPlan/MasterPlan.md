# ruprizzle-orm — Master Plan (v1 MVP)

**What:** A pure-Rust ORM that takes the best of Prisma (declarative schema DSL as
single source of truth, codegen'd client, migration diffing, nested relation
loading, great errors) and Drizzle (SQL-transparent query builder, zero hidden
runtime, "no magic" escape hatches, edge-friendly thin core).

**Scope of this repo:** ORM **only**. No UI components, no auth, no RPC, no
reference app. Those are integrated later from a separate project.

**Team:** Vaibhav Gupta (architecture, specs, review, hard algorithms) + Vaibhav Gupta
(implementation, tests, plumbing).
**Target:** `0.1.0-alpha` on crates.io.
**Timeline:** 6 working weeks (~34 agent-days across two parallel tracks), with a
1-week buffer. See [Timeline](#timeline).

---

## The one-paragraph pitch

You write `schema.ruprizzle`. You run `ruprizzle generate`. You get typed entity
structs, per-model column tokens, a fluent query builder that compiles to exactly
the SQL you expect, and a migration engine that diffs your schema against the last
snapshot and writes the DDL for you. Postgres and SQLite from day one, behind a
`DbDialect` trait so more dialects are additive. Built on `sqlx` for the wire
protocol and pooling — we do not write a driver.

---

## Non-negotiable design principles

1. **Schema is the single source of truth.** Entities, migrations, and the client
   are all derived. Never hand-edit generated code.
2. **No hidden query engine.** No sidecar binary, no WASM engine. The generated
   client is plain Rust calling `sqlx`. (This is the Drizzle lesson; Prisma spent
   years undoing its Rust engine.)
3. **Predictable SQL.** Every builder call maps to a visible SQL fragment. `.to_sql()`
   is available on every query for debugging.
4. **Type errors, not runtime errors.** Column tokens are typed `Column<Model, T>`,
   so `user::email.eq(42)` fails to compile.
5. **Escape hatch always present.** `sqlx::query_as!` interop and `raw()` fragments
   are first-class, not a defeat.
6. **Dialect differences are explicit,** never papered over silently. If Postgres
   supports something SQLite does not, codegen tells you at generate time.

---

## Crate layout (cargo workspace)

| Crate | Role | Ships to users? |
|---|---|---|
| `ruprizzle-core` | IR/AST types, `Span`, diagnostics, shared errors | transitively |
| `ruprizzle-parser` | Pest grammar → AST → validated IR | no (build/CLI only) |
| `ruprizzle-dialect` | `DbDialect` trait + Postgres/SQLite impls | transitively |
| `ruprizzle-codegen` | IR → Rust source (entities, tokens, client) | no |
| `ruprizzle-migrate` | snapshot, diff engine, planner, runner | transitively |
| `ruprizzle` (runtime) | what user apps depend on: executor, filters, builders | **yes** |
| `ruprizzle-macros` | `#[derive(FromRow)]` passthrough, `raw!` | **yes** |
| `ruprizzle-cli` | `ruprizzle` binary | **yes** |

Rationale: parser/codegen never enter the user's dependency graph, so app compile
times stay low. This directly answers the RealityCheck concern about build times.

---

## Plan files

| File | Covers | Phase |
|---|---|---|
| [ImplPlan01Foundation.md](ImplPlan01Foundation.md) | Workspace, core IR, diagnostics, CI, test harness | P0 |
| [ImplPlan02SchemaDslParser.md](ImplPlan02SchemaDslParser.md) | DSL spec, Pest grammar, AST lowering, validation | P1 |
| [ImplPlan03DialectsSqlGen.md](ImplPlan03DialectsSqlGen.md) | `DbDialect` trait, Postgres, SQLite, type mapping | P2 |
| [ImplPlan04CodegenEntities.md](ImplPlan04CodegenEntities.md) | Entity structs, enums, column tokens, client root | P3 |
| [ImplPlan05QueryBuilderRuntime.md](ImplPlan05QueryBuilderRuntime.md) | Select/Insert/Update/Delete, filters, pagination, tx | P4 |
| [ImplPlan06RelationsInclude.md](ImplPlan06RelationsInclude.md) | Relation IR, `include`, batched loading, N+1 defense | P5 |
| [ImplPlan07Migrations.md](ImplPlan07Migrations.md) | Snapshots, diff engine, DDL planner, runner, drift | P6 |
| [ImplPlan08CliDx.md](ImplPlan08CliDx.md) | CLI commands, error UX, formatter, introspection | P7 |
| [ImplPlan09TestingRelease.md](ImplPlan09TestingRelease.md) | Test matrix, benchmarks, docs, crates.io release | P8 |
| [ImplPlan10AppendixDecisions.md](ImplPlan10AppendixDecisions.md) | ADRs, risk register, explicit v2 deferrals | — |

---

## Timeline

Two tracks run in parallel. **Track C** = Vaibhav Gupta (spec-first, algorithmic work).
**Track D** = Vaibhav Gupta (implementation breadth, tests, plumbing).

| Week | Track C (Vaibhav Gupta) | Track D (Vaibhav Gupta) | Gate |
|---|---|---|---|
| 1 | P0 workspace + core IR; P1 grammar spec | P0 CI, test harness, docker-compose DBs | **G1**: sample schema parses to correct IR |
| 2 | P1 validation rules; P2 dialect trait | P1 parser lowering + error spans; P2 Postgres dialect | **G2**: valid Postgres DDL emitted |
| 3 | P3 codegen architecture + token design | P2 SQLite dialect; P3 entity/enum emission | **G3**: generated crate compiles clean |
| 4 | P4 filter algebra + typestate builders | P4 Insert/Update/Delete, pagination, tx | **G4**: CRUD round-trips on both DBs |
| 5 | P5 relation loader + batching algorithm | P5 codegen for relation accessors; P6 snapshot format | **G5**: nested include, no N+1 |
| 6 | P6 diff engine (the hard part) | P6 runner + drift; P7 CLI; P8 tests/docs | **G6**: diff produces correct migrations |
| 7 | *(buffer)* polish, docs, release | *(buffer)* bugfixes, examples | **Ship 0.1.0-alpha** |

**Gate rule:** a gate that fails on Friday moves the whole board right by one week.
Do not carry a failed gate forward — see the kill criteria in ImplPlan10.

---

## Progress tracker

Update the status column as work lands. `⬜ todo · 🟡 in progress · ✅ done · ⛔ blocked`

**P0 landed.** 34 tests pass; `cargo xtask ci` is green. One open item: the
Postgres half of the harness has not run on real hardware yet (no container
runtime on the dev machine), so confirm it on the first CI run before P1 depends
on it. Deviations are logged in
[ImplPlan10AppendixDecisions.md](ImplPlan10AppendixDecisions.md#p0-deviation-log).

**P1 landed.** All four schemas under `examples/` parse and lower to
snapshot-verified IR; 16 of 18 validation rules are enforced with a fixture each
(V03-naming and V18 deliberately deferred); errors accumulate and every one carries
a span and a fix. Deviations:
[P1 deviation log](ImplPlan10AppendixDecisions.md#p1-deviation-log).

**P0–P7 landed.** G1–G6 are signed off in their own plans, each with evidence
recorded there. The whole workspace is green against a live PostgreSQL 17.10 and
SQLite under `RUPRIZZLE_REQUIRE_DB=1`. P2-4 landed on `perf/research-harnesses`
with native `Pool::Postgres` and `Pool::Sqlite` construction and `row_buffer_size`.
G5's bound is now asserted, not asserted-by-inspection: `include_is_bounded` counts
the statements a two-level include issues and fails if it is ever more than one
per level. Two P5 checklist items are explicitly **not** signed off —
composite-key relations and the generated nested-create helpers; see the gaps
section of [ImplPlan06](ImplPlan06RelationsInclude.md#remaining-known-gaps).

**P8 is mostly complete.** Crates are published, the test matrix is green, the
examples compile, and `cargo xtask harden` runs. The docs site GitHub Pages
enablement and public announcement are still pending.

| Phase | Deliverable | Status | Gate |
|---|---|---|---|
| P0 | Workspace, core IR, CI green | ✅ | — |
| P1 | `schema.ruprizzle` parses + validates | ✅ | G1 ✅ |
| P2 | Postgres + SQLite DDL generation | ✅ | G2 ✅ |
| P3 | Entity/enum/token codegen compiles | ✅ | G3 ✅ |
| P4 | Query builder CRUD on both DBs | ✅ | G4 ✅ |
| P5 | Relations + nested include batched | ✅ | G5 ✅ |
| P6 | Migration diff + runner | ✅ | G6 ✅ |
| P7 | CLI complete, error UX polished | ✅ | — |
| P8 | Tests, benches, docs, published | 🟡 | Ship (docs site / announcement pending) |

---

## What v1 explicitly does NOT include

Deferred to 0.2+ — full reasoning in ImplPlan10.

- MySQL / MariaDB dialects (trait makes them additive; not in v1)
- Many-to-many implicit join tables (explicit join model only in v1)
- Studio / data browser GUI
- Database introspection → schema (`db pull`)
- Raw-SQL compile-time verification (`sqlx::query!` style macro over our schema)
- Polymorphic relations, self-referential recursive loading beyond depth 2
- Read replicas, sharding, connection routing
- Soft deletes, optimistic locking, audit columns as first-class features
- JSON path querying, full-text search helpers, PostGIS types

---

## Definition of done for v1

- [x] `cargo install ruprizzle-cli` works
- [x] Quickstart: empty dir → running query in under 5 minutes
- [x] Postgres **and** SQLite pass the identical integration suite
- [x] Migration diff handles the 12 change classes in ImplPlan07
- [x] Generated code has zero `clippy::pedantic` warnings
- [x] `examples/` has 4 schemas: blog, saas-tenant, ecommerce, minimal
- [x] Known-limitations doc is published and honest
