# Production Readiness Assessment — ruprizzle-orm v1.5.0

**Version assessed:** workspace `1.0.0` at branch `dev-v2-x`, commit `f2d898a`
(the tree the roadmap labels "v1.5.0 — Completed")
**Date:** 2026-08-31
**Assessor:** static analysis + live build, lint, doc, dependency and test execution
**Scope:** the ORM workspace, the three new edge/serverless adapter crates, and
Ruprizzle Studio.
**Method:** `secure-ship/prod-readiness-gate` universal items + the Rust crate ship
checklist, cross-checked against `rust-crate/publish-open-source`.
**Supersedes:** nothing. This is the first assessment of the v1.1–v1.5 line and it
sits alongside `ProjectPlan/ProductionReadiness.md` §17 (89/100 for `1.0.0`).

---

## 1. Verdict

> ## VERDICT: **BLOCK**
>
> v1.5.0 must not be published to crates.io in its current state.

> **Status as of 2026-08-31: remediated.** All eight items of §6 are closed. Sections
> §1–§7 below are preserved as written — they are the record of what the tree looked
> like at `f2d898a` and the argument for the block, and rewriting them would erase the
> finding. **§8 records what each item became, and §9 the gate results afterwards.**
> Read those two before acting on anything above them.

| Axis | Score | Grade | `1.0.0` (§17) |
|---|---|---|---|
| **Production readiness** | **56 / 100** | **D — Do not publish** | 89 / 100 |
| Engineering craft (v1.1–v1.4 only) | 87 / 100 | B+ | 90 / 100 |

The mechanical gates are **all green**, and that is precisely what makes this
assessment necessary. `cargo build`, `clippy -D warnings`, `cargo doc -D warnings`,
`cargo fmt --check`, `cargo deny check` and `cargo test --workspace` every one pass
across the whole workspace with `--all-features`. None of them can tell you that
three published-crate-shaped artifacts and half of Studio do not do the thing their
own documentation says they do.

**The v1.1–v1.4 work is real.** Postgres array filters, full-text search, soft
deletes, offline query checking, LSP 2.0, seeding, implicit m2m, nested writes,
recursive-CTE tree helpers, OpenTelemetry spans, primary/read-replica routing,
the TTL/tag query cache and PostGIS types are all genuinely implemented against
the compiler and the query builder, with tests that exercise real SQL.

**The v1.5 work is a shell.** `ruprizzle-turso`, `ruprizzle-d1` and `ruprizzle-neon`
are three copies of the same in-memory `HashMap` with a database's name on the
outside. Studio's entire data plane — table browser, cell editor, row delete, SQL
sandbox, EXPLAIN tree, migration safety diff — returns fabricated values and never
touches the database it connected to.

---

## 2. Scorecard by dimension

| # | Dimension | Weight | Score | `1.0.0` | Rationale |
|---|---|---|---|---|---|
| 1 | Correctness & testing | 20% | **4.5** | 8.0 | Suite is green, but the v1.5 tests assert that the fakes behave like fakes (§4.1, §4.2). Studio's five tests check HTTP 200 and fixture strings; the adapters' two tests each assert `frames_synced == 0` against a `sync()` that returns a literal. |
| 2 | Security | 15% | **8.0** | 9.0 | Core parameterised binding intact; `forbid(unsafe_code)` on all three new crates; `cargo deny check` clean on advisories, bans, licences and sources. Docked for Studio's unauthenticated mutation surface and a production guardrail built on substring matching (§5.1). |
| 3 | Operability & observability | 15% | **7.5** | 7.5 | v1.4's OTel spans and Prometheus metrics are real and are the genuine highlight of the line. Offset by Studio shipping two *fabricated* diagnostics (EXPLAIN, migration diff), which is worse than shipping none. |
| 4 | Data safety & migrations | 15% | **4.0** | 8.5 | `ruprizzle-migrate` itself is untouched and still sound. But a screen labelled "migration safety diff" that reports `SAFE` for every model without reading the database is a data-safety *regression* (§4.3), and the three adapters accept writes and silently discard them (§4.1). |
| 5 | Architecture & design | 10% | **7.5** | 9.0 | The `Executor` decoupling in `73831ed` is good, correct work that makes third-party backends possible. The three adapters that consume it are hollow. |
| 6 | CI/CD & release engineering | 10% | **3.0** | 7.0 | The three new crates appear in **no** publish list, **no** package check and **no** panic budget (§4.4). Studio is behind a non-default feature that **no CI job enables**, so it is never linted or tested by CI (§4.5). |
| 7 | Documentation | 5% | **3.0** | 9.0 | Five feature releases are entirely unrecorded: `CHANGELOG.md`'s `[Unreleased]` reads "_Nothing yet._" README and `docs/` describe `1.0.0` throughout. Worse, the crate-level docs of the three adapters describe connectivity that does not exist (§4.1). |
| 8 | API stability & semver | 5% | **4.0** | 7.0 | The workspace version has not moved from `1.0.0` across five releases. Three new public crates have never been through `cargo-semver-checks`. |
| 9 | Performance | 5% | **8.0** | 8.0 | Untouched this line; benches intact. |

