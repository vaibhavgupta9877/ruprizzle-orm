# ruprizzle-orm v1.0.0 readiness analysis

**Date:** 2026-08-17  
**Branch:** `dev-v0-2`  
**HEAD:** `facf6d5`  
**Workspace version:** `0.1.1-beta.1`  
**Assessor:** Devin (live build, lint, test, deny, doc, public-api)

---

## TL;DR

**No — the current package is not ready to be published as a stable `1.0.0` release.**

The code is feature-rich and the feature workstreams up through W5 (minus W5-07 LSP) are largely implemented, but the repository is still at version `0.1.1-beta.1` and multiple hard release gates are red right now:

- `cargo fmt --all --check` fails.
- `cargo clippy --workspace --all-targets` fails because `tests/integration/tests/diagnostics_snapshot.rs` still references the removed `SchemaError::ScalarListUnsupported` variant.
- `cargo xtask harden` fails for the same reason.
- `cargo deny check advisories` fails on `RUSTSEC-2023-0071` (timing side-channel in `rsa 0.9.10`, pulled in by `sqlx-mysql`).
- `cargo doc --workspace --no-deps` emits rustdoc warnings that would fail CI with `RUSTDOCFLAGS: -D warnings`.
- No `1.0.0-rc.1` tag exists and no RC feedback window has been run — and `ProjectPlan/v1/PathToStableV1.md` explicitly forbids skipping that.
- A 48-hour soak test has not been performed (only 10–15 s smoke runs).

After the build breaks and the advisory are fixed, the next sensible milestone is probably `0.4.0-beta.1` (or `0.3.0-beta.1` at minimum), then a real `1.0.0-rc.1` with at least a two-week feedback period before `1.0.0`.

---

## 0. Fix status

| # | Blocker | Status | Commit |
|---|---------|--------|--------|
| 1 | `cargo fmt --all --check` | **fixed** | TBD |
| 2 | `tests/integration/tests/diagnostics_snapshot.rs` stale `SchemaError` | **fixed** | — |
| 3 | `cargo clippy --workspace --all-targets` / `cargo xtask harden` | **not fixed** | — |
| 4 | `cargo deny` `RUSTSEC-2023-0071` | **not fixed** | — |
| 5 | rustdoc warnings | **fixed** | — |
| 6 | PostgreSQL / `sqlx::Any` benchmark fix | **not fixed** | — |

---

## 1. Current state of the repository

| Item | Current value |
|------|--------------|
| Version | `0.1.1-beta.1` (`Cargo.toml` workspace package) |
| Branch | `dev-v0-2` |
| Tags | none (no `1.0.0-rc.1`) |
| Production-readiness score (last self-assessment) | **82 / 100** at `7636f44` (`ProjectPlan/ProductionReadiness.md`) |
| Plan status | W0–W4 and W5-01–W5-06 marked complete; W5-07 (LSP) deferred; W6-04/W6-05 (RC + final assessment) not executed (`ProjectPlan/v1/PathToStableV1.md`) |
| Tests (live, excluding stale integration test) | **all green** — `cargo test --workspace --exclude ruprizzle-integration-tests` passes, `cargo test -p ruprizzle --features sqlite-rusqlite` passes |
| `cargo fmt --all --check` | **fails** (`crates/runtime/examples/cross_orm_bench.rs` has formatting diffs) |
| `cargo clippy --workspace --all-targets` | **fails** in `tests/integration/tests/diagnostics_snapshot.rs` |
| `cargo xtask harden` | **fails** at the lint step for the same reason |
| `cargo deny check advisories` | **fails** on `RUSTSEC-2023-0071` |
| `cargo doc --workspace --no-deps` | succeeds but emits 4+ rustdoc warnings |

The most important signal here is that the build/test gating the project depends on is not actually green at HEAD, even though the plan text says W0 is complete. This is a process problem, not just a code problem.

---

## 2. Hard blockers for a stable v1.0.0

