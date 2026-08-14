# v2 Features Plan — closing the Prisma/Drizzle DX gap

**Date:** 2026-08-14
**Author:** Vaibhav Gupta
**Status:** Draft — proposed, not yet scheduled
**Precondition:** [`ProjectPlan/ProductionReadiness.md`](../ProductionReadiness.md) currently
reports a **build failure at HEAD** (`dev-v0-2` @ `169606b`,
`crates/runtime/src/query.rs`). Nothing in this plan should start until that is fixed and the
full gate (`fmt`, `clippy`, `test`, `harden`) is green again. Building new surface area on an
unverified base compounds risk instead of reducing it.
**Builds on:** [`ProjectPlan/v1/PathToStableV1.md`](../v1/PathToStableV1.md), which is the
active plan for reaching 1.0 (semver policy, metrics/operability, capability commitments). v1
explicitly defers Studio/GUI, offline query checking, and edge/serverless drivers to
"post-1.0, reconsider then." This document is that reconsideration: it is the **v2 plan**,
scoped for *after* v1 ships, not a replacement for it.

---

## 1. Why now, and why these features

`docs/FeaturesMasterComparison.md` and the 2026-08-14 readiness reassessment show that the last
three weeks of work (savepoints, MySQL, `db pull` introspection, seeding, joins, correlated
subqueries, CTEs, set operations) closed nearly every **query-capability** gap against
Prisma/Drizzle. What's left is not query capability — it's the **developer-experience layer**
around the query engine that Prisma and Drizzle are actually known for:

| Gap | Prisma | Drizzle | ruprizzle today |
|---|---|---|---|
| Visual data browser / editor | Prisma Studio | Drizzle Studio | **None** |
| Compile-time / offline query checking | Generated client is fully typed against schema | `drizzle-kit` + TS types | **None** — no `sqlx-data.json`-equivalent |
| Editor support | Full LSP via generated client + TS | TS types + drizzle-kit | TextMate grammar only, no LSP |
| Seeding | `prisma db seed` | `drizzle-seed` | ✅ has it (`crates/cli/src/seed.rs`) |
| Schema introspection | `prisma db pull` | `drizzle-kit introspect` | ✅ has it (`db pull`) |
| Edge/serverless drivers | Accelerate, Data Proxy | Neon/Turso/D1/PlanetScale HTTP drivers | **None** |
| True streaming cursors | N/A (buffered) | N/A (buffered) | Buffered (parity, not a gap) |
| Array bind values (Postgres) | Supported | Supported | **Rejected at bind time** |

The pattern: ruprizzle now *generates and executes* SQL as well as or better than the
competition (per `docs/BenchmarkResults.md`), but it gives the developer far less **visibility
and confidence** while writing and running that code. That's the v2 theme — not "more query
features," but **"see what's happening, catch mistakes before running, work from anywhere."**

---

## 2. Feature set, ranked by leverage

### 2.1 Studio — local-first data browser and editor (headline feature)

**What:** A local web UI, launched via `ruprizzle studio` (new CLI subcommand), that connects to
the configured database and gives a Prisma-Studio-equivalent experience: browse tables, page
through rows, filter/sort, edit cells inline, create/delete rows, follow FK relations by
clicking through, and view the generated schema graph.

**Why it's the headline item:** it's the single most-requested Prisma/Drizzle feature category
(named explicitly as a competitor win in `docs/FeaturesMasterComparison.md`) and the one with no
existing ruprizzle counterpart at all — every other v2 item has a partial analog already.