**Weighted total: 5.625 / 10 → 56 / 100.**

---

## 3. Gate results — what actually ran

Every command below was executed against `f2d898a` on 2026-08-31.

| Gate | Command | Result |
|---|---|---|
| Build | `cargo build --workspace --all-features` | **PASS** (exit 0) |
| Format | `cargo fmt --all --check` | **PASS** |
| Lint | `cargo clippy --workspace --all-features --all-targets -- -D warnings` | **PASS** (exit 0) |
| Docs | `RUSTDOCFLAGS=-D warnings cargo doc --workspace --no-deps --all-features` | **PASS** (exit 0) |
| Tests (no DB) | `cargo test --workspace` | **PASS** (exit 0) |
| Tests (Postgres) | `RUPRIZZLE_REQUIRE_DB=1 RUPRIZZLE_TEST_PG_URL=… cargo test --workspace` | **PASS** for SQLite + Postgres |
| Dependencies | `cargo deny check` | **PASS** — advisories ok, bans ok, licences ok, sources ok |
| Licences | `LICENSE-MIT` + `LICENSE-APACHE` present, `license = "MIT OR Apache-2.0"` | **PASS** |
| Crate metadata | `description`/`repository`/`keywords`/`categories`/`readme` on all 14 crates | **PASS** |
| docs.rs config | `[package.metadata.docs.rs] all-features = true` on all 14 | **PASS** |

**UNVERIFIED (not counted as passes):**

- **MySQL/MariaDB paths.** No MySQL is installed on this machine, so the six
  aggregate tests that require it turned into failures under `RUPRIZZLE_REQUIRE_DB=1`
  (`crates/testkit/src/lib.rs:731` — "backend unavailable"). This is an environment
  artifact, not a defect; CI's `integration` job covers it. It is listed here because
  it was not verified locally, not because it is suspected broken.
- **`cargo publish --dry-run` / `cargo package --list` for the three new crates.**
  Cannot be run meaningfully — the crates are not in the release pipeline at all (§4.4).
- **`cargo semver-checks`** against the published `1.0.0` for the three new crates.
- **Error tracking with a verified live event.** No such wiring exists in this
  repository; it is a library, so this universal item is read as "the tracing/metrics
  surface is exercised", which v1.4 satisfies.

---

## 4. Blocking findings

### 4.1 — The three edge adapters are in-memory fakes with a database's name on them

**`crates/turso/src/lib.rs`, `crates/d1/src/lib.rs`, `crates/neon/src/lib.rs`.**

None of the three declares a driver dependency. There is no `libsql`, no `worker`,
no `reqwest`, no `tokio-postgres`, no HTTP client of any kind in any of the three
manifests. What they contain instead is byte-for-byte the same private field:

```rust
memory_store: RwLock<HashMap<String, Vec<HashMap<String, Value>>>>,
```

and the same `execute_internal`, which string-matches the SQL prefix, extracts a
table name by scanning for `" FROM "` or `" INTO "`, and reads or appends to that
`HashMap`. Bound parameters are stored under invented column names `col_0`, `col_1`,
… so even the in-memory round-trip does not preserve the caller's schema.

`TursoPool::sync()` — the operation the crate exists for — is:

```rust
Ok(SyncStats { frames_synced: 0, pushed_local_writes: false })
```

Its doc comment says it "Explicitly synchronizes the local replica with the remote
primary" and "Returns `TursoError::Sync` if remote synchronization fails." No
synchronisation is attempted and no failure can be returned. The crate-level docs
promise "connecting to remote Turso libSQL databases or running embedded SQLite
replicas with automatic background synchronization."

`auth_token` is accepted, stored, and never sent anywhere.

**Cost in production:** an application that adopts `ruprizzle-turso` compiles,
connects, runs its inserts, reports success, and loses every write on process exit.
Reads return an empty set that is indistinguishable from an empty table. There is no
error, no warning, and no log line. The failure is silent and total.

**Fix:** either implement the drivers against `libsql` / the D1 HTTP API / the Neon
WebSocket protocol, or do not publish these three crates. If they are wanted as a
staging artifact, rename the types to say what they are (`InMemoryTursoStub`), mark
the crates `publish = false`, and replace the crate-level docs with a statement that
no network I/O is performed. The current shape is the only one that is not acceptable.

### 4.2 — Studio's data plane is fabricated

`AppState` holds `pub pool: Option<ruprizzle::Pool>` (`crates/cli/src/studio/handlers/mod.rs:27`).
`grep -rn "\.pool" crates/cli/src/studio/` returns **nothing**. Studio connects to the
user's database and then never reads the connection.

Every handler produces invented data:

| Route | File | What it actually returns |
|---|---|---|
| `GET /studio/models/{m}/table` | `table.rs:304` | Five hardcoded rows: id `1`–`5`, every other cell the literal string `"Sample <field_name>"` |
| `PATCH …/rows/{id}/cell` | `table.rs:218` | Re-renders the submitted value into the cell template. No `UPDATE` is issued. |
| `DELETE …/rows/{id}` | `table.rs:276` | `StatusCode::OK`. Nothing is deleted. |
| `POST /studio/sandbox/execute` | `sandbox.rs:46` | Never executes the SQL. Always renders `✓ Query Executed Successfully` and a one-row table `result=1, status=OK`, plus an "Execution time" measured across the zero statements it ran. |
| `POST /studio/explain` | `explain.rs:32` | A hardcoded three-node Postgres plan (`Nested Loop Join` / `Index Scan on users_pkey` / `Seq Scan on posts`) with invented cost strings `0.42..12.80`. The submitted query is not even read. |
| `GET /studio/relations/{m}/{id}` | `relations.rs:34` | `"Linked <field_name>"` for every field |

The genuinely working parts are the dashboard, the ERD (`erd.rs`) and the model
navigation — all three read the parsed `Schema`, which is real.

**Cost in production:** a user opens Studio against their production database with
`--allow-writes`, edits a cell, sees it update in the UI, and closes the tab believing
the row changed. It did not. Separately, the sandbox teaches the user that arbitrary
SQL they type is being run against their database when it is not — the first time that
assumption is wrong in the other direction, it will be with a query they believed they
had already tested.

### 4.3 — "Migration safety diff" reports SAFE unconditionally

`crates/cli/src/studio/handlers/diff.rs:29`. The handler iterates the models in the
parsed schema and pushes, for each:

```rust
DiffChange {
    title: format!("Model `{}` structure verified", m.name),
    description: format!("Table contains {} fields with verified column mappings", m.fields.len()),
    risk: "SAFE".to_string(),
}
```

Nothing is verified. No database is read, no migration is diffed, and `risk` is a
literal. A schema whose migration would drop a populated column renders exactly the
same green `SAFE` badge as one that adds a nullable field.

**This is scored separately from §4.2 because it is the one fake with a blast radius
beyond the UI.** The other handlers mislead about state; this one mislabels danger as
safety, on the screen a user consults *before* running a destructive migration. It is
the single highest-risk item in this assessment.

### 4.4 — The new crates are outside the entire release pipeline

`ruprizzle-turso`, `ruprizzle-d1` and `ruprizzle-neon` appear in none of:

- `.github/workflows/release.yml` — the publish sequence names ten crates and stops
  at `ruprizzle-cli`.
- `xtask/src/main.rs:336` — the `cargo package --list` pre-flight, same ten.
- `xtask/src/main.rs:730` — the `cargo publish` ordering, same ten.
- `xtask/src/main.rs:46` `PANIC_BUDGET` — the `unwrap`/`expect`/`panic!` audit, eight
  crates. The new ones have no ceiling, so `cargo xtask harden` does not see them.

Consequence: **pushing a `v1.5.0` tag today publishes ten crates, none of which is the
headline v1.5 feature.** The release would announce edge adapters that are not on
crates.io.

`cargo xtask release-check --tag v1.5.0` also fails before any of that, because the
workspace version is `1.0.0` and `CHANGELOG.md` has no `1.5.0` heading. That gate is
working correctly and is currently the only thing standing between this tree and a
publish.

### 4.5 — Studio has zero CI coverage

The `studio` feature is not in `default`. `grep -rn "studio" .github/workflows/`
returns **nothing** — no job passes `--features studio`, and neither the `clippy` job
(`--workspace --all-targets`, no features) nor the release gate's two feature-specific
clippy runs enable it. `crates/cli/tests/studio_tests.rs` and its five tests have never
run in CI. Roughly 800 lines of HTTP handlers, six askama templates and an embedded
asset pipeline are unlinted and untested by the pipeline that gates releases.

It passes locally under `--all-features`, which is how this assessment checked it. That
is not a substitute for a job.

### 4.6 — Five releases of undocumented, unversioned change

- Workspace version is `1.0.0`. It has not moved through v1.1, v1.2, v1.3, v1.4 or v1.5.
- `CHANGELOG.md` `[Unreleased]` reads `_Nothing yet._`
- `RELEASES.md` stops at `0.1.0-alpha.2` — it never recorded `1.0.0` either.
- README's status paragraph, feature list, crate table and roadmap all describe `1.0.0`.
- User-facing documentation for the new features is absent: searching `README.md` and
  `docs/*.md` for `turso`, `d1`, `neon`, `studio`, `cache` and `replica` finds no
  user documentation for any of them.

A consumer reading docs.rs or the README has no way to learn that array filters,
full-text search, soft deletes, the query cache, replica routing or PostGIS types
exist — which means the genuinely good v1.1–v1.4 work is currently invisible.

### 4.7 — The LSP advertises `@@tenant`, which the parser rejects

`crates/lsp/src/completion.rs:302` offers `@@tenant($1)` as a completion
("declare multi-tenant partition key") and `crates/lsp/src/hover.rs:139` documents it.
`grep -rn "tenant" crates/parser/src crates/core/src crates/codegen/src crates/migrate/src`
returns nothing. Row-level security and multi-tenancy
(`ProjectPlan/v2/10_RowLevelSecurityAndMultiTenancyPlan.md`) are not implemented.

