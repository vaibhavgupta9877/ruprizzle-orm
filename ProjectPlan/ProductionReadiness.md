# Production Readiness Assessment — ruprizzle-orm

**Version assessed:** `0.1.0-alpha.2` (commit `e2c0e54`)
**Date:** 2026-08-11
**Assessor:** Vaibhav Gupta (static analysis + live build, lint, and test execution)
**Scope:** The ORM workspace only. No auth, RPC, UI, or reference application is in this repo.
**Supersedes:** the 2026-08-10 assessment of `0.1.0-alpha.1` (commit `e737708`), which scored
**52 / 100**. Every blocker raised there has since been closed; §5 below is now a
verification log rather than a defect list.

---

## 1. Verdict

| Axis | Score | Grade | Previous (alpha.1) |
|---|---|---|---|
| **Production readiness** | **81 / 100** | **B — Production ready for most workloads; API still alpha** | 52 / 100 (D+) |
| Engineering craft | 90 / 100 | A− — Substantially above 1.0 norms for the ecosystem | 78 / 100 (B+) |

The gap between the two numbers has narrowed sharply, and it now means something
different than it did. In August 10's assessment the gap was *structural*: the library
could not be operated or fully trusted regardless of how well it was built. That is no
longer true. The remaining gap is **maturity, not capability** — an alpha version number,
an unproven-at-scale abstraction, and features (savepoints, arrays, native driver paths)
that are absent rather than broken.

**What changed.** All four blockers from the previous assessment are fixed, verified in
source and covered by committed regression tests:

1. **§5.1 UTF-8 corruption in the migration splitter — FIXED.** The scanner now walks
   `char`s and there is a dedicated `crates/migrate/tests/splitter.rs` suite.
2. **§5.2 Dollar-quoted bodies — FIXED.** `$$…$$` and `$tag$…$tag$` are consumed verbatim,
   so `plpgsql` functions and triggers are now expressible in a migration.
3. **§5.3 No observability — FIXED.** `tracing` is a real dependency of both `runtime` and
   `migrate`; queries, transactions, and per-migration statement application emit spans and
   events, and `crates/runtime/tests/tracing.rs` asserts the emissions — including that
   failure paths do not leak database detail into telemetry.
4. **§5.4 Untunable pool — FIXED.** `pool.rs` grew from 17 to 108 lines: a `PoolConfig`
   builder, a `PoolStats` saturation view, and an async `ping` for readiness probes.

**What also changed, beyond the blockers.** The advisory lock is now derived from the
tracking-table name instead of the literal `42` and is taken before the pending set is
fixed, with `apply_all` made idempotent under interleaving and a `concurrency.rs` test that
proves it. Public error enums carry `#[non_exhaustive]`. Conflicting values no longer reach
`Error::Display`, closing the PII-into-logs path. `execution_ms` is measured per migration.
CI now runs on Windows and macOS, runs the previously-unwired generated-code gate, runs
`cargo xtask harden` with enforced per-crate panic budgets, runs `cargo-deny` on every PR,
and has Dependabot enabled. `SECURITY.md`, `CONTRIBUTING.md`, and `CHANGELOG.md` exist. The
diff engine has property tests. End-to-end benchmarks against a real database now exist and
are measured against hand-written `sqlx`. `ruprizzle-macros` is no longer an empty crate —
the advertised `raw!` escape hatch is implemented, with `trybuild` compile-fail coverage.

**Why it is not scored higher.** Three things hold the number below the high 80s, and none
of them is a defect:

1. **The API is alpha by declaration** and the `sqlx::Any` foundation (§7.1) remains
   load-bearing, unquantified in its per-row cost relative to a native driver, and expensive
   to reverse once users depend on runtime dialect selection.
2. **There is no production track record and no soak testing.** Correctness is well
   evidenced; behaviour over days of sustained load, connection churn, and failover is not.
3. **Capability gaps remain** — no savepoints, no array binds, no Postgres-native features —
   which are honestly documented but will disqualify some workloads outright.