### 2.1 Two mechanical build failures

1. `cargo fmt --all --check` is red. The diff is purely in `crates/runtime/examples/cross_orm_bench.rs` (import order and line-wrapping).
2. `tests/integration/tests/diagnostics_snapshot.rs` still uses `SchemaError::ScalarListUnsupported`:

   ```rust
   SchemaError::ScalarListUnsupported {
       found: "String".into(),
       span: span_of("title").into(),
   },
   ```

   This variant no longer exists in `crates/core/src/diagnostic.rs` — it was removed when scalar arrays became a supported feature. This is a stale unit-test reference; the fix is either to remove the case or replace it with the current diagnostic for the same scenario.

Because of this one file, `cargo clippy --workspace --all-targets` and `cargo xtask harden` both fail. This single issue is what makes the tip unpublishable as any kind of release.

### 2.2 Dependency security advisory

`cargo deny check advisories` reports `RUSTSEC-2023-0071` against `rsa 0.9.10`, which is pulled in transitively by `sqlx-mysql 0.8.6` (`deny.toml`). The advisory has no safe upgrade available. For an alpha/beta, this can be documented/excepted; for a stable v1.0.0, having a known crypto timing-side-channel in the dependency tree is a real issue. Options are:

- Wait for an `rsa` / `sqlx` patch.
- Add a `cargo-deny` advisory exception only for MySQL and document the risk.
- Accept that MySQL support cannot be called production-grade until the dependency is fixed.

### 2.3 Documentation warnings

`cargo doc` currently produces rustdoc warnings (broken/redundant intra-doc links in `crates/runtime/src/compile.rs`, `executor.rs`, `filter.rs`). The CI `docs` job runs with `RUSTDOCFLAGS: -D warnings` (`.github/workflows/ci.yml`), so the docs job would fail on CI even if local `cargo doc` exits 0.

### 2.4 v1.0 process is not complete

The project's own definition of done for `1.0.0` requires:

- Every workstream exit gate met.
- Production readiness ≥ 92/100.
- Fuzzers clean at ≥ 4 CPU-hours per target.
- 48-hour soak with no leak/degradation.
- `1.0.0-rc.1` with a real feedback window and at least one external upgrade report. (`ProjectPlan/v1/PathToStableV1.md` §7)

None of these are true today. The plan is explicit: the current 43 downloads across four 0.x versions are **not** enough exposure to freeze an API on, and the RC window is non-negotiable. (`docs/Stability.md`)

### 2.5 Assurance gaps

- **Soak**: only 10 s and 15 s smoke runs are recorded; the 48-hour run is documented but not executed. (`docs/SoakReport.md`)
- **Mutation testing**: `ruprizzle-migrate` has a **~25 % mutation score** (99/393 killed), meaning many tests pass without asserting the behavior they cover. (`docs/MutationTesting.md`) `ruprizzle` runtime mutants are listed but the baseline is not recorded yet.
- **Coverage**: last measured at **~68 %** overall. (`docs/TestingAnalysis.md`)
- **Pre-v1 critical bugs**: several critical transaction/pool defects were found and fixed recently (BUG-01 through BUG-06, BUG-08, BUG-09), which is good, but it is also evidence that the native driver paths were not covered well enough before the beta. (`ProjectAnalysis/PreV1/PendingBugs.md`)

---

## 3. Feature coverage (what is actually there)

The feature surface is genuinely broad and mostly matches the "Prisma/Drizzle parity" goal.