The editor therefore suggests an attribute that fails to parse. Small in scope, but it
is a defect the user meets in the first five minutes.

---

## 5. Non-blocking findings

### 5.1 Studio's production guardrail is substring matching

`is_production_url` (`crates/cli/src/studio/mod.rs`) tests for `"prod"`, `"production"`,
`"aws.neon.tech"` and `"turso.io"` in a lowercased URL. It misses every production
database not named for the fact — an RDS endpoint, a bare IP, `main-db.internal`. It
also false-positives on `postgres://…/product_catalog_dev`. As a speed bump it is
reasonable; it should not be described as a guardrail in user-facing copy. Studio also
has no authentication on its mutation routes; it binds `127.0.0.1` by default, but
`--host 0.0.0.0` is one flag away and exposes `DELETE` routes to the network.

### 5.2 Version drift in the editor extension

`editor/vscode/package.json` is at `1.2.0` while every crate is at `1.0.0`. Pick a
policy — lockstep or independent — and write it down.

### 5.3 `askama` 0.12

Two minor versions behind (0.14 current). Not a vulnerability; `cargo deny` is clean.
Worth folding into the v2 dependency modernisation work rather than doing now.

---

## 6. Shortest path to green

In order. Items 1–3 are the release-correctness fixes; 4–6 are the honesty fixes.

1. **Decide the fate of the three adapters.** Implement the real drivers, or mark them
   `publish = false` and retitle their public types and docs to say "in-memory stub".
   Do not publish them as-is. *(§4.1)*
2. **Make Studio's diff screen either real or absent.** Wire it to
   `ruprizzle-migrate`'s existing diff engine, or remove the route. An unconditional
   `SAFE` badge is worse than no screen. *(§4.3)*
3. **Wire the remaining Studio handlers to `AppState.pool`,** or gate the unimplemented
   routes behind a clearly-labelled `--demo` mode that says the data is synthetic. *(§4.2)*
4. **Add the new crates to all four lists** in `release.yml` and `xtask/src/main.rs`,
   including `PANIC_BUDGET`. *(§4.4)*
5. **Add a `--features studio` job to CI** covering clippy and test. *(§4.5)*
6. **Bump the workspace version and write the changelog** for v1.1–v1.5, then let
   `cargo xtask release-check` confirm tag/version/CHANGELOG agree. *(§4.6)*
7. Fix `@@tenant` — implement it or remove it from the LSP. *(§4.7)*

A defensible alternative that ships value this week: **cut `v1.4.0` instead.** Items
1–3 and 5 all concern v1.5 code exclusively. Everything in v1.1–v1.4 is real, tested,
and passes every gate. Bumping to `1.4.0`, documenting those four milestones, and
holding Studio and the adapters for `1.5.0` would put five months of genuine work in
users' hands without publishing anything that misrepresents itself.

---

## 7. Highest risk if you ship anyway

**The migration safety diff.** Everything else on this list costs a user time or
trust; that screen costs them data. It is reached at exactly the moment a developer
is deciding whether a schema change is safe to run against production, it renders a
green `SAFE` badge for every model in the schema, and it arrives at that verdict
without opening a database connection — the code path contains no query at all. A
developer who trusts it once, on a migration that drops a populated column, has an
unrecoverable data-loss incident and a post-mortem that ends at a tool which told them
it had verified something it never looked at. Shipping the three hollow adapters is
a reputational problem that a patch release can fix; shipping that badge is the one
failure here that a patch release cannot undo.

---

## 8. Remediation status

Tracks the seven items in §6 against the branch. Updated as each lands; every entry
names the commit that closed it.

| # | §6 item | Finding | Status |
|---|---|---|---|
| 1 | Decide the fate of the three adapters | §4.1 | **DONE** — stub route taken |
| 2 | Make Studio's diff screen real or absent | §4.3 | **DONE** — wired to introspection |
| 3 | Wire the remaining Studio handlers to the pool | §4.2 | **DONE** — every route queries |
| 4 | Add the new crates to all four release lists | §4.4 | **DONE** — plus an audit |
| 5 | Add a `--features studio` CI job | §4.5 | **DONE** — CI and release gate |
| 6 | Bump the workspace version and write the changelog | §4.6 | **DONE** — `1.5.0` |
| 7 | Fix `@@tenant` in the LSP | §4.7 | **DONE** — removed, plus `@@policy` |
| 8 | Studio guardrail copy and version drift | §5.1, §5.2 | **DONE** |

### 8.1 — Adapters: stub route taken (§4.1)

§6 item 1 offered two exits. The second was taken, in full:

- `TursoPool` → `InMemoryTursoStub`, `D1Pool` → `InMemoryD1Stub`,
  `NeonPool` → `InMemoryNeonStub` (builders and inner types renamed to match). The type
  a user writes down now says what it is.