**Design direction:**
- **Backend:** a thin HTTP server embedded in the `ruprizzle-cli` crate (behind a `studio`
  feature flag to avoid bloating the default CLI binary), reusing the existing `Pool` and
  generated schema metadata — **no new query engine**, it drives the same `SelectQuery`/`Tx`
  APIs the generated client uses. Serves a REST-ish JSON API: `GET /api/tables`,
  `GET /api/tables/:name/rows?offset=&limit=&filter=&sort=`, `PATCH /api/tables/:name/rows/:pk`,
  `POST`/`DELETE` equivalents. Read path first, write path gated behind an explicit
  `--allow-writes` flag (default read-only, matching Drizzle Studio's safer default over
  Prisma's).
- **Frontend:** a static single-page app (no separate build/deploy story — ship it embedded in
  the binary via `include_dir!` or similar), talking only to `localhost`. Keep it a static asset
  bundle rather than a live framework dependency, to avoid adding a JS toolchain to the Rust
  release pipeline.
- **Schema source:** reuse the same introspection/codegen metadata `db pull` and codegen already
  produce — Studio should describe tables from the same source of truth the generated client
  uses, not re-parse SQL independently.
- **Safety:** binds to `127.0.0.1` only by default; refuses to start against a database URL that
  looks like a production host unless `--yes-i-know` is passed (mirrors `db push`'s existing
  destructive-action gating pattern in the migrate crate).

**Effort:** Large — new CLI subcommand, new embedded HTTP server, new frontend. Estimate:
**3–4 weeks** for a v1 of Studio covering browse/filter/sort/edit on Postgres + SQLite.

**Sequencing:** Start after 2.2 (offline query checking) is scaffolded, since Studio's row
editor benefits from the same typed-schema introspection work; but Studio can proceed in
parallel by a separate contributor since the dependency is soft (shared schema metadata format,
not shared code).

### 2.2 Offline / compile-time query checking

**What:** A `ruprizzle check` command (and optional generated `query-manifest.json`, analogous
to `sqlx-data.json`) that validates every query in the codebase against the schema **without a
live database connection**, catching type mismatches, unknown columns, and invalid joins at
build/CI time instead of at first execution.

**Why:** named explicitly in `docs/KnownLimitations.md` as absent; it's the single gap most
likely to matter in CI pipelines that don't want a live DB for a type check step, and it's a
core part of both Prisma's (generated client) and Drizzle's (TS types) value proposition.

**Design direction:**
- Reuse the existing parser/codegen crates' schema model — this is fundamentally a "run the
  existing type-checking logic that today only runs against a live `Pool` at codegen time,
  but make it runnable against a serialized schema snapshot instead."
  Note: because ruprizzle's queries are already expressed through a typed Rust query builder
  (not a macro over raw SQL strings, unlike `sqlx::query!`), a large fraction of this checking
  already happens at **Rust compile time** via the type system. The gap is narrower than for
  `sqlx`: what's missing is validating *dynamically constructed* queries (e.g., `raw!` escape
  hatch, or filters built at runtime) against the schema ahead of execution.
- Ship `ruprizzle check` as a CI-friendly command: exit non-zero on any unresolvable query,
  print file:line diagnostics.
- Snapshot format: reuse the schema representation already produced by `db pull`/codegen so this
  doesn't invent a second schema IR.

**Effort:** Medium. Estimate: **1.5–2 weeks**, mostly plumbing existing type-checking logic into
a schema-snapshot-driven CLI path instead of requiring a live connection.

### 2.3 LSP / editor support

**What:** A minimal Language Server for `.ruprizzle`/schema DSL files (if one exists) or for
inline query macros — diagnostics (unknown column, type mismatch) and go-to-definition for
schema fields, at minimum. Full completion is a stretch goal.

**Why:** `docs/KnownLimitations.md` already flags "No LSP yet; syntax highlighting is available
as a TextMate grammar" as a known 0.2 deferral — this item is not new, it's promoting an
already-acknowledged gap into a scheduled v2 feature.

**Design direction:** Build on top of 2.2 — the LSP's diagnostics engine should be a thin
wrapper around whatever powers `ruprizzle check`, not a separate implementation, to avoid two
sources of truth for "is this query valid."

**Effort:** Medium-Large, but can ship incrementally (diagnostics-only first, completion later).
Estimate: **2–3 weeks** for diagnostics + go-to-definition; completion is a v2.1+ stretch.

**Sequencing:** After 2.2. Do not start before offline checking exists — the LSP has nothing to
wrap otherwise.

### 2.4 Postgres array bind values

**What:** Close finding #3 from the readiness assessment — `Value::Array` is currently rejected
at bind time in all four encoders (`sqlx::Any`, SQLite, native Postgres, `tokio-postgres`).

**Why it's in v2 and not v1-blocking:** it's a real capability gap but narrow and well-isolated
(4 call sites per the existing finding), unlike Studio/offline-checking which require new
subsystems. Grouping it here because it's genuine Prisma/Drizzle parity work, not because it's
architecturally related to the DX items above.

**Design direction:** Native Postgres path (`postgres-tokio-postgres`) is the natural place to
land this first — `tokio-postgres` has first-class array support already. `sqlx::Any` and
plain SQLite arrays likely stay unsupported/rejected (SQLite has no native array type), so this
should ship as a **documented, feature-gated capability** rather than a blanket enablement —
consistent with how `docs/adr/ADR-010-PostgresArraysAndSqliteFallback.md` already frames the
Postgres/SQLite split for a related concern.

**Effort:** Small. Estimate: **3–5 days**, given the ADR groundwork already exists.

### 2.5 Edge/serverless driver support

**What:** HTTP-based driver adapters for at least one of Neon, Turso/libSQL, or Cloudflare D1 —
matching the "Partial" edge/serverless support Prisma and Drizzle already have and ruprizzle
currently marks "No" for in `docs/FeaturesMasterComparison.md`.