### Query builder / CRUD
- Select, insert, update, delete, bulk insert, upsert.
- Typed filters, `and`/`or`/`all`/`any`, `between`, `in_set`, `not_in_set`, null, string matchers, `distinct`, `count`, `exists`.
- Pagination: `limit`/`offset`, `page`, `after`/`before` cursors.
- Ordering, projections, `columns`.
- Aggregates: `sum`, `avg`, `min`, `max`, `count`, `count_distinct`, plus `group_by`/`having`.
- Joins: `inner_join`, `left_join`, `right_join`, `full_join`, self-joins, table aliasing, typed `Join2`/`LeftJoin2`/`Maybe` results. (`docs/adr/ADR-011-ExplicitJoinsAlongsideBatchedRelations.md`)
- Subqueries: `in_subquery`, `not_in_subquery`, `exists`/`not_exists` correlated subqueries.
- CTEs: `with`, `with_recursive`.
- Set operations: `union`, `union_all`, `intersect`, `except`.
- JSON operators on Postgres, MySQL, SQLite (with the SQLite containment caveat documented). (`docs/KnownLimitations.md`)
- Arrays: Postgres native, SQLite/MySQL JSON fallback, `contains`/`contained_by`/`overlaps`.
- Prepared statements with `prepare()` and cheap rebind.
- Conditional building: `filter_if`, `set_if`, etc.

### Relations
- One-to-many, one-to-one, many-to-many via explicit join model (`@relation(through: ...)`). (`docs/FeaturesMasterComparison.md`)
- Batched `include` (bounded one query per level), per-relation filters and `take`.
- Nested writes: `with_related` insert, `connect`/`disconnect`/`set` on update, `cascade`/`set_null`/`restrict` on delete.

### Transactions / drivers
- `Tx`, `begin_with_isolation`, savepoints to arbitrary depth, closure form, correct `Drop` rollback on native drivers. (`CHANGELOG.md`)
- Drivers: `sqlx::Any` (default/generic), native `sqlx::Postgres`, native `sqlx::Sqlite`, native `sqlx::MySql`, native `rusqlite` (`sqlite-rusqlite`), native `tokio-postgres` (`postgres-tokio-postgres`).
- PostgreSQL, SQLite, MySQL/MariaDB supported.

### Migrations / CLI
- Declarative `schema.ruprizzle` DSL, parser, IR, codegen.
- `migrate dev`, `deploy`, `status`, `resolve`, `reset`, `squash`, `db push`, `db pull` introspection, `db seed`. (`CHANGELOG.md`)
- Drift detection, destructive gating, rename detection with prompt, FK-cycle handling, down migrations.
- Metrics (`metrics` feature), tracing, slow-query events, `PoolStats`, connection lifecycle events. (`docs/Operations.md`)

### Explicitly missing / deferred
- **LSP** for `schema.ruprizzle` (W5-07, deferred to 0.2). (`docs/KnownLimitations.md`)
- **Compile-time query checking** (deferred to 1.1).
- **Vector / pgvector search** (deferred to 1.1).
- **MSSQL / MongoDB / DuckDB / ScyllaDB / CockroachDB / edge serverless** (out of scope for 1.0).
- **Multi-tenancy / row-level security** (out of scope).
- **Lazy loading** (deliberate rejection — batched loading is the design position). (`ProjectPlan/v1/PathToStableV1.md` §5 note)

The feature comparison table in `docs/FeaturesMasterComparison.md` is the best reference for where this sits relative to competitors.

---

## 4. Public API and stability posture

- A `cargo public-api` review was performed (W6-01). No accidental internal leakage was found; `ruprizzle-testkit` and `xtask` are correctly `publish = false`. The generated client's public shape is intentionally semver-covered, while its internal helpers are not. (`docs/PublicApiReview.md`)
- `docs/Stability.md` defines the semver contract, MSRV policy (Rust 1.85), deprecation windows, and the RC process.
- `cargo-semver-checks` is wired into CI. (`.github/workflows/ci.yml`)
- The migration guide from `0.1.1-beta.1` to `1.0.0` exists. (`docs/MigrationGuideToV1.md`)

This is all good. The problem is that the **process** has not been executed: no RC, no final rescored assessment, and the working tree is not even green on the existing gates.

---

## 5. Performance and competitive guess