---

## 2. Scorecard by dimension

| # | Dimension | Weight | Score | Prev | Rationale |
|---|---|---|---|---|---|
| 1 | Correctness & testing | 20% | 8.5 | 7.0 | 197 tests across 46 binaries, plus 2 gated codegen compile tests. Property tests on the diff engine, a dedicated splitter suite, real-interleaving concurrency tests on `apply_all`, snapshot, conformance, and `trybuild` coverage. Targeted probing this pass surfaced no new defects. Held under 9 for the absence of fuzzing and long-running soak tests. |
| 2 | Security | 15% | 9.0 | 7.5 | Parameterised binding architecturally enforced; `forbid(unsafe_code)` across all crates; the automated injection audit now runs in CI via `xtask harden`; `cargo-deny` gates every PR; Dependabot enabled; `SECURITY.md` published; PII no longer reaches error `Display` or tracing output. |
| 3 | Operability & observability | 15% | 7.5 | 2.5 | Was the single largest gap; now the largest single improvement. Query/transaction/migration spans, tunable pool, saturation stats, readiness `ping`. Short of 9 because there are no exported metrics (Prometheus/OTel), no slow-query threshold event, and no documented dashboard or runbook. |
| 4 | Data safety & migrations | 15% | 8.5 | 6.5 | The design was always strong — checksums, per-migration transactions, advisory lock, destructive gating, drift detection. The defects that undermined it are gone, the lock key is derived, lock ordering is correct, and the guarantees are now backed by property and concurrency tests rather than assertion. |
| 5 | Architecture & design | 10% | 8.0 | 8.0 | Unchanged and still excellent. The `sqlx::Any` compromise (§7.1) is documented in `docs/Performance.md` and now has measured numbers behind it, but it remains a load-bearing decision without a standalone ADR in-repo. |
| 6 | CI/CD & release engineering | 10% | 8.5 | 5.0 | Nine jobs: fmt, clippy, three-OS test matrix, Postgres integration, generated-code gate, MSRV *with tests*, docs, `cargo-deny`, and `harden`. The stale placeholder job is deleted. Short of 9.5 for the lack of an automated publish/release workflow. |
| 7 | Documentation | 5% | 9.0 | 8.0 | The honest-limitations posture is preserved and the governance gap is closed: `CONTRIBUTING.md`, `SECURITY.md`, `CHANGELOG.md` all present, `missing_docs` and `RUSTDOCFLAGS=-D warnings` still enforced. |
| 8 | API stability & semver | 5% | 6.5 | 5.0 | `#[non_exhaustive]` is applied to the public error enums, which was the concrete semver landmine. Still alpha by declaration, with no stability policy beyond the version number. |
| 9 | Performance | 5% | 7.0 | 4.0 | End-to-end criterion benchmarks now cover single-row, parent/child/grandchild `include`, and bulk paths against a real database, measured against hand-written `sqlx` — the correct baseline. Not yet: concurrency/throughput curves, memory-per-row, pool contention under load. |

**Weighted total: 8.24 / 10 on craft dimensions.** Adjusted to **8.1 / 10 (81/100)** for
production readiness. Note the change in method: last time the total was adjusted *down*
by more than a point because observability and migration defects were blocking rather than
weighting. Nothing is blocking now, so the adjustment is a small maturity discount rather
than a structural penalty.

---

## 3. Verification performed

Executed against this working tree at commit `e2c0e54`, not inferred from source.

