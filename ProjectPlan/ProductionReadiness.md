# Production Readiness Assessment — ruprizzle-orm

**Version assessed:** `0.1.1-beta.1` (commit `c3ef7f0`, published to crates.io 2026-08-13)
**Date:** 2026-08-13
**Assessor:** Vaibhav Gupta (static analysis + live build, lint, test, harden, and gate execution)
**Scope:** The ORM workspace only. No auth, RPC, UI, or reference application is in this repo.
**Supersedes:** the 2026-08-11 assessment of `0.1.0-alpha.3` (commit `529c234`), which scored
**82 / 100**. That document's §5 verification log and §6 gap table remain accurate and are
carried forward here in condensed form; §8 below is the live finding list.

---

## 1. Verdict

| Axis | Score | Grade | Previous (alpha.3) |
|---|---|---|---|
| **Production readiness** | **84 / 100** | **B — Production ready for most workloads; API still beta** | 82 / 100 (B) |
| Engineering craft | 91 / 100 | A− — Substantially above 1.0 norms for the ecosystem | 90 / 100 (A−) |

The two-point move is deliberately small, and the reason matters. Between alpha.3 and
beta.1 the project did not close a blocker — there were none left to close. It shipped
**the last of the native driver paths**, **grew the suite by 21 tests across 9 more
binaries**, **made the whole workspace publishable**, and **actually published**. Those are
maturity gains, and maturity is exactly what the previous assessment said was missing. But
maturity accrues in months of exposure, not in two days of commits, so the number moves
slowly on purpose.

### What changed in `0.1.1-beta.1`

1. **The native Postgres path shipped.** `postgres-tokio-postgres` joins `sqlite-rusqlite`
   as a feature-gated backend. Both ends of the `sqlx::Any` trade-off now have a native
   escape: SQLite via `rusqlite`, Postgres via `tokio-postgres` with real type decoding for
   `Uuid`, `Decimal`, `DateTime`, `Date`, `Time`, and `Json`. §7.1's core risk — "the
   `sqlx::Any` foundation is load-bearing and expensive to reverse" — is now substantially
   defused: it is the *default*, not the *only*, path on either database.
2. **The `sqlx::Any` decision is written down.** ADR-009 (`ImplPlan10AppendixDecisions.md`)
   records the context, the decision, and the costs — per-row text serialisation, type
   inference reliance, and array rejection. Finding #3 from the previous pass is now
   **partially closed**: the ADR exists and is cited from `docs/KnownLimitations.md`; what
   is still missing is a standalone, top-level `docs/adr/` tree rather than an appendix
   section.
3. **The workspace is fully publishable.** `1301d1c` made `ruprizzle-dialect` publishable —
   it had been a path-only dependency, which would have broken the publish of every crate
   above it. All eight crates now carry real `version =` requirements on each other.
4. **The suite grew.** 218 passing tests across 55 binaries, up from 197 across 46. The
   9 new binaries are largely the `dialect` crate's tests, moved out of the integration
   suite into the crate they belong to.
5. **Beta version discipline.** `0.1.1-beta.1` replaces `0.1.0-alpha.3` across the
   workspace, the docs site, the announcement, and the benchmark comparison tables. The
   version-reference sweep (`456fabe`) was done properly rather than left to drift.

### What regressed