- `publish = false` on all three manifests, with a comment pointing back at §4.1. The
  `documentation = "https://docs.rs/..."` keys are removed — they pointed at pages that
  will never exist.
- Crate-level docs and READMEs rewritten. Each opens with a "This crate performs no
  network I/O" heading that names what specifically does not happen (no libSQL
  connection, no D1 REST call, no Neon WebSocket), states that credentials are stored
  and never transmitted, and states that writes are lost on exit. Each documents the
  exact supported subset — `SELECT ... FROM t` ignores filters, joins, ordering and
  limits; everything that is not `SELECT` or `INSERT` is discarded — and, for Turso and
  Neon, points at the route that does work today (`ruprizzle::connect` against an
  embedded replica file, or against the Neon connection string, since Neon is ordinary
  Postgres over TLS).
- `InMemoryTursoStub::sync()` no longer fabricates `SyncStats { frames_synced: 0 }`. It
  returns the new `TursoError::Unsupported`, always. The signature is kept so a real
  adapter can drop in later without a call-site change.
- Bound parameters are no longer stored under invented `col_0`, `col_1` names when the
  statement declares a column list; `INSERT INTO users (id, email)` now round-trips as
  `id` and `email`. That makes the stub usable as an honest test double rather than a
  thing that merely looks like one.
- Two tests per crate replace the assert-the-fake-is-a-fake pair: one proves the
  declared-column round trip, one proves that a second stub instance sees none of the
  first one's writes — the non-persistence is now pinned by a test instead of being an
  undocumented surprise.

Dimension 4 (data safety) no longer carries the "accepts writes and silently discards
them" charge: the discard is now stated in the type name, the crate docs, the README
and a test. Dimension 8 (semver) drops the "three new public crates never through
`cargo-semver-checks`" concern for these three, since they are no longer publishable.

### 8.2 — Migration safety diff: made real (§4.3)

The single highest-risk item in §7. The handler no longer emits a card per model with
`risk: "SAFE"` hardcoded. `crates/cli/src/studio/handlers/diff.rs` now:

- reads the live catalog through `ruprizzle_migrate::introspect::pull`, the same
  introspector `ruprizzle db pull` uses, and compares it against the parsed schema;
- classifies each difference as `SAFE` / `CAUTION` / `DESTRUCTIVE` from what is
  actually in the database, not from a literal:
  - a column the schema drops runs `COUNT(*) ... WHERE col IS NOT NULL` first and is
    `DESTRUCTIVE` with the real row count in the card when anything would be lost,
    `CAUTION` when the column is entirely `NULL`;
  - a `NOT NULL` column added to a non-empty table with no default is `CAUTION`, with
    the row count that will make the migration fail;
  - tightening a nullable column to `NOT NULL` counts the `NULL`s and only says `SAFE`
    when there are none;
  - creating a missing table, and relaxing a constraint, are `SAFE`, which is true;
  - a table with no model is `CAUTION` with its row count, since Ruprizzle will not
    manage it.
- renders an explicit red "No comparison was made" banner, and **no cards at all**,
  when there is no connection or the catalog read fails. The old "zero drift detected"
  green banner is suppressed in that state. A user can no longer read absence of
  warnings as safety.

Scope is deliberately the same as `ruprizzle_migrate::drift` — tables, columns,
nullability — and the page's own description now says that column types, indexes and
foreign keys are *not* compared, rather than implying a completeness it does not have.

Two supporting changes:

- `crates/cli/src/studio/db.rs` is new: the single real data-access path for Studio.
  It casts to text per provider (not via `DbDialect::cast_expr`, whose MySQL mapping
  is `CHAR(255)` and would truncate), quotes identifiers per provider, and has no
  branch that invents a row when a query fails.
- `run_studio` no longer swallows connection failures with `.ok()`. A supplied
  `--database-url` that cannot be reached is now a startup error instead of a Studio
  that silently runs against `None`.

Tests: `diff_reports_real_drift_against_a_live_database` builds a real SQLite database
whose `users` table carries a populated `legacy_notes` column the schema does not
declare, and asserts the page names the column, marks it `DESTRUCTIVE`, and reports
"destroys the 1 row(s)". `diff_refuses_to_report_safety_without_a_connection` asserts
that with no pool the page contains neither `SAFE` nor the zero-drift banner.

### 8.3 — Studio's data plane: wired to the pool (§4.2)

`grep -rn "\.pool" crates/cli/src/studio/` used to return nothing. Every route in the
§4.2 table now issues real SQL through `crate::studio::db`, and every one of them
reports failure instead of substituting something that looks like data.

