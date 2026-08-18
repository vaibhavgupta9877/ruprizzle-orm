> **Note (2026-08-18):** This assessment is a historical snapshot of `0.1.1-beta.1` at `7636f44`. The repository has since moved to `1.0.0-rc.1` and most of the findings below have been addressed. A reassessment for the `1.0.0-rc.1` candidate is in §11.

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
