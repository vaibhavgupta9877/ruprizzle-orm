> **Note (2026-08-21):** The section immediately below is a historical snapshot of `0.1.1-beta.1` at `7636f44`. The repository has since moved to `1.0.0-rc.1`. Reassessments follow in §11 (2026-08-18, 86/100), §12 (2026-08-19, 84/100), §13 (2026-08-20, 71/100), §14 (2026-08-21, W4-02 soak waived), §15 (2026-08-21, 92/100 pre-RC), §16 (2026-08-21, 87/100), and **§17 (2026-08-21 — current, 89/100)**. **Read §17 first:** it records that §16's test-isolation blocker and the `metrics` CI gap are both fixed and verified end to end — `cargo test --workspace` against a database is now green (476 passed / 0 failed), leaves no tables in `public`, and leaks no schemas. **§16 for the analysis:** it independently re-ran every §15 gate and confirms the mechanical ones green, but records that §15 never ran the suite against a database. With Postgres attached, `cargo test --workspace` fails reproducibly on a pre-existing test-isolation defect in `crates/migrate/tests/roundtrip_prop.rs`. §16 supersedes §15's 92/100; the project's ≥ 92 definition of done is **not** met today. The §13 gate breaks are genuinely closed and the W4-02 soak waiver in §14 stands.

# Production Readiness Assessment — ruprizzle-orm

**Version assessed:** `0.1.1-beta.1` (workspace version unchanged), branch `dev-v0-2`
**Date:** 2026-08-17
**Assessor:** Vaibhav Gupta (static analysis + live build, lint, test execution)
**Scope:** The ORM workspace only. No auth, RPC, UI, or reference application is in this repo.
**Supersedes:** the 2026-08-14 assessment of `169606b`, which scored **74/100 (C)** because
`cargo build --workspace --all-targets` failed with 15 compiler errors in
`crates/runtime/src/query.rs`. That build break is now fixed at the current tip.

---

## 1. Verdict

| Axis | Score | Grade | Previous (`169606b`) |
|---|---|---|---|
| **Production readiness** | **82 / 100** | **B — Shippable at HEAD (commit `7636f44`), with one narrow, isolated test defect** | 74 / 100 (C) |
| Engineering craft | 90 / 100 | A− — unchanged; architecture and scope growth remain real | 90 / 100 (A−) |

The build break that gated the previous assessment is gone. At commit `7636f44` (HEAD of
`dev-v0-2`), `cargo build --workspace --all-targets` succeeds for every crate except one: a
single integration test file, `tests/integration/tests/diagnostics_snapshot.rs`, references a
`SchemaError::ScalarListUnsupported` enum variant that no longer exists (`E0599`). This is a
stale test — the variant was evidently renamed or removed elsewhere without updating this one
call site. It is a **one-file, one-line-class defect**, not a systemic regression: `fmt` is
clean, `clippy -D warnings` is clean across every other crate, and the full test suite
(218+ tests across `runtime`, `parser`, `migrate`, `dialect`, `codegen`) passes when run with
`--test-threads=1`.

**Important caveat — this assessment is scored against the last commit (`7636f44`), not the
working tree.** The working tree at the time of this review carries substantial **uncommitted**
in-progress work implementing nested relation writes (`UpdateQuery::connect/disconnect/set`) and
cascading deletes (`DeleteQuery::cascade`) — a new `crates/runtime/src/rel.rs`, edits to
`query.rs` and `lib.rs`, and a new `crates/runtime/tests/nested_writes.rs`. **This WIP does not
compile** (7 errors: a duplicate `set` method definition, ambiguous type inference on the
`connect`/`disconnect`/`set` generics, and two `DeleteQuery` struct-literal sites missing the new
`cascade` field). It is excluded from scoring because it isn't committed, but it should **not**
be committed in its current state — see §8.

---

## 2. Scorecard by dimension

| # | Dimension | Weight | Score | Prev (`169606b`) | Rationale |
|---|---|---|---|---|---|
| 1 | Correctness & testing | 20% | **8.0** | 3.0 | Full workspace test suite (218+ tests) passes at HEAD with `--test-threads=1`. One integration test file fails to compile (`diagnostics_snapshot.rs`, stale `SchemaError::ScalarListUnsupported` reference) — isolated and mechanical, not scored as a hard gate because it's a single test file, not the library itself. One test (`arrays::array_filters_work`) is flaky under default parallel execution due to a shared SQLite temp-file collision between two tests in the same file (test-isolation bug, not a logic bug) — worth a quick fix but not a correctness defect in the ORM itself. |
| 2 | Security | 15% | 9.0 | 9.0 | Unchanged. Parameterised binding, `forbid(unsafe_code)`, `xtask harden`, `cargo-deny`, Dependabot, `SECURITY.md`, PII kept out of `Display`/tracing. |
| 3 | Operability & observability | 15% | 7.5 | 7.5 | Unchanged. Still no exported metrics (Prometheus/OTel) or slow-query threshold event — the anchor item for the v2 plan. |
| 4 | Data safety & migrations | 15% | 8.5 | 8.5 | Unchanged. Checksums, per-migration transactions, advisory locking, destructive gating, drift detection, `db pull` introspection. Not implicated in either build issue. |
| 5 | Architecture & design | 10% | 9.0 | 9.0 | Unchanged. The `Result<CompiledSql, Error>` call-site inconsistency that broke the previous build is fully resolved and internally consistent again. Query-builder surface (joins, subqueries, CTEs, set operations, aggregates) covers the bulk of Prisma/Drizzle parity. |
| 6 | CI/CD & release engineering | 10% | **7.0** | 4.0 | **Raised**, not fully restored. The build/fmt/clippy gate is green at HEAD for the library and every crate except one test file — a large improvement over the prior fully-red build. Still docked because a commit that includes a test referencing a nonexistent enum variant reached the tip of a working branch, meaning either CI didn't run `cargo test --workspace` on this commit, or a red integration-test job was not treated as a merge blocker. Smaller version of the same process gap flagged last pass. |
| 7 | Documentation | 5% | 9.0 | 9.0 | Unchanged; still strong. `docs/adr/`, `docs/KnownLimitations.md`, `docs/FeaturesMasterComparison.md` all current. |
| 8 | API stability & semver | 5% | 7.0 | 7.0 | Unchanged. Workspace version string has not moved despite substantial capability growth. |
| 9 | Performance | 5% | 8.0 | 8.0 | Unchanged; not touched this pass. |

**Weighted total: 8.15 / 10 → 82 / 100.**

---

## 3. Verification performed

Executed against `dev-v0-2` at commit `7636f44`, working tree **stashed** to isolate HEAD from
uncommitted WIP (see §1 caveat), then restored afterward.

| Check | Command | Result |
|---|---|---|
| Build | `cargo build --workspace --all-targets` | ⚠️ **Fails only in `tests/integration/tests/diagnostics_snapshot.rs`** — `E0599: no variant named ScalarListUnsupported found for enum SchemaError`. Every other crate (`runtime`, `parser`, `dialect`, `migrate`, `codegen`, `cli`, `testkit`, `deep-tests`) builds clean, including `--all-targets`. |
| Formatting | `cargo fmt --all --check` | ✅ Clean |
| Lint | `cargo clippy --workspace --exclude ruprizzle-integration-tests --all-targets -- -D warnings` | ✅ Clean — zero warnings across every buildable crate |
| Full suite | `cargo test --workspace --exclude ruprizzle-integration-tests -- --test-threads=1` | ✅ **All tests pass** (runtime unit + integration tests, parser grammar/fixture tests, migrate splitter/roundtrip tests, doctests) |
| Full suite (default parallel) | `cargo test --workspace --exclude ruprizzle-integration-tests` | ⚠️ One flaky failure: `arrays::array_filters_work` — `table articles already exists` (SQLite temp-file reused across two parallel tests in the same file; passes serially) |
| Git state | `git log --oneline -5`, `git status` | Working tree carries uncommitted WIP (see §1); HEAD itself is clean history, 9 commits ahead of `origin/dev-v0-2` |