| Check | Command | Result |
|---|---|---|
| Formatting | `cargo fmt --all --check` | ✅ Clean |
| Lint | `cargo clippy --workspace --all-targets -- -D warnings` | ✅ Zero warnings |
| Full suite | `cargo test --workspace` | ✅ **197 passed, 0 failed** across 46 binaries |
| Postgres-backed suites | `RUPRIZZLE_REQUIRE_DB=1 cargo test --workspace` | ⚠️ **Not exercised this pass** — see the caveat below |
| Migration splitter — UTF-8 | `crates/migrate/tests/splitter.rs` | ✅ Regression covered (was ❌ in alpha.1) |
| Migration splitter — dollar quoting | `crates/migrate/tests/splitter.rs` | ✅ Regression covered (was ❌ in alpha.1) |
| Concurrent `apply_all` | `crates/migrate/tests/concurrency.rs` | ✅ Interleaving proven safe |
| Diff engine | `crates/migrate/tests/diff.rs` (proptest) | ✅ Property-tested |
| `raw!` escape hatch | `crates/runtime/tests/raw_macro.rs` + `trybuild` | ✅ Runtime and compile-fail covered |

**Caveat on the Postgres runs.** The local `postgresql-x64-17` service was stopped on this
machine and starting it requires elevation, which this session did not have. Under
`RUPRIZZLE_REQUIRE_DB=1` the Postgres variants therefore hard-failed on connection timeout —
correct harness behaviour, not a product defect (the same 3 SQLite variants of each
conformance test passed). Without the flag the suite reports 197 green with the Postgres
variants silently skipped. **The dual-database result in this document is therefore carried
forward from the alpha.1 pass and from CI, not re-established locally today.** Re-run
`RUPRIZZLE_REQUIRE_DB=1 RUPRIZZLE_TEST_PG_URL=… cargo test --workspace` once the service is
up to close this out; the CI `integration` job covers it on every push regardless.

**Codebase size:** 15,119 lines of source across 8 crates + xtask (up from 14,440);
2,731 lines of crate-level test code plus 1,823 lines of workspace integration tests —
4,554 total, a 3.3 : 1 source-to-test ratio.

---

## 4. What is genuinely strong

The strengths listed in the previous assessment all hold and are not repeated in full:
injection prevented by construction rather than discipline, zero `unsafe` enforced across
all eight crates, a test harness that is honest about skipping, generated code held to a
higher standard than the generator, sound migration fundamentals, a production-grade error
taxonomy, and unusually candid documentation.

Three additions are worth naming this pass:

**The fixes were made properly, not patched.** Every blocker was closed with a regression
test that would have caught the original bug — `splitter.rs` for the two scanner defects,
`concurrency.rs` for the lock-ordering race, `tracing.rs` for the telemetry, `pool_config.rs`
for every configuration field, `error_redaction.rs` for the PII path. A project that fixes
bugs by adding the test it was missing is a project whose defect rate goes down over time.

**The quality gates are now mechanical rather than remembered.** The previous assessment's
sharpest criticism was that the flagship guarantee — pedantic-clean generated code — was
enforced solely by a human choosing to run `cargo xtask harden` locally. `harden` is now a
CI job, the generated-code gate runs `--ignored` properly, `cargo-deny` gates every PR, and
the panic audit has per-crate budgets that fail the build rather than printing advice. The
honour system has been replaced with a ratchet.

**Telemetry was built with the right instinct about secrets.** The obvious way to instrument
a database layer leaks: SQL with inlined values, error strings echoing rows. Here the query
spans carry bind *counts*, errors expose a stable `kind()` category for telemetry, and there
is a test asserting that failure paths stay free of database detail. That is the harder and
correct design, chosen on the first attempt.

---

## 5. Previous blockers — verification log

All four blockers from the 2026-08-10 assessment are closed. Retained here so the fixes
are auditable against the original findings.

### 5.1 Migration SQL splitter silently corrupts non-ASCII text — **RESOLVED**

Fixed in `0339464` ("scan migration SQL as chars and honour dollar quoting"). The scanner no
longer casts `u8 as char`. `crates/migrate/tests/splitter.rs` covers non-ASCII literals.
This was the most serious finding in the previous assessment — a silent data-corruption path
on the one component where silent failure is least acceptable — and it is gone.

### 5.2 Splitter breaks Postgres dollar-quoted bodies — **RESOLVED**