### 5.1 Measured SQLite numbers (latest log, 2026-08-17)

These are the most recent apples-to-apples numbers from `local/cross-orm-bench/BENCHMARKS.log` (1,000 users, 10,000 posts, 50,000 comments, etc.):

| Operation (µs/op, lower = better) | ruprizzle (rusqlite) | Diesel | prax | ruprizzle (sqlx) | Sea-ORM | Drizzle | Prisma |
|-----------------------------------|----------------------:|-------:|-----:|-----------------:|--------:|--------:|-------:|
| `select_by_pk` | **3.1** | 10.1 | 17.6 | 20.9 | 66.5 | 39.3 | 174.8 |
| `find_many_1000` | **424.2** | **303.8** | 794.7 | 1,620.4 | 1,710.1 | 406.8 | 2,879.6 |
| `find_filtered_ordered` | **549.2** | **421.8** | 925.3 | 1,841.1 | 1,810.3 | 480.2 | 3,332.9 |
| `find_filtered_paginated` | 309.0 | 307.7 | 347.4 | 393.1 | 472.9 | 372.6 | 667.1 |
| `find_in_list` | 32.8 | **41.4** | 80.8 | 107.1 | 131.8 | 104.8 | 445.1 |
| `find_complex_filter` | **156.7** | 167.2 | 230.3 | 310.3 | 355.2 | 248.4 | 836.1 |
| `count_filtered` | **19.5** | 25.4 | 34.9 | 35.5 | 89.9 | 46.7 | 187.0 |
| `exists_filtered` | **2.6** | 9.5 | 16.1 | 17.1 | 58.6 | 46.1 | 155.9 |
| `include_posts` | 7,411.6 | **3,725.5** (manual join) | 10,818.1 | 22,514.6 | 22,010.8 | 189,412.7 | 42,982.6 |
| `include_posts_and_comments` | 57,814.5 | **20,724.0** | 43,439.2 | 137,868.3 | 118,143.1 | 9,280,171.7 | 262,781.5 |
| `bulk_insert_1000` | 1,434.7 | **7,080.6** | **1,195.3** | 1,966.0 | 6,279.6 | 8,518.1 | 13,614.5 |
| `prepared_select_by_pk` | **2.6** | 9.9 | 4.5 | 18.2 | 63.7 | 14.9 | 177.0 |

### 5.2 PostgreSQL / `sqlx::Any` overhead

`docs/Performance.md` shows ruprizzle within **7–12 %** of hand-written `sqlx::query` on a local Postgres for single-row and 1,000-row selects, and within **1.8 %** for bulk insert. The 2-level `include` is within the 15 % threshold. The remaining overhead is largely `sqlx::Any` text marshalling of `Uuid`/`Decimal`/`DateTime`/`Json`; the native `tokio-postgres` feature avoids that for Postgres. (`docs/Performance.md`)

### 5.3 Query construction (no I/O)

ruprizzle, prax, and Diesel all compile a query in ~0.4–2 µs. Sea-ORM and Drizzle are an order of magnitude slower. Prepared-statement rebind in ruprizzle is ~53 ns. (`docs/Performance.md`)

### 5.4 Calculated competitive guess

Based on these numbers, assuming the measured versions hold and no major upstream rewrites:

| Criterion | Likely ranking | Why |
|-----------|---------------|-----|
| SQLite single-row / simple read | **ruprizzle (rusqlite) #1** | 3.1 µs is the fastest measured; zero async-dispatch overhead. |
| SQLite multi-row read | **Diesel #1, ruprizzle (rusqlite) #2** | Diesel's native `libsqlite3-sys` is leaner on multi-row decode. The gap is ~20–50 %. |
| SQLite relation `include` (automatic) | **ruprizzle (rusqlite) #1** | Batched loader beats Sea-ORM, Prisma, Drizzle. Diesel's manually-written join is faster but is not automatic. |
| SQLite bulk insert | **prax #1, ruprizzle (rusqlite) #2** | prax is slightly faster; both are far ahead of Diesel/Sea-ORM/Prisma/Drizzle. |
| Query construction overhead | **Diesel/prax/ruprizzle cluster at the top** | All are sub-microsecond. Sea-ORM/Drizzle are slower. |
| PostgreSQL ORM overhead | **Top tier, near raw sqlx** | Within single-digit to low-double-digit percent of hand-written `sqlx`. Likely faster than Prisma/Sea-ORM, comparable to Diesel for non-join work. |
| Network-bound production workload | Differences shrink | Once latency is network-dominated, the ORM layer matters less, but ruprizzle's no-sidecar, low construction cost, and no query-engine binary are still advantages. |
| Rich types with `sqlx::Any` | ruprizzle at a measurable disadvantage | Uuid/Decimal/DateTime/Json round-trip as text; use `postgres-tokio-postgres` or `sqlite-rusqlite` to remove this. |

**Honest bottom line for performance:** ruprizzle is already in the fastest tier, especially on SQLite with the `rusqlite` feature. On PostgreSQL it should be competitive with Diesel and faster than the higher-level typed builders, provided the native `tokio-postgres` path is used for rich types. It is not "universally faster than everything" — a hand-optimized Diesel join can still beat it — but it is the strongest automatic/batched-relation loader in the measured set and has the lowest query-construction cost.

---

## 6. Planned fix: PostgreSQL / `sqlx::Any` rich-type overhead

### 6.1 What is actually happening

`crates/runtime/src/value.rs` has two different encoders for the `Value` enum:

- For `sqlx::Any`, rich types (`Decimal`, `Uuid`, `DateTime`, `Date`, `Time`, `Json`) are converted to `String` and then bound as text. The database must parse/cast them. (`crates/runtime/src/value.rs` lines 315–390)
- For `sqlx::Postgres`, the same rich types are encoded natively through `sqlx::Encode<'q, sqlx::Postgres>`, avoiding string conversion. (`crates/runtime/src/value.rs` lines 392–535)

`sqlx::Any` itself only supports a narrow set of value kinds: `Null`, `Bool`, `SmallInt`, `Integer`, `BigInt`, `Real`, `Double`, `Text`, `Blob`. (`~/.cargo/registry/src/index.crates.io-*/sqlx-core-0.8.6/src/any/value.rs`) There is no native `Any` representation for `Uuid`, `Decimal`, `DateTime`, or `Json`, so any `Value` bound through `sqlx::Any` must be text/bytes.

### 6.2 Why real-world Postgres is already mostly safe

`crates/runtime/src/pool.rs` already selects the native `sqlx::Postgres` pool by default for `postgres://` and `postgresql://` URLs. (`crates/runtime/src/pool.rs` lines 404–437) This means a normal `ruprizzle::connect("postgres://...")` call does **not** use `sqlx::Any`; it uses `Pool::Postgres`, and the `Value` encoder will be the native `sqlx::Postgres` path. Rich types are already bound as native Postgres values there.

### 6.3 Where the problem shows up

The `end_to_end` benchmark in `crates/runtime/benches/end_to_end/main.rs` explicitly constructs an `sqlx::Any` pool and wraps it as `ruprizzle::Pool::Any` (`crates/runtime/benches/end_to_end/main.rs` lines 195–214). The numbers in `docs/Performance.md` come from that `Any` comparison. This is an accurate like-for-like measurement against `sqlx::Any`, but it is **not** representative of the default runtime path. Users who call `ruprizzle::connect` on a Postgres URL are already on the native path.

### 6.4 Proposed fix steps

1. **Update the `end_to_end` benchmark to use the default connection path.**
   - Replace the manual `sqlx::any::AnyPoolOptions` construction with `ruprizzle::connect_with(&url, &config)`.
   - This will make the benchmark exercise `Pool::Postgres` (or `Pool::Sqlite` / `Pool::Mysql` depending on URL) and produce numbers that match what users get by default.
   - Keep the existing `Any` numbers as a separate optional arm or a footnote, so the `sqlx::Any` overhead is still documented but not the headline result.