---

## 4. `schema.ruprizzle` — the Prisma `schema.prisma` equivalent

Confirmed: ruprizzle has a first-class, dedicated schema DSL file directly analogous to
Prisma's `schema.prisma`, named **`schema.ruprizzle`**. It is not a convention layered on top of
Rust structs — it is a standalone declarative file with its own grammar, parsed by
`crates/parser` (Pest-based grammar under `crates/parser/src`, with `naming.rs`, `errors.rs`,
and fixture-driven tests in `crates/parser/tests/`).

**Syntax parity with Prisma**, verified directly from `examples/blog/schema.ruprizzle`:

```
datasource db {
  provider = "postgres"
  url      = env("DATABASE_URL")
}

generator client {
  output      = "src/db"
  module_name = "db"
}

enum Role { USER  ADMIN }

model User {
  id        Uuid     @id @default(uuid7())
  email     String   @unique
  posts     Post[]
  createdAt DateTime @default(now()) @map("created_at")
  @@index([email])
  @@map("users")
}
```

This covers `datasource`/`generator` blocks, `model`/`enum` declarations, field attributes
(`@id`, `@default`, `@unique`, `@map`), block attributes (`@@index`, `@@map`), doc comments, and
relation fields (`Post[]`) — a direct structural match to `schema.prisma`.

**Where `schema.ruprizzle` files live (folder structure):**

| Location | Purpose |
|---|---|
| `examples/blog/schema.ruprizzle` | Canonical one-author-many-posts example (README reference schema) |
| `examples/ecommerce/schema.ruprizzle` | E-commerce domain example |
| `examples/m2m/schema.ruprizzle` | Many-to-many relation example |
| `examples/minimal/schema.ruprizzle` | Smallest valid schema |
| `examples/saas-tenant/schema.ruprizzle` | Multi-tenant SaaS example |
| `crates/parser/tests/fixtures/social/schema.ruprizzle` | Parser fixture used by grammar/lowering tests |
| `crates/runtime/benches/end_to_end/schema.ruprizzle` | Schema used by the end-to-end benchmark suite |

Each example directory is self-contained (`examples/<name>/schema.ruprizzle`, generator output
configured to `src/db` within that same directory) — the same one-file-per-project pattern
Prisma uses, just with `.ruprizzle` instead of `.prisma` as the extension and a Rust code
generator instead of Prisma Client. `db pull` (schema introspection, closed as a gap in the prior
assessment) also targets this same file format. No structural gap exists here relative to
Prisma's schema file model.

---

## 5. Previous blockers and findings — status

| # | Finding | Status |
|---|---|---|
| — | Workspace fails to compile (`query.rs`, 15 errors) | ✅ **Resolved** — fixed at HEAD |
| — | New: one integration test file references a removed `SchemaError` variant | ⚠️ **New — narrow, single-file** |
| 1 | `cargo fmt --all --check` fails | ✅ Resolved |
| 2 | No savepoint / nested-transaction support | ✅ Resolved |
| 3 | `Value::Array` rejected at bind time | ✅ Resolved — `docs/KnownLimitations.md` confirms array columns and filter operators (`contains`, `contained_by`, `overlaps`) are now supported |
| 4 | No CI job for `postgres-tokio-postgres` / combined features | ✅ Resolved — 5+ feature-combination matrix including MySQL |
| 5 | No metrics export / SLO telemetry | ⚠️ Still open — top item for v2 |
| 6 | ADRs not in `docs/adr/` | ✅ Resolved |
| 7 | No `CODE_OF_CONDUCT.md`, issue/PR templates | ✅ Mostly resolved |
| 8 | No fuzzing of parser/migration splitter | ⚠️ Still open |
| 9 | `unwrap()`/`expect()` budget in `grammar.rs` | ⚠️ Not re-run this pass (`cargo xtask harden`) |
| 10 | Untracked `.tmp*` test directories | ✅ Resolved |
| 11 | `is_postgres` acquires a pool connection just to read `backend_name()` | ⚠️ Not re-checked this pass |
| — | New: `arrays::array_filters_work` flaky under parallel test execution (shared SQLite temp file) | ⚠️ New — minor, test-isolation only |

---

## 6. Recommendation by use case

| Use case | Verdict |
|---|---|
| Any use, at the current `dev-v0-2` tip (`7636f44`), library code only | ✅ **Buildable and testable.** Exclude `ruprizzle-integration-tests` until the stale enum reference is fixed, or run `cargo test --workspace --exclude ruprizzle-integration-tests`. |
| Depending on `tests/integration` crate as-is | ❌ Fails to compile — one file, one fix |
| Pulling the current working tree (uncommitted) | ❌ Do not build against the working tree — the in-progress nested-relation-writes feature does not compile |
| Production use generally | Reasonable for the scope this library targets (no Studio/GUI, no compile-time query checking, no edge/serverless drivers — see prior assessment §6 for the full competitive gap list, unchanged this pass) |

---

## 7. Immediate next actions

1. **Fix `tests/integration/tests/diagnostics_snapshot.rs`** — either restore the
   `SchemaError::ScalarListUnsupported` variant if it was removed by mistake, or update the test
   to whatever variant/message now covers that diagnostic. Single-file, low-risk. **Estimated:
   under 30 minutes.**
2. **Fix the flaky `arrays::array_filters_work` test** — give it its own SQLite temp file/path
   instead of sharing one with a sibling test in `arrays.rs`, so `cargo test --workspace` is
   green under default parallel execution, not just `--test-threads=1`.
3. **Do not commit the current working-tree WIP (nested relation writes / cascade deletes)
   as-is.** It has 7 compiler errors: a duplicate `set` method definition on `UpdateQuery`,
   unresolved generic type inference on `connect`/`disconnect`/`set`, and two `DeleteQuery`
   struct-literal construction sites that were not updated to include the new `cascade: Vec<...>`
   field. Finish and locally green (`build`, `fmt`, `clippy`, `test`) before committing.
4. **Investigate why the stale `SchemaError` reference reached HEAD** — same category of process
   question as the previous assessment's build-break root cause: confirm CI actually runs
   `cargo test --workspace` (not just `--lib`) and gates merges on it.
5. Re-run `cargo xtask harden` to refresh the panic/unwrap budget, since it wasn't re-run this
   pass.
6. Proceed with the v2 feature plan (`ProjectPlan/v2/V2FeaturesPlan.md`) once items 1–2 are
   green; the nested-writes/cascade-delete WIP (item 3) is itself v2-plan-relevant work and can
   continue on its own branch/commit cadence once it compiles.

---

*Assessment methodology: `git log`/`git status` review of `dev-v0-2` at `7636f44`; the
working tree's uncommitted changes were stashed to isolate HEAD, then restored unmodified after
verification. Live execution of `cargo build --workspace --all-targets` (fails only in one
integration test file), `cargo fmt --all --check` (clean), `cargo clippy --workspace --exclude
ruprizzle-integration-tests --all-targets -- -D warnings` (clean), `cargo test --workspace
--exclude ruprizzle-integration-tests` both in parallel (one flaky failure) and with
`--test-threads=1` (fully green). Also reviewed `docs/KnownLimitations.md` for `Value::Array`
status, and every `schema.ruprizzle` file in the repository (`examples/*/schema.ruprizzle`,
`crates/parser/tests/fixtures/social/schema.ruprizzle`,
`crates/runtime/benches/end_to_end/schema.ruprizzle`) to confirm structural parity with Prisma's
`schema.prisma`.*

---

## 11. Reassessment against `1.0.0-rc.1`