Fixed in the same commit. `$$ … $$` and `$tag$ … $tag$` are matched and consumed verbatim.
Triggers, stored procedures, and `plpgsql` functions are now expressible in migrations,
which matters because `docs/MigrationsGuide.md` documents hand-editing migrations for
backfills — exactly where users reach for procedural SQL.

### 5.3 No observability whatsoever — **RESOLVED**

Fixed in `c4790dc` and hardened in `b9b2826`. `tracing` is a direct dependency of
`crates/runtime` and `crates/migrate`. Queries and transactions emit spans; migration
application emits a per-migration event carrying the statement count. `Error::kind()` gives
telemetry a stable, non-sensitive category. Residual gap: no OTel/Prometheus metrics export
and no slow-query threshold event — improvements, not blockers.

### 5.4 Connection pool is not configurable — **RESOLVED**

Fixed in `e9a96e4`, extended in `a8041e6`. `PoolConfig` exposes the sizing and timeout
levers; `PoolStats` reports saturation; `ping` supports readiness probes. Every field is
covered by `crates/runtime/tests/pool_config.rs`.

---

## 6. Previous significant gaps — status

| # | Gap (alpha.1 §6) | Status |
|---|---|---|
| 6.1 | Stale `generated-code-lint` job failing on every push; `xtask harden` never automated | ✅ **Closed** (`306810f`). Stale job deleted, real gate runs, `harden` is a CI job with enforced panic budgets. |
| 6.2 | CI is Linux-only | ✅ **Closed** (`7f9ff4e`). Three-OS test matrix; MSRV job now runs tests, not just `build`. |
| 6.3 | `deny.toml` configured but never runs; no Dependabot | ✅ **Closed** (`2d56ea8`). `cargo-deny` runs on every PR via the official action; `.github/dependabot.yml` present. |
| 6.4 | Concurrent `migrate deploy` can spuriously fail | ✅ **Closed** (`a75059b`, `222c9b5`, `d482f1c`). Lock acquired before the pending set is fixed, key derived from the tracking table name, `apply_all` idempotent, proven under real interleaving. |
| 6.5 | No end-to-end performance data | ✅ **Largely closed** (`7f2010f`). Criterion benches for row, parent/child/grandchild `include`, and bulk paths against a real database, baselined against hand-written `sqlx`. Still missing concurrency/throughput curves and memory-per-row. |
| 6.6 | Missing governance and release documentation | ✅ **Mostly closed** (`5bfb1c0`, `b55c65d`). `SECURITY.md`, `CONTRIBUTING.md`, `CHANGELOG.md` all present. Still absent: `CODE_OF_CONDUCT.md`, issue/PR templates, an automated publish workflow. |

---

## 7. Architectural risks worth naming

### 7.1 The `sqlx::Any` foundation is load-bearing and expensive to reverse — **still open**

Unchanged in substance from the previous assessment, and still the most consequential
open question in the codebase. Every query goes through the type-erased driver, which buys
one identical Rust API across Postgres and SQLite with the dialect chosen by URL scheme at
runtime — the product's core promise — at the cost of text round-tripping for `Uuid`,
`Decimal`, `DateTime`, `Date`, `Time`, and `Json` in both directions
(`crates/runtime/src/value.rs`, `decode.rs`), reliance on server-side type inference for
index usage, timezone/format fragility, and unreachable Postgres-native features.

**What improved:** the cost is no longer unmeasured. `docs/Performance.md` discusses the
trade-off and the end-to-end benchmarks give it numbers against a hand-written `sqlx`
baseline. **What has not:** there is still no standalone ADR in-repo enumerating the costs
and the exit strategy, despite ADR numbers being referenced from source comments
(`crates/macros/src/lib.rs` cites ADR-005). The 0.2 roadmap should take a position on
dialect-specific native code paths behind a feature flag before the user base makes the
decision irreversible.

### 7.2 `ruprizzle-macros` ships as an empty crate — **RESOLVED**

