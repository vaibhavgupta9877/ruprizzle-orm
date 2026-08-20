> **Note (2026-08-20):** The section immediately below is a historical snapshot of `0.1.1-beta.1` at `7636f44`. The repository has since moved to `1.0.0-rc.1`. Reassessments follow in §11 (2026-08-18, 86/100), §12 (2026-08-19, 84/100), and **§13 (2026-08-20, 71/100 — current)**. **Read §13 first:** it supersedes the others, and it downgrades the score because three release gates (`fmt`, `clippy`/`test`, `xtask harden`) are red at HEAD `3090b14`.

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

1. Complete the 48-hour `rusqlite` soak (W4-02) and record final results in `docs/SoakReport.md`.
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

1. **Re-run and complete a clean 48-hour `rusqlite` soak (W4-02).** Investigate the `disk I/O error` and `Insufficient system resources exist to complete the requested service. (os error 1450)` events before restarting.
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
| Completed flag | `False` — run is ongoing |

Zero errors across 1.46 billion operations, and the `os error 1450` crash is gone (the
non-panicking log write fixed it). Two things to keep watching:

- **Pool saturation.** `waiters > 0` in 788 of 7,240 health samples (10.9 %), with
  `waiters=4` — every worker queued against a 4-connection pool — in 687 of them. The
  SoakReport's own watch list names "waiters sustained > 0" as a warning sign.
- **Memory.** ~48 % RSS growth before plateauing. Flat across the back half, so likely
  steady-state allocator behaviour rather than a leak, but it should be confirmed over
  the remaining 32 h.

W4-02 remains **unmet** — 32.4 % of the gate is not the gate — but it is now on a
credible path rather than blocked on an unexplained failure.

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
7. **Continue the segmented soak to 100 %** (32 h remaining). Investigate the sustained
   `waiters=4` pool saturation and confirm RSS stays plateaued.
8. **Re-tag `1.0.0-rc.1`** once #1–#6 are green — the current tag is 27 commits stale and
   predates all of this work.

Items 1–3 total roughly a dozen changed lines. **This assessment scores 71/100 not
because the project is far from ready, but because a nearly-trivial set of breaks is
sitting directly on top of every automated gate** — which is exactly what those gates
exist to prevent, and exactly why item 5 matters most in the long run.
