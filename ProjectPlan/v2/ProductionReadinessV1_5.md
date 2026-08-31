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