**`cargo fmt --all --check` now fails** on five hunks across three files
(`crates/runtime/src/rusqlite.rs`, `crates/runtime/examples/layer_attribution.rs`,
`crates/runtime/examples/pg_any_types.rs`). CI has a dedicated `fmt` job, which means the
`fmt` gate is red on the commit that was published to crates.io. This is trivial to fix —
one `cargo fmt --all` — but it is the first time in three assessments that a quality gate
this project *owns* is failing, and the mechanism the previous assessment praised ("the
honour system has been replaced with a ratchet") only works if the ratchet is watched. It
is recorded as finding #1 below and is the single cheapest item on the list.

### Why it is not scored higher

Unchanged in substance from the previous pass, and the reason the number is 84 and not 90:

1. **No production track record and no soak testing.** Correctness is well evidenced across
   218 tests, property tests, and concurrency tests. Behaviour over days of sustained load,
   connection churn, and failover is still unmeasured. 43 total downloads across four
   published versions is not exposure.
2. **Capability gaps remain.** No savepoints, no array binds, no MySQL, buffered rather
   than streaming cursors, partial many-to-many and aggregates. All honestly documented in
   `docs/KnownLimitations.md`, and all still disqualifying for some workloads.
3. **The API is beta by declaration.** `#[non_exhaustive]` closed the concrete semver
   landmine, but there is no published stability policy beyond the version number.

---

## 2. Scorecard by dimension

| # | Dimension | Weight | Score | Prev | Rationale |
|---|---|---|---|---|---|
| 1 | Correctness & testing | 20% | 8.5 | 8.5 | **218 passed, 0 failed, 4 ignored across 55 binaries** (from 197/46). Property tests on the diff engine, dedicated splitter suite, real-interleaving concurrency tests, snapshot, conformance, and `trybuild` coverage. Held at 8.5 rather than raised: the growth is breadth over the same classes of test, and the two things that would move this number — fuzzing and soak testing — are both still absent. |
| 2 | Security | 15% | 9.0 | 9.0 | Parameterised binding architecturally enforced; `forbid(unsafe_code)` across all crates; the injection audit runs in CI via `xtask harden` and passes; `cargo-deny` gates every PR; Dependabot enabled; `SECURITY.md` published; PII reaches neither `Display` nor tracing output. Unchanged and still the strongest dimension. |
| 3 | Operability & observability | 15% | 7.5 | 7.5 | Query/transaction/migration spans, tunable `PoolConfig`, `PoolStats` saturation view, readiness `ping`. Unchanged this pass. Short of 9 for the same three reasons: no exported metrics (Prometheus/OTel), no slow-query threshold event, no documented dashboard or runbook. This is now the **largest single scoring gap** and the highest-leverage target for v1. |
| 4 | Data safety & migrations | 15% | 8.5 | 8.5 | Checksums, per-migration transactions, derived advisory lock taken before the pending set is fixed, destructive gating, drift detection — all backed by property and concurrency tests. Unchanged. Held below 9 by the documented FK-cycle and heuristic-rename limitations. |
| 5 | Architecture & design | 10% | 8.5 | 8.0 | **Raised.** Both native driver paths now exist (`sqlite-rusqlite`, `postgres-tokio-postgres`), so the type-erased `Any` driver is a default rather than a constraint, and ADR-009 records the trade-off in writing. Short of 9 only because the ADRs live in a plan appendix rather than a first-class `docs/adr/` tree. |
| 6 | CI/CD & release engineering | 10% | 8.0 | 8.5 | **Lowered.** The nine jobs are unchanged and correct — fmt, clippy, three-OS test matrix, Postgres integration, generated-code gate, MSRV with tests, docs, `cargo-deny`, `harden` — but the `fmt` job is failing on the published commit (finding #1), and there is still no automated publish/release workflow. A gate that is red is worth less than a gate that is green, regardless of how well it is configured. |
| 7 | Documentation | 5% | 9.0 | 9.0 | Honest-limitations posture preserved and extended for beta; `CONTRIBUTING.md`, `SECURITY.md`, `CHANGELOG.md` present and current; `missing_docs` and `RUSTDOCFLAGS=-D warnings` enforced; the cross-ORM `FeaturesMasterComparison.md` is unusually candid about where competitors win. Held below 9.5 only for the absent operations/runbook page. |
| 8 | API stability & semver | 5% | 7.0 | 6.5 | **Raised.** `#[non_exhaustive]` holds, and the promotion from alpha to beta with a clean version sweep across nine surfaces is itself evidence of semver discipline. Still no written stability policy, which is the remaining half. |
| 9 | Performance | 5% | 8.0 | 7.5 | **Raised.** `docs/Performance.md` shows within ~5% of hand-written `sqlx` on Postgres; `docs/BenchmarkResults.md` shows the `rusqlite` path at **3.0 µs** on `select_by_pk` versus Diesel's 9.9 µs, and **1.19 ms** on `bulk_insert_1000` versus Diesel's 5.34 ms. The native Postgres path adds a second measured escape from the `Any` overhead. Still missing: concurrency/throughput curves, memory-per-row, pool contention under load. |

**Weighted total: 8.40 / 10 on craft dimensions.** Adjusted to **8.4 / 10 (84/100)** for
production readiness. The adjustment remains a small maturity discount rather than a
structural penalty — nothing is blocking.

---

## 3. Verification performed

Executed against this working tree at commit `c3ef7f0`, not inferred from source.

| Check | Command | Result |
|---|---|---|
| Formatting | `cargo fmt --all --check` | ❌ **Fails — 5 hunks across 3 files** (see finding #1) |
| Lint | `cargo clippy --workspace --all-targets -- -D warnings` | ✅ Zero warnings |
| Full suite | `cargo test --workspace` | ✅ **218 passed, 0 failed, 4 ignored** across 55 binaries |
| Panic + injection audit | `cargo xtask harden` | ✅ Complete; every crate at or under budget (`parser` 29/29, `codegen` 1/1, `migrate` 2/2, `cli` 2/2) |
| Publish state | crates.io API | ✅ `0.1.1-beta.1` live, 4 versions published, none yanked |
| Migration splitter | `crates/migrate/tests/splitter.rs` | ✅ UTF-8 and dollar-quoting regressions covered |
| Concurrent `apply_all` | `crates/migrate/tests/concurrency.rs` | ✅ Interleaving proven safe |
| Diff engine | `crates/migrate/tests/diff.rs` (proptest) | ✅ Property-tested |
| `raw!` escape hatch | `crates/runtime/tests/raw_macro.rs` + `trybuild` | ✅ Runtime and compile-fail covered |

**Codebase size:** 18,855 lines of source across 8 crates + CLI + xtask (up from 15,119);
3,103 lines of crate-level test code plus 2,104 lines of workspace integration tests —
**5,207 total, a 3.6 : 1 source-to-test ratio** (improved from 3.3 : 1).

**Note on the rustfmt failure:** rustfmt 1.9.0-stable. All five hunks are line-wrapping
differences in code added by the recent `rusqlite` and benchmark work — no semantic
content. They are listed as a finding because CI enforces the gate, not because the code
is wrong.

---

## 4. What is genuinely strong

The strengths named in the previous two assessments all hold and are not repeated:
injection prevented by construction, zero `unsafe` across all crates, a test harness honest
about skipping, generated code held to a higher standard than the generator, sound
migration fundamentals, a production-grade error taxonomy, mechanical rather than
remembered quality gates, and telemetry designed with the right instinct about secrets.

Two additions this pass:

**The driver strategy resolved into a coherent position rather than drifting.** The
previous assessment's sharpest architectural criticism was that `sqlx::Any` was
load-bearing, unmeasured, and expensive to reverse. The answer given was not to rip it out
or to defend it, but to make it the *default of three options*, measure all of them, and
write the trade-off down in ADR-009. Runtime dialect selection stays for users who want one
API across two databases; `sqlite-rusqlite` and `postgres-tokio-postgres` exist for users
who want the floor. That is a better outcome than either extreme, and it was reached by
building the alternatives rather than by arguing about them.

**Publishing was treated as an engineering problem.** `1301d1c` catching that
`ruprizzle-dialect` was path-only — before the publish, not after a failed one — is the
kind of thing that usually gets discovered by a broken `cargo publish` at 2 a.m. The
version sweep across `Cargo.toml`, the docs site, the announcement, and the comparison
tables was done in one deliberate commit rather than trickling out as bug reports.

---

## 5. Previous blockers — verification log (condensed)

All four blockers from the 2026-08-10 alpha.1 assessment remain closed and were re-verified
this pass. Full detail is in the alpha.3 assessment; summary retained for auditability:

| # | Blocker | Fix | Status |
|---|---|---|---|
| 5.1 | UTF-8 corruption in the migration splitter | `0339464` — scanner walks `char`s; `splitter.rs` suite | ✅ Closed, re-verified |
| 5.2 | Dollar-quoted bodies broken | `0339464` — `$$…$$` / `$tag$…$tag$` consumed verbatim | ✅ Closed, re-verified |
| 5.3 | No observability | `c4790dc`, `b9b2826` — `tracing` spans, `Error::kind()` | ✅ Closed, re-verified |
| 5.4 | Untunable pool | `e9a96e4`, `a8041e6` — `PoolConfig`, `PoolStats`, `ping` | ✅ Closed, re-verified |

The six significant gaps from that assessment (§6.1–6.6: stale CI job, Linux-only CI,
unwired `cargo-deny`, concurrent `migrate deploy`, no performance data, missing governance
docs) are likewise all closed or mostly closed. Only §6.6 has a residue:
`CODE_OF_CONDUCT.md`, issue/PR templates, and an automated publish workflow are still
absent — carried forward as finding #7.

---

## 6. Architectural risks worth naming

### 6.1 The `sqlx::Any` foundation — **substantially mitigated**

Previously the top architectural risk. `0.1.1-beta.1` closes most of it:

- **`sqlite-rusqlite`** decodes directly from the live `rusqlite::Row`, no text round-trip.
- **`postgres-tokio-postgres`** decodes native Postgres types directly, including
  `rust_decimal/db-tokio-postgres` for real numeric handling.
- **ADR-009** records the decision, its costs, and its exit strategy in writing.
- **`Pool`** exposes typed `as_any`, `as_sqlite`, `as_postgres`, and feature-gated
  `as_rusqlite` / `as_tokio_postgres` accessors so callers can reach the driver directly.
- The `sqlx::Executor` implementation on `&Pool` returns a clear `sqlx::Error` for native
  variants rather than panicking.

**What remains:** the ADR lives in a plan appendix, not a first-class `docs/adr/` tree, and
source comments cite ADR numbers as though such a tree exists. Cosmetic, but it makes the
decisions harder to find than they should be for a project whose main differentiator is
being legible.

### 6.2 Feature-flag combinatorics — **new, low**

Two independent backend features (`sqlite-rusqlite`, `postgres-tokio-postgres`) plus the
default `Any` path yields four meaningful build configurations. CI tests the default and,
per the alpha.3 pass, the `sqlite-rusqlite` feature. There is no job that tests
`postgres-tokio-postgres`, and none that tests both features enabled together. This is how
feature-gated code rots — not through bad code, but through untested combinations. Worth a
matrix entry before v1, and recorded as finding #4.

### 6.3–6.6 — **all previously resolved, re-verified**

`ruprizzle-macros` shipping empty (fixed in `e2c0e54`), public error enums not
`#[non_exhaustive]` (`ef8f667`), error messages echoing user data (`9d4f0fc`, `fbf40b2`),
and the hardcoded advisory-lock key (`a75059b`) all remain fixed with their regression
tests in place.

---

## 7. Competitive position

From `docs/FeaturesMasterComparison.md` and `docs/BenchmarkResults.md` (2026-08-12), the
honest read of where ruprizzle stands against the tools it is measured against:

**Where it wins today.** Fastest measured `select_by_pk` on SQLite of any tool in the
comparison (3.0 µs via `rusqlite`, versus Diesel 9.9, Drizzle 29.0, Prisma 162.3). Fastest
`bulk_insert_1000` (1.19 ms, ~4× Diesel). Auto-batched nested `include` at ~2× Sea-ORM and
~7× Prisma. Schema-first migration diffing with drift detection, which only Prisma and
prax match. `.to_sql()` on every builder, which neither Sea-ORM nor Prisma offers cheaply.
No hidden query engine or sidecar binary, unlike Prisma.

**Where it loses today.** Multi-row reads to Diesel (180 µs versus 230 µs on
`find_many_1000`). Database breadth to essentially everyone — MySQL, MSSQL, MongoDB, and
edge/serverless targets are all absent. Ecosystem maturity to all four established tools.
Lazy loading, multi-tenancy, and vector search to prax and Prisma. Streaming cursors to
everyone (ruprizzle buffers, deliberately and with a measured justification, but it is
still a gap for large result sets).

This shapes the v1 plan: the performance story is already competitive and does not need
work; the **capability surface** is where the distance lies. See
`ProjectPlan/v1/PathToStableV1.md`.

---

## 8. Open findings

| # | Finding | Location | Severity | Δ |
|---|---|---|---|---|
| 1 | `cargo fmt --all --check` fails on 5 hunks; the CI `fmt` job is red on the published beta commit. Pure line-wrapping, no semantic content. One command to fix. | `crates/runtime/src/rusqlite.rs`, `crates/runtime/examples/{layer_attribution,pg_any_types}.rs` | **Medium** | **new** |
| 2 | No savepoint or nested-transaction support; `Tx` exposes `begin` / `begin_with_isolation` / `commit` / `rollback` only. Still the most likely capability gap to disqualify a real workload. | `crates/runtime/src/tx.rs` | Medium | unchanged |
| 3 | `Value::Array` exists in the runtime enum but errors at bind time in all four encoders (`sqlx::Any`, SQLite, Postgres, `tokio_postgres`). Reads as unimplemented, not dead code. | `value.rs:132,271,318,384`, `tokio_postgres.rs:325` | Low | unchanged |
| 4 | No CI job exercises `postgres-tokio-postgres`, and none exercises both backend features together. Four meaningful build configurations, two tested. | `.github/workflows/ci.yml` | Low | **new** |
| 5 | No metrics export (Prometheus/OTel) and no slow-query threshold event; tracing spans exist but nothing turns them into an SLO. Now the largest scoring gap. | `crates/runtime/src` | Low | unchanged |
| 6 | ADRs live in a plan appendix (`ImplPlan10AppendixDecisions.md`) rather than a first-class `docs/adr/` tree, despite source comments citing bare ADR numbers. ADR-009 exists and is good; it is just hard to find. | repo-wide | Low | **downgraded** — was "no ADR at all" |
| 7 | No `CODE_OF_CONDUCT.md`, no issue/PR templates, no automated publish workflow. Publishing four versions by hand has worked so far; it will not scale past the first contributor. | `.github/` | Low | unchanged |
| 8 | No fuzzing of the parser or the migration splitter — both hand-written scanners over untrusted-ish input, and the splitter has already produced two silent-corruption defects once. | `crates/parser`, `crates/migrate` | Low | unchanged |
| 9 | 29 `unwrap()`/`expect()` in `crates/parser/src`, 27 in `grammar.rs`. Frozen at budget by `xtask harden`, so capped rather than growing, but not individually justified by comment. | `crates/parser/src/grammar.rs` | Low | unchanged |
| 10 | `local/deep-tests/db/.tmp*/` directories accumulate as untracked cruft after test runs (21 present in this working tree) and are not gitignored. | `.gitignore` | Trivial | **new** |
| 11 | `is_postgres` acquires a pool connection solely to read `backend_name()`, then drops it. | `crates/migrate/src/runner.rs` | Trivial | unchanged |

---

## 9. Path to a stable release

The previous assessment's "Path to a stable 0.1.0" is superseded. `0.1.1-beta.1` is
published and stable-quality for the workloads it supports; the meaningful next milestone
is **1.0**, which is a capability and commitment question rather than a defect question.

**Immediate (this week, ~1 day total):**

1. `cargo fmt --all` and confirm the CI `fmt` job is green (finding #1). **10 minutes.**
2. Gitignore `local/deep-tests/db/.tmp*/` (finding #10). **5 minutes.**
3. Add `postgres-tokio-postgres` and both-features jobs to the CI matrix (finding #4). **2 hours.**
4. `CODE_OF_CONDUCT.md`, issue/PR templates, and a `cargo publish` release workflow
   (finding #7). **Half a day.**

**Everything beyond that** — savepoints, array binds, metrics export, fuzzing, soak
testing, MySQL, streaming cursors, and the Prisma/Drizzle capability gaps identified in §7
— is planned in **[`ProjectPlan/v1/PathToStableV1.md`](v1/PathToStableV1.md)**, which
supersedes this section.

---

## 10. Recommendation by use case

| Use case | Verdict | Change from alpha.3 |
|---|---|---|
| Side projects, prototypes, internal tools | ✅ **Use it.** | unchanged |
| Production service, non-critical data | ✅ **Use it.** Observability, pool control, and migration safety are in place. Pin the exact version — it is beta — and read `docs/KnownLimitations.md` first. | unchanged |
| Production service, critical or regulated data | ⚠️ **Viable with care.** The migration engine is property-tested, concurrency-tested, and free of the corruption path. Reservations are the beta API, absent savepoints, and no soak-test evidence. Pilot on a non-critical service first. | unchanged |
| Latency-sensitive SQLite workloads | ✅ **Use `sqlite-rusqlite`.** Fastest `select_by_pk` and `bulk_insert_1000` in the measured comparison — ahead of hand-written Diesel on both. | unchanged |
| Latency-sensitive or rich-typed Postgres workloads | ✅ **Enable `postgres-tokio-postgres`.** Native type decoding for `Uuid`, `Decimal`, `DateTime`, `Date`, `Time`, `Json`, bypassing the `sqlx::Any` text round-trip. | **new** |
| Workloads needing savepoints, arrays, `LISTEN`/`NOTIFY`, `COPY`, or true streaming cursors | ❌ **Not supported.** Use `sqlx` directly. | unchanged |
| MySQL, MSSQL, MongoDB, or edge/serverless targets | ❌ **Not supported.** Postgres and SQLite only. See §7. | clarified |
| Evaluation against Diesel / SeaORM / sqlx / Prisma / Drizzle | ✅ **Worth evaluating,** on measured numbers rather than claims. `FeaturesMasterComparison.md` names where competitors win. Schema-first migration diffing remains genuinely differentiated. | strengthened |
| Publishing to crates.io | ✅ **Done.** Four versions live, none yanked. Fix finding #1 before the next publish. | ⚠️ → ✅ |

---

## 11. Closing assessment

Three assessments in, the pattern is consistent and worth stating plainly: this project
fixes the hard findings rather than the cheap ones. The alpha.1 pass named UTF-8 corruption
and missing observability as the two most important actions; both were done. The alpha.3
pass named the `sqlx::Any` foundation as the top architectural risk and savepoints as the
most valuable next capability; the driver risk is now substantially mitigated on both
databases with the decision written down. That is two of three, and the third is now the
top item in the v1 plan.

The single blemish this pass is that a gate the project owns is red — `cargo fmt` — on the
commit that went to crates.io. It costs one command to fix and nothing was harmed by it,
but it is worth naming precisely because the previous assessment praised the replacement of
the honour system with a ratchet. A ratchet still needs someone watching the teeth.

The library is not held back by anything it does wrong. It is held back by what it has not
yet done: run in production, be exercised by users who will find the edges, and grow the
capability surface to the point where a team choosing between it and Prisma or Drizzle is
weighing trade-offs rather than counting absences. The first is a matter of time. The
second is planned in `ProjectPlan/v1/PathToStableV1.md`.

The single most valuable next action remains **savepoint support** — the one absent
capability likely to force a user to abandon the library mid-project rather than work
around it. The second is **fuzzing the two hand-written scanners**, the one class of defect
this otherwise excellent suite is structurally unlikely to find.

---

*Assessment methodology: source review across 18,855 lines in 8 crates + CLI + xtask; live
execution of `cargo fmt --all --check` (failed, 5 hunks), `cargo clippy --workspace
--all-targets -- -D warnings` (clean), `cargo test --workspace` (218 passed, 0 failed, 4
ignored, 55 binaries), and `cargo xtask harden` (passed, all crates at or under budget) on
this working tree at commit `c3ef7f0`; crates.io API query confirming four published,
unyanked versions; commit-by-commit review of the seven commits between `529c234` and
`c3ef7f0`; review of `.github/workflows/ci.yml`, `deny.toml`, `dependabot.yml`,
`docs/FeaturesMasterComparison.md`, `docs/KnownLimitations.md`, and
`ImplPlan10AppendixDecisions.md`.*
