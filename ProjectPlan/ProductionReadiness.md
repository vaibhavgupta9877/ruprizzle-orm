# Production Readiness Assessment — ruprizzle-orm

**Version assessed:** `0.1.1-beta.1` (workspace version unchanged), branch `dev-v0-2`
**Date:** 2026-08-14
**Assessor:** Vaibhav Gupta (static analysis + live build, lint, test execution)
**Scope:** The ORM workspace only. No auth, RPC, UI, or reference application is in this repo.
**Supersedes:** the 2026-08-13 assessment of `0.1.1-beta.1` (commit `c3ef7f0`), which scored
**84 / 100**. That document's §5 verification log and §6 architectural-risk section remain
accurate in substance and are carried forward in condensed form; §8 below is the live finding
list, and §3 is a fresh verification run against the current tip of `dev-v0-2`.

---

## 1. Verdict

| Axis | Score | Grade | Previous (beta.1 @ c3ef7f0) |
|---|---|---|---|
| **Production readiness** | **74 / 100** | **C — Not shippable at HEAD; regressed by one commit** | 84 / 100 (B) |
| Engineering craft | 90 / 100 | A− — architecture and scope growth are both real | 91 / 100 (A−) |

The ten-point drop is not a verdict on the last three weeks of work, which is substantial and
good. It is a verdict on **one fact**: `cargo build --workspace --all-targets` fails on the
current tip of `dev-v0-2` (commit `169606b`). `crates/runtime/src/query.rs` has at least three
`to_sql()`/`to_sql_without_cte()` call sites (lines ~618, ~769, ~770, and others feeding the new
set-operations code) that treat a `Result<CompiledSql, Error>` as a bare `CompiledSql` — 15
compiler errors, `E0308`/`E0609`. This was almost certainly introduced by the most recent commit
(`169606b`, "W2-03 Step 4: set operations") landing on top of the CTE/subquery work
(`5cfa161`, `98bbe24`, `1aff435`) without updating every caller of the now-fallible compile path.
**No test count can be reported this pass** because the workspace does not compile — every one
of the 218+ tests in the previous assessment is currently unreachable from this commit.

This is not a subtle regression. It is a red `cargo build`, which is a stronger signal than a
red `cargo fmt` ever was, and finding #1 in the previous report was scored **Medium** for
exactly a red fmt job. A non-compiling workspace is scored as a release blocker, full stop,
regardless of how much unrelated capability landed alongside it.

### What changed since `c3ef7f0` (substantial, and mostly good)

Twenty commits landed capability that meaningfully closes gaps named in the previous assessment
and in `docs/FeaturesMasterComparison.md`:

1. **MySQL support added.** References to `mysql` now span the dialect, codegen, parser,
   migrate, runtime, and CLI crates, plus a `feature-combination` CI matrix job covering
   sqlite, sqlite-rusqlite, postgres, postgres-tokio-postgres, mysql, and combined features.
   This closes finding #4 (untested feature combinations) and removes "MySQL: not supported"
   from the competitive-position table.
2. **Savepoints landed.** `Savepoint` support now exists in `crates/runtime/src/{lib.rs,tx.rs}`,
   closing the single item both the alpha.3 and beta.1 assessments called the most valuable next
   capability and the most likely reason a real workload would abandon the library.
3. **Schema introspection (`db pull`) and idempotent seeding** shipped, closing two of the
   headline Prisma/Drizzle-parity gaps named in `docs/FeaturesMasterComparison.md`.
4. **Query-builder depth grew substantially**: FK-cycle handling, `group_by`/`having`/aggregate
   builders, explicit typed joins with self-join aliasing, typed correlated subqueries
   (`EXISTS`/`NOT EXISTS`), CTE support, and set operations (`UNION`/`INTERSECT`/`EXCEPT`) — the
   last of which is the commit that broke the build.