| Route | Was | Is |
|---|---|---|
| `GET /studio/models/{m}/table` | Five rows of `"Sample <field>"` | `SELECT` of the model's real columns, ordered by primary key, `LIMIT 50 OFFSET (page-1)*50`; `?search=` compiles to a parameterised `LIKE` across the scalar columns |
| `POST …/rows` | Echoed the submitted form back | Parameterised `INSERT` of the fields the user filled in, then the row is **read back** and that is what renders — defaults, identities and coercion are visible |
| `PATCH …/rows/{id}/cell` | Re-rendered the typed value | Parameterised `UPDATE … WHERE pk = ?`; `0` rows matched is `404`; the stored value is re-read so the cell shows what the database kept |
| `DELETE …/rows/{id}` | `200 OK`, nothing deleted | Parameterised `DELETE`; `200` only when a row was actually removed, `404` otherwise |
| `POST /studio/sandbox/execute` | Never ran the SQL; always `✓ Query Executed Successfully` with `result=1` | Runs the statement; reads render as the real column names and rows with the real count, writes as the real affected-row count, failures as the database's own message |
| `POST /studio/explain` | Hardcoded three-node Postgres plan; the query was not read | `EXPLAIN` (`EXPLAIN QUERY PLAN` on SQLite) of the submitted statement, parsed into the tree. `ANALYZE` is never used, so the statement is not executed. Cost and row estimates render **only** where the plan carries them |
| `GET /studio/relations/{m}/{id}` | `"Linked <field>"` per field | `SELECT … WHERE pk = ?` for the target record; a dangling foreign key says so instead of inventing values |

Cross-cutting:

- **No connection is a visible state, not an empty one.** Each screen renders a red
  banner naming what could not be done. The table grid says "No rows were read"
  rather than "No records found", so an unreachable database is not confusable with
  an empty table — the failure mode §4.2 called out.
- **Writes are refused for the right reason.** Read-only mode returns `403`;
  writes-enabled-but-disconnected returns `503`. Previously both paths returned `200`.
- **Text inputs bind safely into typed columns.** Studio's editors are text boxes, so
  `db::bind_expr` forces the placeholder to text and casts it to the field's declared
  scalar type. Values are always bound, never interpolated; identifiers come from the
  parsed schema and are quoted per provider.
- **List and relation fields are read-only.** They cannot be represented in a text
  box, so `db::is_editable` keeps them out of the insert and update paths instead of
  writing something wrong.
- **Sandbox output is escaped.** The result table is built by hand from database text,
  so `html_escape` runs over every column name and cell.
- `POST /studio/explain` had no caller. The sandbox page now has an "Explain Plan"
  button that posts the editor's contents to it.

Eleven tests replace the five that checked for HTTP 200 and fixture strings. They
stand up a real SQLite database and assert the database changed: the `UPDATE` is
visible in a follow-up `SELECT`, the `DELETE` drops the row count from 2 to 1, a
`DELETE` of a row that is not there returns `404`, the insert is present in a
`COUNT(*)`, a refused mutation leaves the table at 2 rows, a failing query surfaces
`no_such_table`, and the old fabricated strings (`Sample email`,
`Query Executed Successfully`, `users_pkey`, `0.42..12.80`, `Linked email`) are all
asserted **absent**.

Dimension 3 (operability) no longer carries the two fabricated diagnostics; dimension
1 (correctness) no longer rests on tests that assert the fakes behave like fakes.

### 8.4 — Release pipeline: one list, and a gate that keeps it honest (§4.4)

§6 item 4 said "add the new crates to all four lists". Three of those four lists are
publish lists, and §8.1 made the three crates `publish = false`, so adding them there
would be wrong. What was actually wrong is that the lists were maintained by hand and
nothing noticed when a crate fell out of them for a whole release line. So:

- **The publish sequence is now one constant.** `PUBLISH_ORDER` in
  `xtask/src/main.rs` is read by `cargo xtask release` and by the
  `cargo package --list` pre-flight, which each carried their own copy before.
- **A new `publish coverage` audit** runs inside `cargo xtask harden` and fails if:
  a workspace crate that is not `publish = false` is missing from `PUBLISH_ORDER`;
  a crate in `PUBLISH_ORDER` is `publish = false` or is not a workspace crate at all;
  or `.github/workflows/release.yml` does not publish exactly `PUBLISH_ORDER`, in
  order. Verified by deleting `publish = false` from `crates/turso/Cargo.toml` and
  confirming `harden` fails with
  `not in PUBLISH_ORDER: ruprizzle-turso (add it, or set publish = false in its manifest)`.
- **The panic and arithmetic/indexing budgets now cover all three adapters** at
  `0`/`0`/`0`. They are unpublished, but they are workspace source and are held to the
  same ceiling as everything else. Reaching zero needed the stub's
  `extract_insert_columns` and Studio's `extract_between` rewritten off direct slice
  indexing onto `str::get`, which is also a real robustness fix on multi-byte input.
  No existing budget was raised.

Dimension 6 (CI/CD) no longer has crates outside the panic budget, and the class of
defect — a list maintained by hand with no gate behind it — is now closed rather than
patched once.

### 8.5 — Studio in CI (§4.5)

`grep -rn "studio" .github/workflows/` returned nothing. It now returns two places:

- **`.github/workflows/ci.yml`** has a `studio feature` job running
  `cargo clippy -p ruprizzle-cli --features studio --all-targets -- -D warnings`
  and `cargo test -p ruprizzle-cli --features studio`. The Studio tests build their
  own temp-file SQLite database in-process, so the job needs no service container.