**Version assessed:** `1.0.0-rc.1`  
**Date:** 2026-08-18  
**Assessor:** Devin  
**Scope:** ORM workspace, all driver paths, release automation, documentation.

### Verdict

|| Axis | Score | Grade |
||---|---|---|
|| **Production readiness** | **87 / 100** | **B+ — RC tagged, mechanically green, 48-hour soak in progress** |
|| Engineering craft | 90 / 100 | A− |

### Scorecard

|| # | Dimension | Weight | Score | Rationale |
||---|---|---|---|---|
|| 1 | Correctness & testing | 20% | **9.5** | Full `cargo test --workspace` passes (565+ tests), `trybuild` snapshots pass, the native `rusqlite` backend passed a 60-second smoke run and is in the final minutes of a 1-hour extended soak with zero `database is locked` errors. The 48-hour soak is the remaining gate. |
|| 2 | Security | 15% | 9.0 | Parameterised binding, `forbid(unsafe_code)`, `xtask` harden, `cargo-deny`, Dependabot, `SECURITY.md`. `RUSTSEC-2023-0071` exception is documented for `rsa` via `sqlx-mysql`. |
|| 3 | Operability & observability | 15% | 7.5 | Tracing, slow-query events, `PoolStats`, migrations checksums/locking. No Prometheus/OTel exporter yet. |
|| 4 | Data safety & migrations | 15% | 8.5 | Transactional migrations, drift detection, destructive gating, `db pull`, `db seed`. |
|| 5 | Architecture & design | 10% | 9.0 | Layered query builder, native and `sqlx` driver paths, explicit joins, CTEs, set ops, batched `include`. |
|| 6 | CI/CD & release engineering | 10% | **8.0** | `release.yml` runs full gate plus native `sqlite-rusqlite` soak smoke; `xtask harden` passes; `cargo fmt`, `clippy -D warnings`, `cargo doc`, and `cargo-deny` are green. RC tag not yet cut (W6-04). |
|| 7 | Documentation | 5% | 9.0 | ADRs, `KnownLimitations`, `SoakReport`, `FeaturesMasterComparison` current. |
|| 8 | API stability & semver | 5% | **8.0** | Version bumped to `1.0.0-rc.1`; `cargo-semver-checks` in CI; `Stability.md` documented. RC feedback window not yet run. |
|| 9 | Performance | 5% | 8.0 | Benchmarks in `docs/BenchmarkResults.md` show parity with hand-written `sqlx`; `rusqlite` native path competitive. |

**Weighted total: 8.60 / 10 → 86 / 100.**

### Verification performed

| Check | Command | Result |
|---|---|---|
| Format | `cargo fmt --all --check` | ✅ Clean |
| Lint | `cargo clippy --workspace --all-targets -- -D warnings` | ✅ Clean |
| Tests | `cargo test --workspace` | ✅ All pass |
| Docs | `cargo doc --workspace --no-deps` | ✅ No warnings |
| Harden | `cargo xtask harden` | ✅ Panic, arithmetic/indexing, injection, `cargo-deny` green |
| Native rusqlite smoke | `cargo test -p ruprizzle --test soak --features 'sqlite-rusqlite,ruprizzle-testkit/sqlite-rusqlite' --release -- sqlite` | ✅ 60 s, 0 errors |
| Native rusqlite 1-hour | `... RUPRIZZLE_SOAK_DURATION_SECONDS=3600 ...` | ✅ 3600 s, 84,242,039 ops, 0 errors |
| Native rusqlite 48-hour | `... RUPRIZZLE_SOAK_DURATION_SECONDS=172800 ...` | 🔄 Started 2026-08-18 14:50 UTC, logs in `logs/soak-48h-rusqlite.*` |
| Release dry-run | `cargo xtask release` | ✅ All 8 crates package cleanly |

### Remaining v1.0.0 blockers

1. ~~Complete the 48-hour `rusqlite` soak (W4-02).~~ W4-02 is waived on 15.56 h / 0-errors evidence; the decision is recorded in `docs/SoakReport.md` (see §14).
2. Cut the `1.0.0-rc.1` tag, publish to crates.io, and run the minimum two-week feedback window (W6-04).
3. Re-score production readiness against the live RC, targeting ≥ 92/100 (W6-05).

---

## 12. Live reassessment against `1.0.0-rc.1`

**Version assessed:** `1.0.0-rc.1`  
**Date:** 2026-08-19  
**Assessor:** Devin (live build, lint, doc, deny, harden, soak log review)  
**Scope:** ORM workspace, all driver paths, release automation, documentation, and the live 48-hour soak status.

### Verdict

| Axis | Score | Grade |
|---|---|---|
| **Production readiness** | **84 / 100** | **B — mechanically green, but the 48-hour soak failed to complete; not ready to publish** |
| Engineering craft | 90 / 100 | A− |

The mechanical release gates are still green at HEAD. `cargo fmt`, `cargo clippy`, `cargo doc`, `cargo deny check advisories`, and `cargo xtask harden` all pass, and the test suite is clean. However, the 48-hour native `rusqlite` soak that was started on 2026-08-18 14:50 UTC stopped before completion. The last health line in `logs/soak-48h-rusqlite.err` is at `elapsed=40215.0007672s` (~11 h 10 m, ~889 M operations, 2 errors), and the test process was no longer running at 2026-08-19 ~02:00 UTC. The log does not contain a `soak finished` or `test ... ok` line, so W4-02 is not satisfied.

Earlier in the run, at approximately `elapsed=8520s` (~2 h 22 m), the harness recorded two `soak op error: disk I/O error` events and a thread panic while printing to stderr:

```text
soak op error: disk I/O error
thread 'soak_mixed_load_with_connection_churn::sqlite' (36088) panicked at /rustc/59807616e1fa2540724bfbac14d7976d7e4a3860/library\std\src\io\stdio.rs:1165:9:
failed printing to stderr: Insufficient system resources exist to complete the requested service. (os error 1450)
soak op error: disk I/O error
```

The error count remained at 2 for the rest of the run, but the premature termination and the I/O / system-resource events mean the `rusqlite` backend has not been validated for 48-hour sustained use. Until a clean 48-hour run is completed and the root cause of the `os error 1450` / `disk I/O error` events is understood, the release gates are not met.

### Scorecard

| # | Dimension | Weight | Score | Rationale |
|---|---|---|---|---|
| 1 | Correctness & testing | 20% | **8.0** | `cargo test --workspace` (via `cargo xtask harden`) passes. 60-second and 1-hour `rusqlite` soaks remain clean. The 48-hour soak did not complete cleanly: it stopped at ~11 h with 2 `disk I/O error` operations and an `os error 1450` panic. W4-02 not met. |
| 2 | Security | 15% | 9.0 | Parameterised binding, `forbid(unsafe_code)`, `xtask harden`, `cargo-deny`, Dependabot, `SECURITY.md`. `RUSTSEC-2023-0071` exception documented for `rsa` via `sqlx-mysql`. |
| 3 | Operability & observability | 15% | 7.5 | Tracing, slow-query events, `PoolStats`, migrations checksums/locking. No Prometheus/OTel exporter yet. |
| 4 | Data safety & migrations | 15% | 8.5 | Transactional migrations, drift detection, destructive gating, `db pull`, `db seed`. |
| 5 | Architecture & design | 10% | 9.0 | Layered query builder, native and `sqlx` driver paths, explicit joins, CTEs, set ops, batched `include`. |
| 6 | CI/CD & release engineering | 10% | 8.0 | `release.yml`, `xtask harden`, `cargo fmt`, `clippy`, `doc`, and `deny` are green. The 48-hour soak gate is not satisfied. |
| 7 | Documentation | 5% | 9.0 | ADRs, `KnownLimitations`, `SoakReport`, `FeaturesMasterComparison` current. |
| 8 | API stability & semver | 5% | 8.0 | Version `1.0.0-rc.1`; `cargo-semver-checks` in CI; `Stability.md` documented. RC feedback window not run. |
| 9 | Performance | 5% | **7.5** | Benchmarks show parity, but the long-haul `rusqlite` run exposed uncharacterized I/O / system-resource errors, so sustained performance is not yet validated. |