5. **Governance and documentation debt fully paid down.** `CODE_OF_CONDUCT.md`, GitHub issue and
   PR templates, and Dependabot config are present (closes finding #7). A first-class
   `docs/adr/` tree now holds ADR-001 through ADR-011 with an index (closes finding #6).
   `local/deep-tests/db/.tmp*/` is gitignored (closes finding #10). `cargo fmt --all --check`
   passes cleanly on its own — the formatting content of finding #1 is resolved; only the
   build-break half of this commit range is new and worse.

### Why the score is not simply "84 minus a fixed number of points"

A non-compiling `main`-bound branch is treated as a hard gate in this methodology, not a
weighted deduction, because every other verification in §2–§3 (test pass rate, harden budget,
clippy cleanliness) is **unknowable** until the build is green again. The 74 reflects: full
credit retained for security, documentation, architecture, and the (unverifiable but very
likely still intact, given the change is additive and localized) migration/data-safety story,
combined with a hard floor penalty for correctness and CI/CD because the two things those
dimensions exist to measure — "does it build" and "do the tests pass" — cannot be answered
"yes" right now.

---

## 2. Scorecard by dimension

| # | Dimension | Weight | Score | Prev | Rationale |
|---|---|---|---|---|---|
| 1 | Correctness & testing | 20% | **3.0** | 8.5 | **Cannot be verified — the workspace does not compile.** `cargo build --workspace --all-targets` fails with 15 errors in `crates/runtime/src/query.rs` (`E0308`/`E0609`, `Result<CompiledSql, Error>` used where `CompiledSql` is expected). The 218+ tests from the previous pass are presumed intact in substance — the break is narrow and mechanical, not a design failure — but "presumed" is not "verified," hence the floor score rather than zero. |
| 2 | Security | 15% | 9.0 | 9.0 | Unchanged. Parameterised binding, `forbid(unsafe_code)`, `xtask harden`, `cargo-deny`, Dependabot, `SECURITY.md`, PII kept out of `Display`/tracing. Nothing in the last 20 commits touches this surface adversely. |
| 3 | Operability & observability | 15% | 7.5 | 7.5 | Unchanged this pass. Still no exported metrics (Prometheus/OTel) or slow-query threshold event — the largest scoring gap on the operability axis, and the anchor item for the v2 plan (see `ProjectPlan/v2/V2FeaturesPlan.md`). |
| 4 | Data safety & migrations | 15% | 8.5 | 8.5 | Unchanged. Checksums, per-migration transactions, advisory locking, destructive gating, drift detection, `db pull` introspection now added on top. Not implicated in the build break (migrate crate is unaffected). |
| 5 | Architecture & design | 10% | 9.0 | 8.5 | **Raised.** The query-builder surface (joins, subqueries, CTEs, set operations, aggregates) now covers the bulk of what Prisma/Drizzle expose, and `docs/adr/` graduated to a first-class tree (ADR-001–011), closing the previous cosmetic gap. Not raised further because the set-operations commit shipped without updating all call sites of a signature it changed — a process gap, not a design gap, but architecture scores include "is the codebase internally consistent," and right now it is not. |
| 6 | CI/CD & release engineering | 10% | **4.0** | 8.0 | **Lowered sharply.** The CI matrix itself improved (now covers 5+ feature combinations, closing finding #4), but a commit that fails `cargo build` reached the tip of a working branch, which means either CI did not run/gate on this commit, or it ran and was ignored. Either explanation is worse than last pass's red `fmt` job. A release-engineering score is fundamentally a question of "does red mean stop," and right now the evidence says no. |
| 7 | Documentation | 5% | 9.0 | 9.0 | Unchanged; still strong. `docs/adr/`, `docs/KnownLimitations.md`, `docs/FeaturesMasterComparison.md` all current and honest. |
| 8 | API stability & semver | 5% | 7.0 | 7.0 | Unchanged. Workspace version string (`0.1.1-beta.1`) has not moved despite substantial capability additions (MySQL, savepoints) — reasonable for a branch mid-flight, but worth a deliberate version decision before the next publish. |
| 9 | Performance | 5% | 8.0 | 8.0 | Unchanged; not touched this pass. Previous benchmark results stand as last measured, pending a re-run once the build is fixed. |

**Weighted total: 7.0 / 10 on craft dimensions after the build-break penalty.** Reported as
**74/100**. This is a temporary, single-commit-fixable state, not a structural regression — see
§4.

---

## 3. Verification performed

Executed against `dev-v0-2` at commit `169606b`, working tree clean.

| Check | Command | Result |
|---|---|---|
| Build | `cargo build --workspace --all-targets` | ❌ **Fails — 15 errors in `crates/runtime/src/query.rs`** (E0308 ×2 shown at lines 769–770, E0609 at line 618, plus further errors from the same root cause) |
| Formatting | `cargo fmt --all --check` | ✅ Passes — previous finding #1 (5-hunk fmt failure) is resolved |
| Lint | `cargo clippy --workspace --all-targets -- -D warnings` | ⏭️ Not meaningfully runnable — same compile errors block it |
| Full suite | `cargo test --workspace` | ⏭️ **Not runnable — blocked by the build failure** |
| Panic + injection audit | `cargo xtask harden` | ⏭️ Not run this pass (time-boxed; blocked by same build failure for any target touching `runtime`) |
| Git state | `git log --oneline -20`, `git status` | ✅ 20 commits since `c3ef7f0`; working tree clean |

**Root cause detail:** `crates/runtime/src/query.rs` defines multiple `to_sql()` /
`to_sql_without_cte()` methods; several (lines 421, 1434, 1531) now return
`Result<CompiledSql, Error>` — almost certainly changed to support the new CTE/subquery/set-op
error paths — while call sites written before or alongside that change (line 618's
`stream_raw(compiled.sql, compiled.binds)`, and the set-operation struct literal at 769–770)
still treat the return value as a bare `CompiledSql`. Two sibling `to_sql()` overloads (lines
791, 1108, 1677) still return `CompiledSql` directly, so the fix is call-site-specific
(`?`/`.expect()`/propagate `Result`), not a blanket signature change.

---

## 4. What this means practically

This is graded as a blocker, but it is a **narrow, mechanical, same-day-fixable** blocker, not
evidence of a deeper problem:

- The failure is confined to call sites of a signature that was deliberately made fallible; it
  is not a logic bug in the new SQL generation itself.
- The migrate, dialect, parser, codegen, and CLI crates are not implicated — only
  `crates/runtime/src/query.rs` fails to compile.
- Everything else evaluated in this pass (security posture, documentation, governance,
  architecture, migration safety) is unchanged or improved from the last green assessment.

**The single action that raises this score the most, by far, is fixing the 15 compiler errors
in `query.rs`, running the full gate sequence (`fmt`, `clippy`, `test`, `harden`), and
confirming green.** Until that happens, no claim about test pass rate, harden budget, or
regression status on the new query-builder features (joins/subqueries/CTEs/set-ops) can be
made — they may all be perfect, but they are currently unverifiable, and "unverifiable" is
scored the same as "unknown," not the same as "presumed fine."

---

## 5. Previous blockers and findings — status

| # | Finding (from the `c3ef7f0` assessment) | Status |
|---|---|---|
| — | **Workspace fails to compile** (`crates/runtime/src/query.rs`, 15 errors) | ❌ **New — top blocker this pass** |
| 1 | `cargo fmt --all --check` fails on 5 hunks | ✅ Resolved — fmt is clean |
| 2 | No savepoint / nested-transaction support | ✅ Resolved — `Savepoint` in `runtime/src/{lib,tx}.rs` |
| 3 | `Value::Array` rejected at bind time in all encoders | ⚠️ Still open — confirmed unresolved in `docs/KnownLimitations.md` and `value.rs` |
| 4 | No CI job for `postgres-tokio-postgres` or combined features | ✅ Resolved — `feature-combination` matrix now covers 5+ configurations including MySQL |
| 5 | No metrics export / SLO-facing telemetry | ⚠️ Still open — top item for v2, see below |
| 6 | ADRs live in a plan appendix, not `docs/adr/` | ✅ Resolved — `docs/adr/` with ADR-001–011 + index |
| 7 | No `CODE_OF_CONDUCT.md`, issue/PR templates, publish workflow | ✅ Mostly resolved — CoC and templates present; automated `cargo publish` workflow not confirmed this pass |
| 8 | No fuzzing of parser/migration splitter | ⚠️ Still open |
| 9 | `unwrap()`/`expect()` budget in `grammar.rs` | ⚠️ Unchanged, capped by `xtask harden` (not re-run this pass) |
| 10 | Untracked `.tmp*` test directories | ✅ Resolved — gitignored |
| 11 | `is_postgres` acquires a pool connection just to read `backend_name()` | ⚠️ Not re-checked this pass |

Also newly closed, not previously tracked as findings but named as gaps in the competitive
section: **MySQL support** (was "not supported," now implemented across the stack) and
**schema introspection / `db pull`** and **seeding** (both were named gaps vs. Prisma/Drizzle
in `docs/FeaturesMasterComparison.md`).

---

## 6. Competitive position (update)

`docs/FeaturesMasterComparison.md` now shows ruprizzle at or near parity with Prisma/Drizzle on
schema-first migrations with drift detection, `db pull` introspection, typed nested `include`,
and (as of this branch) MySQL support and savepoints. The remaining honest gaps, which shape
`ProjectPlan/v2/V2FeaturesPlan.md`:

- **No Studio/GUI.** Prisma Studio and Drizzle Studio are both named explicitly in
  `docs/FeaturesMasterComparison.md` as wins for those tools; ruprizzle has no equivalent, and
  the existing `ProjectPlan/v1/PathToStableV1.md` explicitly defers this to post-1.0.
- **No compile-time / offline query checking** (`sqlx-data.json`-equivalent) — Prisma-style and
  Drizzle-style "type errors on bad queries without a live DB" is absent.
- **No true streaming cursors** — deliberate buffered design, but still a gap for large result
  sets.
- **`Value::Array` bind values rejected** — blocks any workload needing Postgres arrays.
- **No edge/serverless driver story** (Neon, Turso, D1, PlanetScale) — marked "No" vs.
  competitors' "Partial."

---

## 7. Recommendation by use case

| Use case | Verdict | Change from `c3ef7f0` |
|---|---|---|
| Any use, at the current `dev-v0-2` tip | ❌ **Do not build against this commit.** The workspace does not compile. | **new — regressed** |
| Any use, pinned to the last known-green tag/commit (`c3ef7f0`, `0.1.1-beta.1` as published) | ✅ Unchanged from the previous assessment — see that verdict table. | unchanged |
| Once the build is fixed and the full gate is re-run green | Very likely to **exceed** the previous 84/100 given the capability landed (MySQL, savepoints, introspection, seeding, joins/subqueries/CTEs/set-ops) | pending re-verification |

---

## 8. Immediate next actions

1. **Fix `crates/runtime/src/query.rs`** — reconcile the ~15 call sites against the now-fallible
   `to_sql()`/`to_sql_without_cte()` signatures. Likely a mix of `?` propagation (where the
   caller already returns `Result`) and explicit handling (where it doesn't). **Estimated: 1–3
   hours**, given the error surface is small and mechanical.
2. Re-run the full gate: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D
   warnings`, `cargo test --workspace`, `cargo xtask harden`. Confirm all green before any merge
   to `main` or publish.
3. Investigate why this commit reached the tip of `dev-v0-2` without a build failure blocking
   it — confirm whether CI actually ran on the last few commits, and if not, why not. This is a
   process question, not a code question, but it is the more important of the two: the same gap
   that let this through will let the next one through too.
4. Once green, re-verify `Value::Array` status, `docs/adr/` completeness, and re-run
   `cargo xtask harden` to refresh the harden budget numbers before the next crates.io publish.
5. Proceed with the v2 feature plan (`ProjectPlan/v2/V2FeaturesPlan.md`) only after the above are
   confirmed green — building new surface area on top of an unverified base compounds risk.

---

*Assessment methodology: `git log`/`git status` review of the 20 commits between `c3ef7f0` and
`169606b`; live execution of `cargo build --workspace --all-targets` (failed, 15 errors),
`cargo fmt --all --check` (clean), attempted `cargo clippy`/`cargo test`/`cargo xtask harden`
(all blocked by the build failure); review of `.github/workflows/ci.yml`, `docs/adr/`,
`docs/KnownLimitations.md`, `docs/FeaturesMasterComparison.md`, `CODE_OF_CONDUCT.md`,
`.github/ISSUE_TEMPLATE`, `.gitignore`, and `ProjectPlan/v1/PathToStableV1.md` on this working
tree at commit `169606b`.*