2. **Re-run Postgres benchmarks and refresh `docs/Performance.md`.**
   - Run `cargo bench -p ruprizzle --bench end_to_end` with `RUPRIZZLE_TEST_PG_URL` set.
   - Compare against the same hand-written `sqlx::query` using a native `sqlx::Postgres` pool (not `sqlx::Any`), so the baseline is fair.
   - The expected result is that the single/1,000-row select overhead drops from the current 7–12 % to a smaller number (likely 2–5 %) because rich-type text marshalling is no longer in the hot path.

3. **Clarify `docs/Performance.md` and `docs/KnownLimitations.md`.**
   - State explicitly that `sqlx::Any` text marshalling only affects users who explicitly construct `Pool::Any` or use a non-default `Any` URL.
   - Document that the default `postgres://` URL uses native `sqlx::Postgres` and avoids this cost.
   - Keep the advice that rich types on SQLite with the default `sqlx` path are stored as text; for best SQLite performance, enable `sqlite-rusqlite`.

4. **Consider removing or warning on `Pool::Any` for Postgres in v1.**
   - Option A: Keep `Pool::Any` as an escape hatch but emit a `tracing::debug!` or documentation note that rich types may be slower.
   - Option B: For v1, make the generic `Any` path transparently delegate to the native `sqlx::Postgres`/`Sqlite`/`MySql` pool based on the URL scheme, so users never accidentally use `sqlx::Any`. This is close to what `connect_with` already does at construction time, but `Pool::Any` can still be created by hand.

5. **If `sqlx::Any` must be kept for dynamic URL support:**
   - Optimize the text conversion in `value.rs` to avoid per-value `String` allocations where possible (e.g., use a thread-local or stack buffer for `Uuid`/`DateTime` `to_string()`). This is a micro-optimization and will not fully close the gap, because the database still has to parse text.

### 6.5 Estimated impact

- For the default `postgres://` user: **no code change needed**; they already get native encoding. The fix is mostly benchmark and documentation.
- For the `sqlx::Any` user on Postgres: impact is limited by `sqlx::Any` itself; the real fix is to switch to the native pool.
- Expected `Performance.md` numbers: move from "within 7–12 % of hand-written sqlx" to "within 2–5 %" for simple selects, making the v1 performance story stronger.

---

## 7. What I recommend before any v1.0.0

1. **Fix the two mechanical blockers right now**:
   - Run `cargo fmt --all` and commit.
   - Update or remove the `SchemaError::ScalarListUnsupported` case in `tests/integration/tests/diagnostics_snapshot.rs`.
2. **Fix the rustdoc warnings** (`compile.rs`, `executor.rs`, `filter.rs`) so `RUSTDOCFLAGS="-D warnings"` is green.
3. **Decide on `RUSTSEC-2023-0071` / `rsa`**: either wait for a patch, add a documented `cargo-deny` exception, or explicitly scope MySQL support as "not recommended for security-sensitive deployments" until the advisory is resolved.
4. **Re-run `cargo xtask harden` and `cargo deny check`** and confirm green.
5. **Run the 48-hour soak** and record the result in `docs/SoakReport.md`.
6. **Complete and record the runtime mutation-testing baseline** (the migrate baseline is already at a poor 25 %; either improve tests or document it as a known gap).
7. **Bump the version to the next pre-1.0 milestone** (`0.3.0-beta.1` or `0.4.0-beta.1` depending on how you want to version W1–W5 completion) and publish that.
8. **Cut `1.0.0-rc.1`**, run a minimum two-week feedback window, get at least one external project to upgrade and report back, then re-score production readiness against the RC.
9. Only then cut and publish `1.0.0`.

Do not publish `1.0.0` from the current commit.