- **`.github/workflows/release.yml`** runs the same two commands inside the
  pre-publish verification gate, alongside the existing native-driver feature runs.
  Neither of those enabled `studio`, so the gate could pass with Studio uncompiled.

The `clippy` and `test` jobs still pass no `--features`, which is why the job is
separate rather than a flag added to an existing one. Both files carry a comment
pointing back at §4.5 so the next person to touch them knows why the job exists.

### 8.6 — `@@tenant` removed from the LSP (§4.7)

`@@tenant` is gone from `completion.rs` and `hover.rs`, and so is **`@@policy`**,
which §4.7 did not name but has exactly the same defect: `grep -rn "policy"` across
`crates/parser/src`, `crates/core/src`, `crates/codegen/src`, `crates/migrate/src` and
`crates/dialect/src` returns nothing, and row-level security is unimplemented. Leaving
one of the pair would have left the same trap one keystroke away.

`model_attribute_items` now carries a doc comment stating the rule — an attribute is
only offered once the parser and the migration engine act on it — and
`completion_does_not_offer_unimplemented_block_attributes` pins it: `@@index` must
still be suggested, `@@tenant` and `@@policy` must not.

**Correction to §4.7.** The finding says the editor "suggests an attribute that fails
to parse". It does not fail to parse. `ruprizzle validate` accepts

```
model User { id Int @id  org String  @@tenant(org) }
```

without a diagnostic: the grammar takes any block attribute and `lower.rs` only reads
`map`, `id`, `index` and `unique`, so everything else is discarded in silence. That is
worse than the reported behaviour — the developer gets no partitioning, no row-level
security, and no error telling them so.

**Deliberately not fixed here:** making the parser reject unknown attributes. It is
the right end state and would have caught this class of defect at the source, but it
changes parser behaviour for every existing schema and can turn a currently valid file
into a failing one, which is a decision for the version bump rather than a side effect
of an LSP fix. Recorded as follow-up work, and **done in [§10.1](#101--the-parser-rejects-unknown-attributes-v19)**.

### 8.7 — Studio's guardrail says what it is, and will not publish writes (§5.1)

The substring check is kept — it does catch the common case — but it no longer
presents itself as protection:

- `is_production_url` is renamed **`looks_like_production_url`**, and its doc comment
  states plainly that it is a name check, names what it misses (an RDS endpoint, a
  bare IP, `main-db.internal`) and what it false-positives on
  (`…/product_catalog_dev`), and says that anything which must not be reachable
  belongs behind credentials the developer does not hold.
- The refusal message no longer calls itself a "safety guardrail". It says which
  substring matched and that this is a name check, not a real safeguard.
- The test now pins the **negative** cases too: a production RDS endpoint and a bare
  IP both pass the check, and a development database called `product_catalog_dev`
  trips it. The limitation is now documented by a failing-if-changed assertion rather
  than by prose alone.

The second half of §5.1 — no authentication on the mutation routes, with
`--host 0.0.0.0` one flag away — is addressed by refusing the combination:

- **`--allow-writes` on a non-loopback bind is now a startup error** unless
  `--yes-i-know` is passed. The message says why: Studio has no authentication, so
  that bind publishes `INSERT`, `UPDATE` and `DELETE` to every host that can reach the
  port.
- A non-loopback bind **without** writes still prints a warning line at startup.
- `is_loopback_host` parses the address rather than string-matching `127.0.0.1`, so
  `::1`, `[::1]` and `127.0.0.2` are recognised; a test pins both directions.

This does not make Studio safe to expose — authentication is the real fix and is not
built. It makes the dangerous combination require an explicit statement of intent.

### 8.8 — Version, changelog and the drift that caused it (§4.6, §5.2)

The workspace moved from `1.0.0` to **`1.5.0`**. Five milestones landed without the
number moving, so the intermediate versions were never cut and one minor version
carries the whole line.

- **Root `Cargo.toml`**: `[workspace.package] version` and all thirteen internal
  `[workspace.dependencies]` pins.
- **`CHANGELOG.md`**: `[Unreleased]` is empty again and `## [1.5.0] - 2026-08-31`
  records v1.1–v1.5 by milestone. It has a **Fixed** section naming each thing this
  remediation repaired — the unconditional `SAFE` diff, the unread pool, the swallowed
  connection error, the unauthenticated non-loopback bind, `@@tenant`/`@@policy` — and
  a **Not published** section stating plainly what the three adapter crates are. The
  release notes describe what shipped, including what was wrong with it before.
- **`RELEASES.md`**, which had never recorded `1.0.0` either, gains a `1.5.0` entry.
- **`README.md`**: status paragraph, crate-table version note and roadmap section.
- **`docs/WhatsNewV1_1ToV1_5.md`**: the v1.5 section was a table of what does not work.
  It is now a table of what each screen queries, plus Studio's two real limits (no
  authentication; the production check is a name check) and, for the adapters, the
  route that does work for each provider today.