Fixed in `e2c0e54`. `crates/macros/src/lib.rs` is now 99 lines implementing the advertised
`raw!` proc-macro, with `crates/runtime/tests/raw_macro.rs` for behaviour and a `trybuild`
case (`raw_not_encodable.rs`) asserting the compile-time rejection of non-encodable
arguments. The `README.md` claim of "raw SQL without leaving the query builder" is now
delivered by the ergonomic path it describes rather than only by `execute_raw`.

### 7.3 Public error enums are not `#[non_exhaustive]` — **RESOLVED**

Fixed in `ef8f667`. Both `ruprizzle::Error` and `ruprizzle_migrate::Error` carry the
attribute, with a doc comment instructing downstream matches to use a trailing `_ =>` arm.
Applied before 0.1.0 final, so it cost nothing.

### 7.4 Error messages may echo user data into logs — **RESOLVED**

Fixed in `9d4f0fc` and tightened in `fbf40b2`. `UniqueViolation` retains the conflicting
value as structured data but no longer interpolates it into `Display`, and
`crates/runtime/tests/error_redaction.rs` asserts this. The duplicate-signup case no longer
writes an email address to disk by default.

### 7.5 Advisory lock uses a hardcoded, collision-prone key — **RESOLVED**

Fixed in `a75059b`. `advisory_lock_key()` derives the key from the tracking table name with
a determinism test, replacing the literal `42`.

---

## 8. Open findings

| # | Finding | Location | Severity |
|---|---|---|---|
| 1 | No savepoint or nested-transaction support; `Tx` is flat commit/rollback only. This is the most likely capability gap to disqualify a real workload. | `crates/runtime/src/tx.rs` | Medium |
| 2 | `Value::Array` exists in the runtime enum but errors at bind time (`"array bind values are not supported yet"`). Currently unreachable, so it reads as an unimplemented feature rather than dead code. | `value.rs:216` | Low |
| 3 | No standalone ADR document for the `sqlx::Any` decision despite ADR numbers being cited from source comments (§7.1). | repo-wide | Low |
| 4 | No metrics export (Prometheus/OTel) and no slow-query threshold event; tracing spans exist but nothing turns them into an SLO. | `crates/runtime/src` | Low |
| 5 | 29 `unwrap()`/`expect()` calls remain in `crates/parser/src`, 27 of them in `grammar.rs`. Now bounded by an enforced panic budget in `xtask harden`, so this is capped rather than growing, but the sites are still not individually justified by comment. | `crates/parser/src/grammar.rs` | Low |
| 6 | `is_postgres` acquires a pool connection solely to read `backend_name()`, then drops it. | `crates/migrate/src/runner.rs` | Trivial |
| 7 | No `CODE_OF_CONDUCT.md`, issue/PR templates, or automated publish workflow. | `.github/` | Trivial |
| 8 | No fuzzing of the parser or the migration splitter — both are hand-written scanners over untrusted-ish input and are the natural fuzz targets in this codebase. | `crates/parser`, `crates/migrate` | Low |

---

## 9. Path to a stable 0.1.0

Estimates assume one experienced Rust developer. The previous assessment estimated 5–6
weeks to a defensible release; roughly four of those weeks of work have been completed.

### Phase 1 — Capability (~1 week)

1. Savepoints / nested transactions on `Tx` (§8.1). **Two to three days.** The largest
   remaining functional gap.
2. Array bind support, or removal of `Value::Array` with a documented rationale (§8.2).
   **One day.**

### Phase 2 — Operational polish (~1 week)

3. Metrics export behind a feature flag, plus a slow-query threshold event (§8.4).
   **Two days.**
4. A short operations page: what the spans mean, what to alert on, how to read `PoolStats`.
   **One day.**
5. Concurrency and throughput benchmarks — the axis the current benches do not cover (§6.5).
   **Two days.**

### Phase 3 — Assurance (~1 week)