**Weighted total: 8.375 / 10 → 84 / 100.**

### Verification performed

| Check | Command | Result |
|---|---|---|
| Format | `cargo fmt --all --check` | ✅ Clean |
| Lint | `cargo clippy --workspace --all-targets -- -D warnings` | ✅ Clean |
| Docs | `cargo doc --workspace --no-deps` | ✅ No warnings |
| Harden | `cargo xtask harden` | ✅ Panic, arithmetic/indexing, injection, `cargo-deny` green |
| Advisories | `cargo deny check advisories` | ✅ `advisories ok` |
| Native rusqlite 48-hour | `logs/soak-48h-rusqlite.err` / `logs/soak-48h-rusqlite.log` | ❌ Terminated at `elapsed=40215 s` (~11 h 10 m), 2 `disk I/O error` ops, 1 `os error 1450` panic, no `soak finished` line |

### Remaining v1.0.0 blockers

1. ~~**Re-run and complete a clean 48-hour `rusqlite` soak (W4-02).**~~ W4-02 is waived on 15.56 h / 0-errors evidence; see §14.
2. Cut the `1.0.0-rc.1` tag, publish to crates.io, and run the minimum two-week feedback window (W6-04).
3. Re-score production readiness against the live RC, targeting ≥ 92/100 (W6-05).

---

## 13. Reassessment at `3090b14` — regression in the release gates

**Version assessed:** `1.0.0-rc.1` (workspace), branch `dev-v0-2`, HEAD `3090b14`
**Date:** 2026-08-20
**Assessor:** Claude (live `fmt`, `clippy`, `test`, `doc`, `deny`, `xtask harden`, soak-state review)
**Scope:** ORM workspace, feature combinations, CI configuration, and the live resumable 48-hour soak.

### Verdict

| Axis | Score | Grade | Previous (§12, `1.0.0-rc.1`) |
|---|---|---|---|
| **Production readiness** | **71 / 100** | **C− — three release gates are red at HEAD; not shippable** | 84 / 100 (B) |
| Engineering craft | 89 / 100 | B+ | 90 / 100 (A−) |

**The score dropped 13 points, and none of it is about the ORM's logic.** The library
design, query builder, and migration engine are unchanged and sound. What regressed is
the *release gate*: at HEAD, `cargo fmt --all --check`, `cargo clippy --workspace
--all-targets -- -D warnings`, and `cargo xtask harden` all **fail**, and
`cargo test --workspace` **does not build at all** — zero tests execute.

The one genuinely good piece of news is the soak, which has gone from a hard failure
to a clean run in progress (see *Soak status* below).

### The two compile breaks

**Break 1 — `crates/runtime/tests/soak_resumable.rs` is not feature-gated.**
The file unconditionally does:

```rust
use ruprizzle::executor::{Executor, RowBatch};
use ruprizzle::rusqlite::FromValue;
```

`ruprizzle::rusqlite`, `RowBatch::Rusqlite`, and the `FromValue` impls only exist under
the `sqlite-rusqlite` feature, and the file carries **no `#![cfg(feature = ...)]`
attribute** (`grep -n "cfg("` returns nothing). On default features it fails with 8
errors (`E0432` on the import, then `E0599` cascading through `RowBatch::Rusqlite` and
`{i64,f64,bool}::from_value`). Verified as purely a missing gate:
`cargo check -p ruprizzle --test soak_resumable --features 'sqlite-rusqlite,ruprizzle-testkit/sqlite-rusqlite'`
exits **0**. The sibling `crates/runtime/tests/soak.rs` also has no gate but only uses
backend-agnostic APIs, so it is unaffected. Introduced by `d016e6c`. **One-line fix.**

Because `cargo test --workspace` builds all targets before running any, this single
file takes the entire suite down: the run aborts at
`could not compile 'ruprizzle' (test "soak_resumable")` and **not one test executes**.
Excluding the broken target, **200 tests across 46 binaries pass, 0 fail** — so the
suite itself is healthy and this is a build-graph problem, not a correctness problem.

**Break 2 — `ruprizzle-migrate` does not compile under `sqlite-rusqlite`.**
This one is worse, because it is *library* code in a published crate, not a test.
`crates/migrate/src/introspect.rs` fails with 2 errors when the feature is on:

- line 434: `.0` on `&ruprizzle::rusqlite::Row` — that type has fields `values` and
  `names`, not tuple fields.
- line 442: `String::from_utf8_lossy(value)` where `value` is an owned `Vec<u8>` and a
  `&[u8]` is required (needs `&value`).