**Why it's last:** highest effort-to-clarity ratio of the five items — it's a new driver
category (HTTP-transport SQL, not TCP), and the previous readiness assessments have consistently
found that ruprizzle's core differentiation is measured performance on traditional
Postgres/SQLite deployments, not edge reach. This is valuable but should not block Studio or
offline checking, which serve every existing user immediately.

**Design direction:** Pick **one** target first (Turso/libSQL is the closest architectural
match to the existing `sqlite-rusqlite` path) rather than building a generic HTTP-driver
abstraction speculatively. Treat it as a new feature-gated crate (`ruprizzle-turso` or similar),
mirroring the `postgres-tokio-postgres`/`sqlite-rusqlite` pattern already established, so it
plugs into the existing `Pool` accessor pattern (`as_turso()`, etc.) rather than inventing a new
integration seam.

**Effort:** Large, open-ended. Estimate: **3+ weeks** for a single target driver, and this
should be timeboxed/re-scoped rather than committed to a hard estimate until a target is chosen.

---

## 3. Sequencing and phases

```
Phase 0 (prerequisite, not part of v2 scope):
  Fix crates/runtime/src/query.rs build break → green gate → tag a known-good v1 baseline.

Phase 1 (v2.0 — DX foundation, ~5-6 weeks):
  2.4 Array binds            [independent, can start immediately, smallest]
  2.2 Offline query checking [independent, unlocks 2.3]

Phase 2 (v2.1 — visibility layer, ~4-5 weeks, can overlap late Phase 1):
  2.1 Studio (read-only first, writes behind --allow-writes)
  2.3 LSP diagnostics (depends on 2.2's checking engine)

Phase 3 (v2.2 — reach, open-ended, ~3+ weeks):
  2.5 One edge/serverless driver (target chosen at phase start, not now)
```

Rationale for this order: 2.4 is a quick, isolated win that removes an existing documented
limitation with no dependency on anything else. 2.2 is scoped second because 2.3 (LSP) and
part of 2.1 (Studio's schema awareness) both benefit from its schema-snapshot infrastructure —
building it once and reusing it avoids two independent "read the schema and validate against it"
implementations. Studio is scoped as the headline deliverable of Phase 2 despite being the
largest single item, because it's the most externally visible gap and the one most likely to
change a prospective user's evaluation (per §7 of the readiness assessment: "a team choosing
between it and Prisma or Drizzle is weighing trade-offs rather than counting absences" — Studio
is currently a flat absence, not a trade-off). The edge driver is scoped last because it's the
least differentiated relative to ruprizzle's proven strength (raw performance on traditional
deployments) and the most open-ended in scope.

---

## 4. What v2 deliberately does not include

Carried forward from `ProjectPlan/v1/PathToStableV1.md`'s explicit out-of-scope list, and still
out of scope here unless a future plan revisits them:

- MongoDB, ScyllaDB, DuckDB, Cassandra support.
- Multi-tenancy / row-level security primitives.
- `pgvector` / vector search (v1 plan defers to 1.1; not pulled forward here).
- Replacing `sqlx::Any` as the default driver path (ADR-009's position stands).
- MySQL is **not** in this out-of-scope list — it's already shipped as of the 2026-08-14
  readiness reassessment and needs no further v2 work beyond normal maintenance.

---

## 5. Success criteria for v2

A v2 release is ready to ship when, for each landed feature:

1. **Array binds (2.4):** Postgres array values round-trip through `postgres-tokio-postgres`
   with a passing property test; `docs/KnownLimitations.md` updated to remove the caveat.
2. **Offline checking (2.2):** `ruprizzle check` runs in CI with no live DB, catches at least the
   same class of errors the previous readiness assessment's `query.rs` regression would have
   caught (a strong internal validation of the feature's value, given that regression's cause).
3. **Studio (2.1):** a fresh clone + `ruprizzle studio` gets a developer browsing real rows
   within one command, with FK-relation click-through working on at least Postgres and SQLite.
4. **LSP (2.3):** at minimum, unknown-column and type-mismatch diagnostics surface in VS Code
   via the TextMate grammar's language ID, without requiring a live DB connection.
5. **Edge driver (2.5):** at least one target (Turso preferred) passes the same conformance
   suite the existing `sqlite-rusqlite`/`postgres-tokio-postgres` paths pass.

Each item ships independently behind its own feature flag or subcommand — v2 is not a single
big-bang release, consistent with how `sqlite-rusqlite` and `postgres-tokio-postgres` shipped
independently in the run-up to beta.1.