**§5.2 — version drift, and a gate for it.** [`docs/Versioning.md`](../../docs/Versioning.md)
is new and states the policy: the workspace crates and the VS Code extension move in
lockstep on one number, because they are not independently useful and a compatibility
table nobody maintains is worse than a version bump nobody needed.
`editor/vscode/package.json` goes `1.2.0` → `1.5.0`.

The policy is enforced, not just written down. `cargo xtask release-check` now also
fails when `editor/vscode/package.json` disagrees with the workspace version, and when
an internal `[workspace.dependencies]` pin does not equal it — a published crate with a
stale pin would resolve a sibling from the previous release. Verified by setting the
extension back to `1.2.0` and confirming
`editor/vscode/package.json is version 1.2.0 but the workspace is 1.5.0`.

Fifteen codegen snapshots and the blog example carried `RUPRIZZLE_VERSION = "1.0.0"`
and were updated with the bump.

---

## 9. Post-remediation gate results

Run against the tree at the end of §8, on 2026-08-31.

| Gate | Command | Result |
|---|---|---|
| Build | `cargo build --workspace --all-features` | **PASS** |
| Format | `cargo fmt --all --check` | **PASS** |
| Lint | `cargo clippy --workspace --all-features --all-targets -- -D warnings` | **PASS** |
| Docs | `RUSTDOCFLAGS=-D warnings cargo doc --workspace --no-deps --all-features` | **PASS** |
| Tests | `cargo test --workspace --all-features` | **PASS** |
| Dependencies | `cargo deny check` | **PASS** — advisories, bans, licences, sources |
| Hardening | `cargo xtask harden` | **PASS** — including the new publish-coverage audit; no budget was raised |
| Release check | `cargo xtask release-check --tag v1.5.0` | **PASS** — tag, workspace version, internal pins, CHANGELOG and the extension all agree |

Studio is now covered by `cargo clippy -p ruprizzle-cli --features studio --all-targets
-- -D warnings` and `cargo test -p ruprizzle-cli --features studio`, in CI and in the
release gate. Its suite went from 5 tests to 18.

**Still unverified, unchanged from §3:** the MySQL/MariaDB paths (no MySQL on this
machine; CI's `integration` job covers them), and `cargo semver-checks` against the
published `1.0.0`. The three adapter crates are no longer listed as unverified for
packaging: they are `publish = false` and outside the pipeline by design.

**Still open, deliberately:**

- Studio has no authentication. The non-loopback write refusal narrows the exposure;
  it does not close it.
- ~~The parser accepts unknown block attributes and lowering discards them silently
  (§8.6).~~ **Closed in [§10.1](#101--the-parser-rejects-unknown-attributes-v19).**
- `askama` 0.12 (§5.3), folded into the v2 dependency work as the assessment suggested.
- The three adapters are stubs, not drivers. Real `libsql`, D1 HTTP and Neon
  WebSocket implementations remain unwritten; the crates now say so instead of
  implying otherwise.

---

## 10. Follow-up work

The two items §9 left open by choice, taken up afterwards on 2026-09-01. Each was
open because the fix was a behaviour change or a new dependency rather than a repair,
which made it a decision rather than an omission.

### 10.1 — The parser rejects unknown attributes (V19)

§8.6 established that the defect is not the one §4.7 described. `@@tenant` never
failed to parse; the grammar accepts any name after `@@`, lowering looks up only the
four it reads, and everything else is discarded without a word. Removing the two
names from the LSP stopped the editor recommending the trap. It did not close it: any
`@@`-attribute a developer invented, and every misspelling of a real one, still
validated cleanly and did nothing.

The check is wider than the recorded item, because the recorded item was too narrow.
Field attributes have exactly the same hole — `@uniqe` lowered to no unique
constraint and no diagnostic — and there is no defensible version of this fix that
leaves that standing.

**V19**, in `crates/parser/src/lower.rs`:

- Block attributes are checked against `id`, `index`, `map`, `unique`.
- Field attributes are checked against `createdAt`, `default`, `deletedAt`,
  `generated`, `id`, `ignore`, `map`, `relation`, `renamedFrom`, `unique`,
  `updatedAt`, plus the open `db.*` native-type namespace, which stays open because
  each dialect names its own types.
- A near match becomes a suggestion (`@uniqe` → ``did you mean `@unique`?``). With no
  near match the diagnostic lists the whole vocabulary, on the grounds that the
  author has just learned it is smaller than they assumed and the next question is
  what is in it.

Both lists were checked against what `crates/lsp/src/completion.rs` offers, so the
editor cannot suggest something the parser now refuses.

**This is a breaking change and is meant to be.** A schema carrying an unknown
attribute stops validating. It was already not doing what its author wrote; the
difference is that now they are told. Every `.ruprizzle` file in this repository was
audited first — all of them use only known attributes, so nothing here needed
changing.

Fixtures `v19_unknown_block_attribute` and `v19_unknown_field_attribute` join the
executable rule table in `crates/parser/tests/invalid.rs`, which also asserts that
every diagnostic points somewhere and offers a fix.