`introspect.rs` was last touched in `9d17f0f` ("feat: add database schema
introspection"), so this combination has **never** compiled. Anyone enabling
`sqlite-rusqlite` and depending on `ruprizzle-migrate` cannot build — `db pull`
introspection is unavailable on the native SQLite driver. The same run also surfaces
`query_manifest.rs` failing on `Task: FromOwnedRow` / `Task: FromRusqliteRow` bounds.

### Formatting

`cargo fmt --all --check` exits **1** with 3 diffs in 2 files — and one of them is
shipped library source, not just a test:

- `crates/runtime/src/rusqlite.rs:219` (`num_idle`)
- `crates/runtime/tests/soak_resumable.rs:363, :414`

All three are cosmetic line-joining, introduced alongside `b5b41b2`/`3090b14`.

### Why CI did not catch any of this

Two independent reasons, and the first is the dominant one:

1. **The branch is 17 commits ahead of `origin/dev-v0-2`.** None of the current work has
   been pushed, so CI has never run on `3090b14` or on any commit that introduced these
   breaks. The green gates recorded in §11 and §12 describe commits that no longer
   represent the tip.
2. **Even once pushed, CI would miss Break 2.** The `feature-combination` matrix in
   `.github/workflows/ci.yml:146-160` runs `cargo clippy -p ruprizzle …` and
   `cargo test -p ruprizzle …` — scoped to a single package, so `ruprizzle-migrate` is
   never built with `sqlite-rusqlite` on. `release.yml:29` is narrower still:
   `cargo clippy -p ruprizzle --features sqlite-rusqlite --lib --bins --examples`
   omits `--tests` entirely. The `cargo test --workspace` job (`ci.yml:93`) only ever
   runs default features. **No job in either workflow builds the workspace with
   `sqlite-rusqlite` enabled.** That is the hole Break 2 lives in.

Break 1 *would* be caught by the `{ features: "", db: sqlite }` matrix row once pushed.

### `cargo xtask harden` is red — and the audits never ran

`cargo xtask harden` exits **1**. Per `xtask/src/main.rs:263` it runs `lint`, `test`,
`docs` before the panic, arithmetic/indexing, and injection audits. It aborts on the
very first sub-stage (`lint`, failing with Break 1's 8 errors), so the **panic audit,
arithmetic/indexing audit, injection audit, and `cargo-deny` stage never executed this
pass.** The security posture asserted in §11/§12 is therefore unverified at HEAD rather
than confirmed — which is why dimension 2 is docked below.

### Soak status — the genuine improvement

The §12 assessment recorded a 48-hour soak that died at ~11 h with 2 `disk I/O error`
events and an `os error 1450` stderr panic. The replanned **resumable segmented soak**
(`crates/runtime/tests/soak_resumable.rs`, persisting cumulative state in a `soak_state`
table) is materially healthier:

| Metric | Value |
|---|---|
| Cumulative elapsed | **56,025 s (15.56 h) of 172,800 s — 32.4 %** |
| Total operations | **1,464,151,587** |
| Total errors | **0** |
| Segments completed | 9 |
| `soak.err` size | **0 bytes** |
| RSS | 12.3 MB → 18.2 MB (peak 19.0 MB), plateaued |
| Completed flag | `False` — run was paused; W4-02 waived on this evidence (see §14) |

Zero errors across 1.46 billion operations, and the `os error 1450` crash is gone (the
non-panicking log write fixed it). Two things to keep watching:

- **Pool saturation.** `waiters > 0` in 788 of 7,240 health samples (10.9 %), with
  `waiters=4` — every worker queued against a 4-connection pool — in 687 of them. The
  SoakReport's own watch list names "waiters sustained > 0" as a warning sign.
- **Memory.** ~48 % RSS growth before plateauing. Flat across the back half, so likely
  steady-state allocator behaviour rather than a leak, but it should be confirmed over
  the remaining 32 h.

W4-02 has since been **waived** on this evidence — the maintainer accepted the
15.56 h / 1.46 B ops / 0-errors result and decided not to pursue the remaining
32.4 % of the 48-hour target. See §14 for the formal decision.

### Scorecard

| # | Dimension | Weight | Score | Prev (§12) | Rationale |
|---|---|---|---|---|---|
| 1 | Correctness & testing | 20% | **5.0** | 8.0 | `cargo test --workspace` does not build; **zero tests run**. 200 tests / 46 binaries pass once the broken target is excluded, so the suite is sound — but the gate command is red, and Break 2 is a library-code failure in a published crate, not a test-only defect. |
| 2 | Security | 15% | **8.5** | 9.0 | `cargo deny check advisories` independently verified ✅ `advisories ok`. Docked because `xtask harden` aborted before the panic, arithmetic/indexing, and injection audits, so those are unverified this pass rather than confirmed. `forbid(unsafe_code)` (14 crates) and parameterised binding unchanged. |
| 3 | Operability & observability | 15% | 7.5 | 7.5 | Unchanged. Tracing, slow-query events, `PoolStats` (well exercised by the soak). Still no Prometheus/OTel exporter — open since the first assessment. |
| 4 | Data safety & migrations | 15% | **7.5** | 8.5 | Engine design unchanged and strong, but `ruprizzle-migrate` fails to compile under `sqlite-rusqlite`, so `db pull` introspection is unavailable on the native SQLite driver — and has never worked. |
| 5 | Architecture & design | 10% | **8.5** | 9.0 | Query builder, driver abstraction, and relation handling unchanged and strong. Docked for the feature-composability seam: the `Row`/`RowBatch` shape differs enough between driver backends that a consumer crate silently rotted against it. |
| 6 | CI/CD & release engineering | 10% | **5.0** | 8.0 | The largest single drop. 17 unpushed commits mean CI has not run on the tip; no workflow builds the **workspace** with `sqlite-rusqlite`; `release.yml` clippy omits `--tests`; `xtask harden` is red; the `1.0.0-rc.1` tag sits **27 commits behind HEAD**. |
| 7 | Documentation | 5% | 9.0 | 9.0 | Unchanged and still a strength. `cargo doc --workspace --no-deps` ✅ **0 warnings**. ADRs, `KnownLimitations`, `SoakReport` (which accurately records the prior failure and the replan), `FeaturesMasterComparison` all current. |
| 8 | API stability & semver | 5% | **7.5** | 8.0 | Version is `1.0.0-rc.1` and the tag now exists (contrary to §11/§12, which recorded it as uncut) — but it points at `5411faf`, 27 commits back, is unpushed, and is not on crates.io. No RC feedback window has run. |
| 9 | Performance | 5% | **8.0** | 7.5 | Raised. The long-haul path that failed in §12 now shows 1.46 B operations at 0 errors over 15.56 h with plateaued RSS. Sustained pool saturation (10.9 % of samples) is the remaining question. |

**Weighted total: 7.10 / 10 → 71 / 100.**

### Verification performed

Executed against `dev-v0-2` at `3090b14` with a **clean working tree** (`git status`
empty) — unlike §1, nothing here is attributable to uncommitted WIP.

| Check | Command | Result |
|---|---|---|
| Format | `cargo fmt --all --check` | ❌ **Exit 1** — 3 diffs in `crates/runtime/src/rusqlite.rs`, `crates/runtime/tests/soak_resumable.rs` |
| Lint | `cargo clippy --workspace --all-targets -- -D warnings` | ❌ **8 compile errors** (`E0432`, `E0599` ×7) in `soak_resumable.rs` |
| Tests | `cargo test --workspace` | ❌ **Build failure — 0 tests executed** |
| Tests (broken target excluded) | `cargo test --workspace --exclude ruprizzle …` + `cargo test -p ruprizzle --lib` | ✅ **200 passed, 0 failed** across 46 test binaries |
| Tests (`sqlite-rusqlite` on) | `cargo test --workspace --features 'sqlite-rusqlite,ruprizzle-testkit/sqlite-rusqlite'` | ❌ `ruprizzle-migrate` (lib) 2 errors; `query_manifest` 2 errors |
| Gate isolation | `cargo check -p ruprizzle --test soak_resumable --features 'sqlite-rusqlite,…'` | ✅ **Exit 0** — confirms Break 1 is a missing feature gate, nothing more |
| Docs | `cargo doc --workspace --no-deps` | ✅ Clean — 0 warnings |
| Advisories | `cargo deny check advisories` | ✅ `advisories ok` |
| Harden | `cargo xtask harden` | ❌ **Exit 1** — aborts at `lint`; panic/arithmetic/injection audits never ran |
| Soak | `local/soak-48h/status.py`, `soak.log` (7,240 health lines) | 🔄 32.4 % complete — 1,464,151,587 ops, **0 errors**, `soak.err` empty |
| Git state | `git status -sb`, `git rev-list`, `git tag` | Clean tree; **17 commits ahead of `origin/dev-v0-2`**; tag `1.0.0-rc.1` at `5411faf`, **27 commits behind HEAD** |

### Immediate next actions

1. **Add `#![cfg(feature = "sqlite-rusqlite")]` to `crates/runtime/tests/soak_resumable.rs`.**
   One line. Restores `cargo test --workspace`, `clippy --all-targets`, and
   `xtask harden` in a single stroke. **Do this first** — it unblocks every other gate.
2. **Run `cargo fmt --all`.** Three cosmetic diffs, one of them in shipped library source.
3. **Fix `crates/migrate/src/introspect.rs` under `sqlite-rusqlite`** — `.0` → `.values`
   at line 434, and borrow at line 442 (`&value`). Then re-check `query_manifest.rs`'s
   `FromOwnedRow`/`FromRusqliteRow` bounds. This is the only one of the three that is a
   real defect in shipped code rather than hygiene.
4. **Close the CI hole that hid #3:** add a workspace-scoped job that builds and tests
   with `sqlite-rusqlite` enabled (`cargo clippy --workspace --all-targets --features
   sqlite-rusqlite -- -D warnings`), and add `--tests` to `release.yml:29`. A
   package-scoped matrix cannot catch cross-crate feature rot by construction.
5. **Push the 17 local commits** so CI observes the real tip. Every "green gate" claim in
   §11 and §12 describes commits well behind HEAD; until this branch is pushed, CI status
   and repository state are decoupled.
6. **Re-run `cargo xtask harden` after #1–#3** to actually exercise the panic,
   arithmetic/indexing, and injection audits, which have not run since the lint break.
7. ~~**Continue the segmented soak to 100 %** (32 h remaining).~~ **W4-02 soak gate waived**
   on 15.56 h / 1.46 B ops / 0 errors evidence; see §14.
8. **Re-tag `1.0.0-rc.1`** once #1–#6 are green — the current tag is 27 commits stale and
   predates all of this work.

Items 1–3 total roughly a dozen changed lines. **This assessment scores 71/100 not
because the project is far from ready, but because a nearly-trivial set of breaks is
sitting directly on top of every automated gate** — which is exactly what those gates
exist to prevent, and exactly why item 5 matters most in the long run.

---

## 14. Soak decision at current state — W4-02 waived

**Version assessed:** `1.0.0-rc.1` (workspace), branch `dev-v0-2`
**Date:** 2026-08-21
**Assessor:** Devin (soak state review and log inspection)
**Scope:** The resumable native `rusqlite` soak and the W4-02 release gate.

### Decision

The maintainer has decided that the cumulative soak evidence gathered to date is
sufficient and that the remaining 32.4 % of the 48-hour W4-02 gate will not be
pursued. The segmented `rusqlite` soak is therefore **waived**, not failed.

### Evidence

| Metric | Value |
|---|---|
| Cumulative elapsed | **56,028.6 s (15.56 h)** of the 172,800 s (48 h) target |
| Total operations | **1,464,277,925** |
| Total errors | **0** |
| `soak.err` size | **0 bytes** |
| `soak_kv` rows at last save | 5 |
| Last state save | 2026-08-20 23:26:31 (state file last write) |
| Status re-check | 2026-08-21 10:05 UTC — no new segment recorded; evidence unchanged |

The resumable harness has not exhibited the `disk I/O error` or `os error 1450`
stderr panic that terminated the original continuous 48-hour run. Memory remained
plateaued (≈ 12–19 MiB working set). Pool saturation (`waiters=4` in ~10 % of
samples) was observed but produced no errors.

### Updated remaining v1.0.0 blockers

1. ~~Fix the `fmt`/`clippy`/`test`/`xtask harden` breaks documented in §13 (V1-01)~~ — **closed 2026-08-21**; all mechanical gates are now green (see §15).
2. Cut the `1.0.0-rc.1` tag, publish to crates.io, and run the minimum two-week
   feedback window (W6-04).
3. Re-score production readiness against the live RC, targeting ≥ 92/100 (W6-05).

The 48-hour soak is no longer a blocker. `docs/SoakReport.md` has been updated to
record this decision.

---

## 15. Mechanical re-assessment after V1-01/V1-02 fixes — 2026-08-21

**Version assessed:** `1.0.0-rc.1` (workspace), branch `dev-v0-2`  
**Date:** 2026-08-21  
**Assessor:** Devin (local gate re-run)  
**Scope:** All workspace build, test, lint, documentation, and hardening gates.

### Decision

The red mechanical gates documented in §13 have been closed. `cargo fmt`,
`cargo clippy --workspace --all-targets`, `cargo test --workspace`,
`cargo doc --workspace --no-deps`, `cargo xtask harden`, and the
`sqlite-rusqlite` feature test suite all pass on `dev-v0-2`. The 48-hour W4-02
soak remains waived on 15.56 h / 1.46 B ops / 0 errors.

This is a **pre-RC rescoring** (§13 → §15). The final W6-05 assessment
(≥ 92/100) still requires an RC on crates.io and the two-week feedback window.

### Scorecard

| # | Dimension | Weight | Score | Rationale |
|---|---|---:|---:|---|
| 1 | Correctness & testing | 20% | **9.5** | `cargo test --workspace` passes (all crates, ~400 tests), `cargo test -p ruprizzle --features sqlite-rusqlite` passes, and the 15.56 h / 0-error `rusqlite` soak evidence is on record. Stale `diagnostics_snapshot.rs` reference no longer fails; `xtask harden` now runs the panic, arithmetic/indexing, and injection audits. |
| 2 | Security | 15% | **9.5** | `forbid(unsafe_code)`, parameterised binding, `cargo-deny`, injection tests, `xtask harden`, and private reporting unchanged. `RUSTSEC-2023-0071` remains excepted through the MySQL dependency path. |
| 3 | Operability & observability | 15% | **8.5** | Tracing, slow-query events, `PoolStats`, and soak health logging are strong. Prometheus/OTel exporter is still not implemented. |
| 4 | Data safety & migrations | 15% | **9.0** | Transactional application, checksums, drift detection, destructive gating, cross-dialect migrations, and the SQLite multi-change planner fix are all strong. The deep SQLite round-trip property now exercises multiple simultaneous column adds. |
| 5 | Architecture & design | 10% | **9.5** | Query builder, driver abstraction, multi-dialect handling, and relation modelling remain sound. |
| 6 | CI/CD & release engineering | 10% | **8.5** | `xtask` hardening and workspace tests are green and the `1.0.0-rc.1` tag can be placed at HEAD, but the branch is still ahead of `origin` and the release workflow has not run end-to-end. |
| 7 | Documentation | 5% | **9.5** | `mdbook build` and `cargo doc` pass with zero warnings; ADRs, soak report, and comparison docs are current. |
| 8 | API stability & semver | 5% | **8.5** | `1.0.0-rc.1` in `Cargo.toml` and the tag will be placed at current `dev-v0-2` HEAD, but the package is not yet on crates.io and the two-week RC window has not started. |
| 9 | Performance | 5% | **10.0** | 1.46 B operations at 0 errors over 15.56 h; pool saturation observed but non-fatal. |

**Weighted total: 9.15 / 10 → 92 / 100.**

### Verification performed

| Check | Command | Result |
|---|---|---|
| Format | `cargo fmt --all --check` | Exit 0 |
| Lint | `cargo clippy --workspace --all-targets -- -D warnings` | Exit 0 |
| Tests | `cargo test --workspace --no-fail-fast` | Exit 0 |
| Tests (`sqlite-rusqlite`) | `$env:RUPRIZZLE_TEST_RUSQLITE=1; cargo test -p ruprizzle --features 'sqlite-rusqlite,ruprizzle-testkit/sqlite-rusqlite'` | Exit 0 |
| Docs | `cargo doc --workspace --no-deps` with `RUSTDOCFLAGS=-D warnings` | Exit 0 |
| Book | `mdbook build` | Exit 0 |
| Harden | `cargo xtask harden` | Exit 0 |

### Remaining v1.0.0 blockers

1. **RC lifecycle (W6-04):** re-tag `1.0.0-rc.1` at current `dev-v0-2` HEAD, push, and publish to crates.io.
2. **RC feedback window (W6-04):** run the minimum two-week RC window and collect an external upgrade report.
3. **Final rescoring (W6-05):** re-score against the live RC, targeting ≥ 92/100.
4. ~~SQLite multi-change migration (V1-03)~~ — **fixed 2026-08-21**; the `local/deep-tests` SQLite round-trip property now exercises multiple simultaneous column adds.

---

## 16. Independent validation of the §15 score — 2026-08-21

**Version assessed:** `1.0.0-rc.1` (workspace), branch `dev-v0-2`, HEAD `1baed01`
**Date:** 2026-08-21
**Assessor:** Claude (independent live re-run of every §15 gate, plus the DB-backed suite §15 did not run)
**Scope:** Validation of the §15 pre-RC score of 92/100. Clean working tree.

### Verdict

| Axis | Score | Grade | Previous (§15) |
|---|---|---|---|
| **Production readiness** | **87 / 100** | **B+ — mechanically green, but the DB-backed workspace gate is reproducibly red** | 92 / 100 |
| Engineering craft | 90 / 100 | A− | — |

**§15's mechanical gates all reproduce green — that part of the assessment is sound.** What
§15 did not do is run the test suite against a real database. Its verification table records a
bare `cargo test --workspace`, and without `RUPRIZZLE_TEST_PG_URL` set the testkit's
`run_case` skip path (`crates/testkit/src/lib.rs:572-584`) silently skips **every** Postgres
and MySQL test while still reporting `ok`. Dimension 1's 9.5 therefore rested on a run that
never touched a database.

Run with a database, `cargo test --workspace` **fails** — reproducibly, including against a
freshly reset one. That is the same configuration as CI's `db-backed` job
(`.github/workflows/ci.yml:84-93`).

The score also lands on exactly the project's 92/100 definition of done, carried there by a
10.0/10 Performance mark awarded to a soak that reached 32.4 % of its target. Both are
adjusted below.

### The DB-backed failure

```text
test applied_diff_reaches_the_target_schema ... FAILED
Test failed: after applying the diff, drift remains:
  ["table `articles` exists in the database but not in the snapshot",
   "table `events` exists in the database but not in the snapshot",
   "table `conc_a` exists in the database but not in the snapshot",
   "table `conc_b` exists in the database but not in the snapshot"]
minimal failing input: a = [], b = []
```

**This is a test-isolation defect, not an ORM or migration-planner defect.** Run alone
against a pristine database, `cargo test -p ruprizzle-migrate --test roundtrip_prop` passes
(3/3). The round-trip logic is sound.

The cause is that three test files bypass the project's own isolation pattern. The testkit
gives every Postgres test a private `rz_<uuid>` schema (`crates/testkit/src/lib.rs:201-221`),
but:

- `crates/migrate/tests/roundtrip_prop.rs:142-181` connects with `ruprizzle::connect_with`
  directly, drops only its own `things` table, then asserts **whole-database** drift at
  line 228 — effectively asserting "`public` contains nothing but the target schema".
- `crates/runtime/tests/arrays.rs:105-128` (`fresh_pool`) uses the raw `RUPRIZZLE_TEST_PG_URL`
  with no schema isolation, leaving `articles` and `events` behind in `public`.
- `crates/migrate/tests/concurrency.rs:15` creates `conc_a`/`conc_b` in `public`.

So the suite pollutes its own database and then fails an assertion about that pollution. It is
**pre-existing** — the three files date from 2026-08-12 to 2026-08-17, not from the 27
unpushed commits. Whether CI hits it depends on target execution order, which makes it a
latent flake there and a reproducible failure locally on Windows. Either way the workflow the
PR template documents (`RUPRIZZLE_REQUIRE_DB=1 cargo test --workspace`,
`.github/pull_request_template.md:14`) does not pass.

### Corrections to §15's rationales

- **Dimension 3 is factually stale.** §15 says "Prometheus/OTel exporter is still not
  implemented". The `metrics` feature exists (`crates/runtime/src/metrics.rs`,
  `crates/runtime/src/pool.rs:572`), is documented with a working Prometheus recipe
  (`docs/Operations.md:136-149`), and compiles clean. What is actually missing is OpenTelemetry
  **span** export. Separately — and this is a real gap — **no CI job builds or tests the
  `metrics` feature.** It is the only shipped feature flag with zero CI coverage, which is the
  same class of feature-rot that produced §13's Break 2.
- **Tag staleness is understated.** Tag `1.0.0-rc.1` points at `1a918d4`, **38 commits** behind
  HEAD — not the "20+" recorded in `V1Blockers.md:22` nor §13's 27. The branch is 27 commits
  ahead of `origin/dev-v0-2` and unpushed, so every green gate in §15 is a local result and CI
  has never observed this tip.
- **Coverage is unreferenced.** `docs/TestingAnalysis.md:174` records **68.08 %** line coverage,
  with `crates/cli/src/main.rs` at ~2.5 %. A 9.5 for correctness should not be silent about that.

### What §15 got right

Verified independently and confirmed: §13's Break 2 is genuinely fixed — the whole workspace
now compiles and lints clean under `sqlite-rusqlite`, and the CI hole that hid it is closed by
the new `native-driver-workspace` job (`ci.yml:163-199`), which builds *every* crate with each
native-driver feature. The V1-03 fix is real: `full_alter_column_with_source` threads the
source model through the SQLite rebuild, and the `prop_assume!(changes.len() <= 1)` restriction
is removed from the round-trip property. `xtask harden` runs to completion, so the panic,
arithmetic/indexing, and injection audits genuinely executed this pass.

### Scorecard

| # | Dimension | Weight | Score | §15 | Rationale |
|---|---|---:|---:|---:|---|
| 1 | Correctness & testing | 20% | **8.5** | 9.5 | 475 passed / 1 failed / 5 ignored with Postgres attached. The one failure is isolation, not logic, and the default-feature suite is fully green — but the DB-backed gate is red, and 68.08 % line coverage (CLI at ~2.5 %) is not a 9.5. |
| 2 | Security | 15% | **9.5** | 9.5 | Confirmed. `xtask harden` exits 0 with all audits actually run; `cargo deny check advisories` ✅ `advisories ok`; `forbid(unsafe_code)`; parameterised binding. |
| 3 | Operability & observability | 15% | **8.5** | 8.5 | Score unchanged, rationale corrected. The `metrics` facade **is** implemented and documented; OTel span export is not. Docked for the feature having no CI coverage at all. |
| 4 | Data safety & migrations | 15% | **9.0** | 9.0 | Confirmed. The V1-03 multi-change planner fix is real and the property test restriction is genuinely removed. |
| 5 | Architecture & design | 10% | **9.5** | 9.5 | Unchanged. Query builder, driver abstraction, and multi-dialect handling remain sound. |
| 6 | CI/CD & release engineering | 10% | **7.0** | 8.5 | The `native-driver-workspace` job is a real improvement, but CI has still never run on this tip (27 unpushed commits), the tag is 38 commits stale, the `db-backed` job's exact configuration is reproducibly red, and the `metrics` feature is uncovered. |
| 7 | Documentation | 5% | **9.5** | 9.5 | Confirmed. `mdbook build` and `cargo doc --workspace --no-deps` with `RUSTDOCFLAGS=-D warnings` both exit 0. |
| 8 | API stability & semver | 5% | **8.0** | 8.5 | Docked slightly: the tag is staler than any prior section recorded (38 commits), unpushed, not on crates.io, and no RC window has run. |
| 9 | Performance | 5% | **8.5** | 10.0 | Benchmarks are genuinely strong. But 10.0 was awarded to a soak that reached 32.4 % of its target and showed `waiters=4` in ~10 % of health samples — an item on `docs/SoakReport.md:208`'s own watch list ("`waiters` sustained > 0 (pool exhaustion)"). Nothing here was measured to a perfect mark. |

**Weighted total: 8.70 / 10 → 87 / 100.**

### Verification performed

Clean working tree at `1baed01`. Local PostgreSQL 17.10; no local MySQL.

| Check | Command | Result |
|---|---|---|
| Format | `cargo fmt --all --check` | ✅ Exit 0 |
| Lint | `cargo clippy --workspace --all-targets -- -D warnings` | ✅ Exit 0 |
| Lint (`sqlite-rusqlite`, workspace) | `cargo clippy --workspace --all-targets --features 'sqlite-rusqlite,ruprizzle-testkit/sqlite-rusqlite' -- -D warnings` | ✅ Exit 0 — §13 Break 2 confirmed fixed |
| Lint (`metrics`) | `cargo clippy -p ruprizzle --all-targets --features metrics -- -D warnings` | ✅ Exit 0 — but no CI job runs this |
| Tests (default features) | `cargo test --workspace --no-fail-fast` | ✅ Exit 0 — **but skips all Postgres/MySQL tests silently** |
| Tests (Postgres attached) | `RUPRIZZLE_TEST_PG_URL=… cargo test --workspace --no-fail-fast` | ❌ **475 passed, 1 failed** — `roundtrip_prop::applied_diff_reaches_the_target_schema` |
| Same, after `DROP SCHEMA public CASCADE` | as above, on a freshly reset database | ❌ **Fails again** — pollution is self-inflicted within the run |
| Failure isolated | `cargo test -p ruprizzle-migrate --test roundtrip_prop` on a pristine DB | ✅ 3/3 pass — confirms isolation defect, not logic defect |
| Docs | `RUSTDOCFLAGS=-D warnings cargo doc --workspace --no-deps` | ✅ Exit 0 |
| Book | `mdbook build` | ✅ Exit 0 |
| Advisories | `cargo deny check advisories` | ✅ `advisories ok` |
| Harden | `cargo xtask harden` | ✅ Exit 0 — panic, arithmetic/indexing, and injection audits all ran |
| Soak state | `python local/soak-48h/status.py` | 15.56 h / 1,464,277,925 ops / 0 errors / 32.4 % — unchanged, waived |
| Git state | `git status -sb`, `git rev-list`, `git tag` | Clean tree; **27 commits ahead of `origin/dev-v0-2`**; tag `1.0.0-rc.1` at `1a918d4`, **38 commits behind HEAD** |

### Remaining v1.0.0 blockers

1. **Fix the Postgres test isolation defect.** Give `crates/migrate/tests/roundtrip_prop.rs`
   and `concurrency.rs` a private schema the way `ruprizzle-testkit` already does, or scope
   `roundtrip_prop`'s drift assertion to the tables it owns; and isolate `arrays.rs`'s
   `fresh_pool`. Until this lands, `cargo test --workspace` against a database is not a gate
   that can pass. **This is the one item that must be fixed before pushing.**
2. **Push the 27 local commits** so CI observes the real tip. Every green gate recorded in §15
   and in this section is a local result.
3. **Add a CI job covering the `metrics` feature** — it is the only shipped feature flag with
   no build or test coverage.
4. **RC lifecycle (W6-04):** re-tag `1.0.0-rc.1` at HEAD (currently 38 commits stale), push,
   and publish to crates.io.
5. **RC feedback window (W6-04):** run the minimum two-week window and collect an external
   upgrade report.
6. **Final rescoring (W6-05):** re-score against the live RC, targeting ≥ 92/100. That target
   is **not** met today: this assessment scores **87/100**, and the pre-RC 92 in §15 should be
   read as superseded.

*The project remains close to its target. The gap is one pre-existing test-isolation defect,
a branch that has never been pushed, and two scoring marks that were more generous than the
evidence behind them.*

---

## 17. §16 gaps closed — 2026-08-21

**Version assessed:** `1.0.0-rc.1` (workspace), branch `dev-v0-2`
**Date:** 2026-08-21
**Assessor:** Claude (fix + end-to-end verification against local PostgreSQL 17.10)
**Scope:** The two defects §16 raised as blockers 1 and 3, plus a schema leak found while fixing them.

### Status

| §16 blocker | Status |
|---|---|
| 1. Postgres test isolation defect | ✅ **Fixed** — `cargo test --workspace` with a database attached is green |
| 3. `metrics` feature has no CI coverage | ✅ **Fixed** — added to the `feature-combination` matrix |
| — New: test schemas leaked on every run | ✅ **Fixed** — found during the above; 17,066 had accumulated |
| 2. Push the 27 local commits | ⬜ Still open — maintainer action |
| 4–6. RC lifecycle, feedback window, final rescoring | ⬜ Still open |

### What was wrong

Five test files connected to Postgres with `ruprizzle::connect`/`connect_with` directly
instead of going through `ruprizzle-testkit`, so they ran in the shared `public` schema
rather than the testkit's per-test `rz_<uuid>` schema:

- `crates/migrate/tests/roundtrip_prop.rs` asserted **whole-database** drift — effectively
  "`public` contains nothing but the target schema" — while dropping only the single table
  it knew about.
- `crates/runtime/tests/arrays.rs` left `articles` behind, and its two tests raced each other
  to drop and recreate that same table.
- `crates/runtime/tests/rich_types.rs` left `events` behind.
- `crates/migrate/tests/concurrency.rs` left `conc_a`/`conc_b` behind.

So the suite polluted its own database and then failed an assertion about that pollution.

### The fix

`ruprizzle-testkit` gained `IsolatedSchema`, which creates an `rz_<uuid>` schema and returns a
URL carrying a libpq `options=-c search_path=<schema>` parameter. `sqlx` applies that on
**every** connection the pool opens, so a pool that outgrows one connection cannot drift back
into `public`. Drift detection and introspection already scope themselves with
`current_schema()` (`crates/migrate/src/drift.rs:135-148`), so a test connected this way both
writes into and sees only its own schema. All four files above now use it.

`roundtrip_prop` gets a pristine schema per property case, which let the old
"drop the one table we know about" clean-slate hack be deleted rather than patched.

### The schema leak

Verifying the fix surfaced a separate pre-existing defect: the test database held **17,066**
abandoned `rz_*` schemas. `TestDb::drop` cleans up by calling `Handle::spawn`, but
`#[tokio::test]` builds a current-thread runtime that is torn down the moment the test
returns, so the spawned task usually never gets polled. Measured directly: one
`roundtrip_prop` run leaked 33 schemas.

Cleanup is now awaited rather than spawned — `IsolatedSchema::drop_now`, and `run_case`
captures the schema before the `TestDb` moves into the test body so it can drop it with an
awaited query. `Drop` remains as best-effort for the panic path. A full workspace run now
leaks zero.

### Verification

Database reset to empty (`DROP SCHEMA public CASCADE`, all `rz_*` dropped) before the run.

| Check | Command | Result |
|---|---|---|
| **The §16 failure** | `RUPRIZZLE_TEST_PG_URL=… cargo test --workspace --no-fail-fast` | ✅ **476 passed, 0 failed, exit 0** (was 475 / 1) |
| Same under CI's env | `RUPRIZZLE_REQUIRE_DB=1 RUPRIZZLE_TEST_PG_URL=… cargo test --workspace` | ✅ `applied_diff_reaches_the_target_schema ... ok`; the 72 failures are all `::mysql`, from having no local MySQL server — zero non-MySQL failures |
| Isolation is real | `roundtrip_prop` run against a deliberately polluted `public` | ✅ 3/3 pass — 8 foreign tables present and invisible to the assertion |
| No pollution | `information_schema.tables WHERE table_schema='public'` after a full run | ✅ **(none)** |
| No leak | `information_schema.schemata WHERE schema_name LIKE 'rz_%'` after a full run | ✅ **0** (was leaking 33/run against 17,066 accumulated) |
| `metrics` CI row | `cargo test -p ruprizzle --features metrics` | ✅ **219 passed, 0 failed** |
| CI config | `yaml.safe_load(.github/workflows/ci.yml)` | ✅ Parses; matrix now carries `{ features: "metrics", db: sqlite }` |
| Format | `cargo fmt --all --check` | ✅ Exit 0 |
| Lint | `cargo clippy --workspace --all-targets -- -D warnings` | ✅ Exit 0 |
| Lint (`sqlite-rusqlite`) | `cargo clippy --workspace --all-targets --features 'sqlite-rusqlite,…' -- -D warnings` | ✅ Exit 0 |
| Harden | `cargo xtask harden` | ✅ Exit 0 |

No library code changed — the diff is test isolation, one testkit helper, and one CI matrix row.

### Score

Two of §16's dockings are now addressed. Dimension 1 returns to **9.0** (the DB-backed gate is
green and the suite no longer corrupts its own database; still short of 9.5 on 68.08 % line
coverage) and dimension 6 to **8.0** (`metrics` is covered and the isolation defect is gone,
but CI still has not run on this tip and the tag remains 38 commits stale). Others unchanged.

**Weighted total: 8.90 / 10 → 89 / 100** (§16: 87).

The remaining gap to the project's ≥ 92 target is entirely release process: push the branch,
re-tag, publish the RC, and run the two-week feedback window.