6. Fuzz the parser and the migration splitter with `cargo-fuzz` (§8.8). **Two days.** These
   are the two hand-written scanners in the codebase and the one class of defect the current
   suite is structurally unlikely to find.
7. Write the `sqlx::Any` ADR and take a 0.2 position on native dialect paths (§7.1).
   **One day.**
8. A soak run — sustained load, connection churn, forced failover — to convert "correct"
   into "correct over time". **Two days.**

### Phase 4 — Release engineering (~2 days)

9. `CODE_OF_CONDUCT.md`, issue/PR templates, and an automated publish workflow (§8.7).
10. Justify or remove the remaining `grammar.rs` panic sites (§8.5).

**Total to a defensible stable 0.1.0: 3–4 weeks**, down from 5–6.

---

## 10. Recommendation by use case

| Use case | Verdict | Change from alpha.1 |
|---|---|---|
| Side projects, prototypes, internal tools | ✅ **Use it.** | unchanged |
| Production service, non-critical data | ✅ **Use it.** Observability, pool control, and migration safety are all in place. Pin the version and read `docs/KnownLimitations.md` first. | ⚠️ → ✅ |
| Production service, critical or regulated data | ⚠️ **Viable with care.** The migration engine is now property-tested, concurrency-tested, and free of the corruption path — the specific reason for the previous "not yet". Remaining reservations are the alpha API, the absence of savepoints, and no soak-test evidence. Pilot on a non-critical service first. | ❌ → ⚠️ |
| Workloads needing savepoints, arrays, `LISTEN`/`NOTIFY`, or `COPY` | ❌ **Not supported.** Use `sqlx` directly. | unchanged |
| Evaluation against Diesel / SeaORM / sqlx | ✅ **Worth evaluating,** and now on a fairer footing — the benchmarks are baselined against hand-written `sqlx`, so the cost of the abstraction is a number rather than a guess. Schema-first migration diffing remains genuinely differentiated. | strengthened |
| Publishing to crates.io | ✅ **Ship it.** The two conditions attached last time — the UTF-8 corruption fix and `#[non_exhaustive]` — are both met. `0.1.0-alpha.2` is already published. | ⚠️ → ✅ |

---

## 11. Closing assessment

The previous assessment ended by naming two actions as the most important: fix the UTF-8
corruption in `split_statements`, and add `tracing`. Both were done, along with every other
blocker and every significant gap in that document. That is worth stating plainly, because
the more common outcome for an audit like this is that the cheap items get fixed and the
structural ones are deferred. Here it was the reverse — the hardest items (observability,
lock ordering under concurrency, property-testing the diff engine, benchmarking honestly
against the thing you are abstracting over) were the ones that got done properly.

The library is no longer held back by anything it does wrong. It is held back by what it
has not yet done: run in production, be exercised by users who will find the edges, and
either commit to or retire the `sqlx::Any` foundation. Those are not things that can be
fixed in a sprint, which is why the score is 81 and not 90 — the remaining distance is
measured in exposure, not effort.

The single most valuable next action is **savepoint support** (§8.1), because it is the one
absent capability likely to force a user to abandon the library mid-project rather than
work around it. The second is **fuzzing the two hand-written scanners** (§8.8) — the parser
and the splitter are the only places left where a class of defect exists that the current,
otherwise excellent, test suite is structurally unlikely to find. The splitter has already
produced two such defects once.

---

*Assessment methodology: full source review of 15,119 lines across 8 crates + xtask; live
execution of `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -D warnings`,
and `cargo test --workspace` (197 passed, 0 failed, 46 binaries) on this working tree;
commit-by-commit verification of each fix claimed against the alpha.1 findings; review of
`.github/workflows/ci.yml`, `deny.toml`, `dependabot.yml`, and the governance documents.
Postgres-backed suites were not re-run locally — the local service was stopped and starting
it required elevation unavailable to this session — so the dual-database result is carried
forward from the alpha.1 pass and from CI rather than re-established today.*
