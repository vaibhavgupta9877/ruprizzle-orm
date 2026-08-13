# Bug Fixes Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL — use `superpowers:test-driven-development`
> for every task in this plan, without exception. Each fix below has a **known, reproducible
> failing state**; write that reproducer as a committed test *first*, watch it fail, then
> fix. Steps use checkbox (`- [ ]`) syntax for tracking.

**Source of findings:** [`../ProjectAnalysis/PreV1/PendingBugs.md`](../ProjectAnalysis/PreV1/PendingBugs.md)
**Baseline:** `0.1.1-beta.1`, commit `af3ce27` — 218 tests passing, 0 failing
**Target:** all 15 findings closed, test count ≥ 245, no regression in benchmark numbers
**Total effort:** ~8 days

**Relationship to the v1 plan.** This plan is a **prerequisite** to
[`v1/PathToStableV1.md`](v1/PathToStableV1.md), not a parallel track. Phase 1 below must
land before W1 (savepoints) begins — W1-01 adds savepoints to the very transaction types
that BUG-01 and BUG-03 show are mismanaged, and building nested transactions on a broken
lifecycle would multiply the defect rather than contain it. Phase 1 supersedes W0 as the
immediate priority.

---

## Global constraints

Carried over from `ProductionReadinessPlan.md` and still binding:

- **MSRV 1.85.** No feature requiring a later toolchain.
- **`#![forbid(unsafe_code)]` stays in all crates.** No exceptions.
- **Zero clippy warnings** at `cargo clippy --workspace --all-targets -- -D warnings`.
- **No `unwrap()`/`expect()` in new library source.** Tests may use them freely.
- **No new panics reachable from user input.** This plan *removes* two; it adds none.
- **Dual-database parity** via the `both_dbs!` macro from `ruprizzle-testkit`.
- **Feature parity.** Every fix touching `rusqlite.rs` or `tokio_postgres.rs` must be
  verified with that feature enabled — the default build does not compile those paths, which
  is part of why these defects survived.

### Verification command

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo test -p ruprizzle --features sqlite-rusqlite
RUPRIZZLE_REQUIRE_DB=1 \
  RUPRIZZLE_TEST_PG_URL=postgres://ruprizzle:ruprizzle@localhost:5432/ruprizzle_test \
  cargo test --workspace
cargo xtask harden
```

`RUPRIZZLE_REQUIRE_DB=1` is mandatory for Phase 1 — without it an unreachable Postgres is
silently skipped and BUG-03's fix would report green while testing nothing.

---

## Phase ordering

| Phase | Contents | Effort | Rationale |
|---|---|---|---|
| **1 — Transaction lifecycle** | FIX-01, FIX-02, FIX-03, FIX-06 | 3 days | Three critical defects, one root cause. Blocks v1 W1. |
| **2 — Panic elimination & the audit gap** | FIX-05, FIX-11 | 1.5 days | Removes the remaining input-reachable panics *and* the blind spot that hid them. |
| **3 — Correctness of results** | FIX-04, FIX-08, FIX-09 | 2 days | Silent-wrong-data defects. |
| **4 — Observability & DX** | FIX-07, FIX-10 | 1 day | Restores a feature scored as already fixed. |
| **5 — Performance** | PERF-01 … PERF-05 | 1.5 days | Includes one unbounded-memory path. |

Phases 2–5 may be reordered. **Phase 1 may not be deferred.**

---

# Phase 1 — Transaction lifecycle

The three critical findings share one root cause: `sqlx::Transaction` implements `Drop` and
rolls back; the two hand-written native transaction types do not, and `Tx` does not
compensate. Fix them together so the invariant is established once.

## FIX-01 · `Drop` for `RusqliteTransaction`

*Closes BUG-01. 1 day.*

**Files:** `crates/runtime/src/rusqlite.rs`, `crates/runtime/tests/tx_lifecycle.rs` (new)

- [x] **Step 1 — Failing test first.** Create `crates/runtime/tests/tx_lifecycle.rs`. Build
      a `rusqlite` pool with `max_connections = 2`, begin and drop two transactions, then
      assert a third `begin()` succeeds. Confirm it fails today with
      `"rusqlite connection pool exhausted"`.
      *Confirmed: 4 of 7 new tests failed on the unfixed tree, three of them with exactly
      that message.*
- [x] **Step 2 — Restructure for `Drop`.** `commit`/`rollback` consume `self`, so `Drop`
      cannot tell "finished" from "abandoned". Change the fields to
      `conn: Option<Arc<Mutex<Connection>>>` and have `commit`/`rollback` `take()` it. Do not
      use `ManuallyDrop`; `Option` is clearer and has no unsafe requirement.
      *Done via a shared `finish(&mut self, stmt)`; statement methods go through
      `conn()`, which reports `"transaction already finished"` instead of panicking.*
- [x] **Step 3 — Implement `Drop`.** If `conn` is still `Some`, issue `ROLLBACK`, flush the
      prepared-statement cache to match `commit`/`rollback`, and `return_conn`. `rusqlite` is
      synchronous, so this needs no runtime handle.
- [x] **Step 4 — Never panic in `Drop`.** A failing `ROLLBACK` or a poisoned mutex must not
      unwind — `Drop` during an existing unwind would abort the process. Return the
      connection regardless and emit `tracing::warn!` on failure.
      *A poisoned mutex is recovered with `into_inner()` and rolled back anyway: honouring
      the poison flag would mean returning a connection mid-transaction, which is worse.*
- [x] **Step 5 — Observability.** Emit `tracing::warn!` with a stable message on every
      abandoned transaction. Silently rolling back is correct behaviour but a code smell in
      the caller, and it should be visible.
- [x] **Step 6 — Extend the test.** Assert: drop after statements rolls back (data absent);
      drop then reuse of the same connection works; `commit` still commits; `rollback` still
      rolls back; 100 sequential drops leave the pool at full capacity.
- [x] **Step 7 — Verify** with `cargo test -p ruprizzle --features sqlite-rusqlite`.
      *Whole suite green, including the 5 new lifecycle tests.*

**Also fixed here (found while implementing):** a failed `COMMIT` does not end the
transaction in SQLite, so the shared end-of-transaction path now issues a `ROLLBACK` before
the connection goes back to the pool. Previously that path leaked the connection instead,
which hid the hazard.

## FIX-02 · Pool exhaustion instead of divide-by-zero

*Closes BUG-02. 0.5 day.*

**Files:** `crates/runtime/src/rusqlite.rs`, `crates/runtime/tests/tx_lifecycle.rs`

- [x] **Step 1 — Failing test first.** Pool of 1, hold one transaction open, run any query.
      Confirm the panic at `rusqlite.rs:134`.
      *Confirmed: `attempt to calculate the remainder with a divisor of zero`.*
- [x] **Step 2 — Guard before the modulo.** Return the existing exhaustion error when
      `conns.is_empty()`. Do not silently create a new connection — that would make
      `max_connections` a suggestion.
      *The subsequent index is now `get(idx).cloned()` too, so no bare slice index remains
      on this path for FIX-11 to flag.*
- [x] **Step 3 — Use a typed error.** `Error::Message("rusqlite connection pool exhausted")`
      is a string today; promote it to a real variant so callers can match it and so it
      carries a stable `kind()` for telemetry. `Error` is `#[non_exhaustive]`, so adding a
      variant is not breaking.
      *Added `Error::PoolExhausted { backend: &'static str }`, `kind() == "pool_exhausted"`,
      used by both `acquire` and `begin_transaction`.*
- [x] **Step 4 — Reconsider the sharing model.** Non-transactional `acquire` round-robins a
      *cloned* `Arc` without removing it, while `begin_transaction` *pops*. The two disagree
      about what "acquire" means, which is why the pool can empty under one and not the
      other. Document the split explicitly, or unify. **Do not change the semantics in this
      task** — note the decision and open a follow-up; correctness first.
      *Decision: keep the split, documented on `acquire`. Sharing is correct for one-shot
      statements because SQLite serialises on the connection mutex anyway; unifying would
      redefine what `max_connections` means for non-transactional queries. Follow-up left
      to the v1 plan rather than changing semantics in a bug fix.*
- [x] **Step 5 — Test** exhaustion returns `Err` rather than panicking, and that the pool
      recovers once a transaction commits.
      *`an_exhausted_pool_errors_instead_of_panicking` and `beginning_past_capacity_errors`,
      both matching on the typed variant.*

## FIX-03 · `Drop` for `TokioPostgresTransaction`

*Closes BUG-03. 1 day. **Confirm the bug before fixing it.***

**Files:** `crates/runtime/src/tokio_postgres.rs`, `crates/runtime/tests/tx_lifecycle.rs`

- [x] **Step 1 — Confirm first.** BUG-03 is reasoned from source, not reproduced. Against a
      live Postgres with `--features postgres-tokio-postgres`: begin a transaction, `INSERT`,
      drop without commit, then acquire connections until the same one is reused and check
      for an open transaction (`SELECT txid_current_if_assigned()`, or observe
      `idle in transaction` in `pg_stat_activity`). **If it does not reproduce, record why
      and close the finding** — do not implement a fix for a bug that is not there.
      ***Reproduced against PostgreSQL 17.10.*** The test is behavioural rather than a
      `pg_stat_activity` probe, which is a stronger statement of the harm: after abandoning
      a transaction, a write issued through the pool was invisible to a second session,
      because it had landed inside the abandoned transaction. Observed count 0, expected 1.
- [x] **Step 2 — Async rollback from a sync `Drop`.** `ROLLBACK` is async. Capture a
      `tokio::runtime::Handle` at `begin()` and `spawn` the rollback in `Drop` before the
      `Object` is released, mirroring `sqlx::Transaction`. Handle the no-runtime case without
      panicking.
      *No-runtime case: `Object::take` detaches the connection instead of returning it.
      Losing a connection beats handing out a dirty one.*
- [x] **Step 3 — Ordering.** The rollback must complete before the connection is reusable.
      If spawning cannot guarantee that, take the `Object` into the spawned task so it is
      returned to `deadpool` only after the rollback resolves. This is the crux of the fix —
      get it right rather than quick.
      *Done by moving the `Object` into the spawned task. The regression test runs on a
      `max_connections = 1` pool, so its next checkout can only succeed once the rollback
      task has finished and released the connection — the ordering is what it asserts.*
- [x] **Step 4 — Defence in depth.** Switch the default `RecyclingMethod` from `Fast` to
      `Clean`, which discards session state on recycle. Measure the cost: it adds a round
      trip per checkout, so if it is material, gate it behind `PoolConfig`. Do not treat this
      as a substitute for Step 2.
      *Measured over 2,000 checkout+query cycles against a local PostgreSQL 17.10, release
      build: `Fast` 72–78 µs, `Verified` 143 µs, `Clean` 144–178 µs. Roughly **2×**, which
      is material for the driver that exists to cut per-query latency — so it is gated
      behind the new `PoolConfig::reset_on_recycle` (default `false`) rather than made the
      default. Step 2 is the actual fix; this is optional hardening for callers who leave
      session state behind.*
- [x] **Step 5 — Test** that a connection reused after an abandoned transaction has no open
      transaction and that the abandoned writes are absent.
      *Both asserted in one count: the abandoned write must be gone and the following write
      must be committed and visible from a separate session.*
- [x] **Step 6 — Wire into CI.** This needs the `postgres-tokio-postgres` job from the v1
      plan's W0-03. Pull that task forward into this one — the fix is untested in CI without it.
      *Added a `native-drivers` job with a Postgres service: clippy and tests for
      `sqlite-rusqlite`, for `postgres-tokio-postgres`, and for both together, with
      `RUPRIZZLE_REQUIRE_DB=1`. **No CI job compiled either native driver before this** —
      that, not the individual defects, is why all four reached a published release.*

## FIX-06 · Remove `Clone` from `RusqliteTransaction`

*Closes BUG-06. 0.5 day.*

- [x] **Step 1.** Delete the `Clone` derive at `rusqlite.rs:229`.
      *Landed with FIX-01, which had to restructure the same derive.*
- [x] **Step 2.** Fix whatever fails to compile. If an internal caller genuinely needs two
      handles, that caller is the defect — resolve it there, do not restore the derive.
      *Nothing failed: no caller cloned it, as the finding predicted.*
- [x] **Step 3.** Audit `TokioPostgresTransaction` and `Tx` for the same hazard.
      *Clean. Neither `Tx`, `TxInner`, nor `TokioPostgresTransaction` derives `Clone`; the
      remaining `Clone` derives in these modules are on the pools and on `Row`, all of
      which are meant to be shared.*
- [x] **Step 4.** Add a comment stating that a transaction owns its connection uniquely and
      must not be `Clone`, so the derive is not reintroduced.
      *On both native transaction types, not just the one that had the bug.*

**Phase 1 exit gate:** `tx_lifecycle.rs` passes on all backends — `Any`, native `sqlx`
Postgres/SQLite, `rusqlite`, and `tokio-postgres`. Abandoning a transaction on any backend
rolls back, returns the connection, and leaves the pool at full capacity.

---

# Phase 2 — Panic elimination and the audit gap

## FIX-05 · Divide-by-zero on empty insert column sets

*Closes BUG-05. 0.5 day.*

- [ ] **Step 1 — Failing test first.** `InsertManyQuery::rows(vec![vec![]])` panics at
      `query.rs:755`. Confirm, then repeat for the nested path at `query.rs:642` via
      `InsertQuery::with_related`.
- [ ] **Step 2 — Move the clamp to the divisor:** `max / cols_per_row.max(1)`. Apply at both
      sites.
- [ ] **Step 3 — Reject the input properly.** An insert with no columns is a caller mistake.
      Return an error naming it rather than emitting `INSERT INTO t DEFAULT VALUES`, which is
      almost certainly not what was meant.
- [ ] **Step 4 — Test** both sites return `Err`, not a panic.

## FIX-11 · Teach `xtask harden` about arithmetic panics

*Closes the cross-cutting gap that hid BUG-02 and BUG-05. 1 day.*

**This is the highest-leverage task in the plan.** Two of five panic-class defects passed an
audit whose whole purpose is finding panics, because it greps for `unwrap()`/`expect()` and
these were `%` and `/`.

- [ ] **Step 1.** Extend the panic audit in `xtask` to flag `/` and `%` on non-literal
      divisors in library source (excluding `tests/`, `benches/`, `examples/`).
- [ ] **Step 2.** Expect false positives — division by a provably non-zero constant is fine.
      Use the existing per-crate budget mechanism rather than a hard failure, so the count
      ratchets down without blocking on day one.
- [ ] **Step 3.** Also flag direct slice indexing (`x[i]`) in library source. `self.rows[0]`
      at `query.rs:754` is guarded today, but only by a check several lines away.
- [ ] **Step 4.** Set each crate's budget to its post-fix count so the ratchet holds.
- [ ] **Step 5.** Document the new categories in `CONTRIBUTING.md`.
- [ ] **Step 6.** Run against the full workspace and triage every hit. Fix or justify —
      no bare `#[allow]`.

**Phase 2 exit gate:** `cargo xtask harden` passes with the new categories enabled, and no
input-reachable arithmetic or indexing panic remains in library source.

---

# Phase 3 — Correctness of results

## FIX-04 · `fetch_one`/`fetch_optional` must not discard includes

*Closes BUG-04. 1 day.*

**Files:** `crates/runtime/src/query.rs`, `crates/runtime/tests/relations.rs`

- [ ] **Step 1 — Failing test first.** `.include(posts()).fetch_one()` returns a model whose
      relation `is_absent()`. Add to `relations.rs`.
- [ ] **Step 2 — Apply the existing guard.** Move `fetch_optional` and `fetch_one` from the
      generic `impl<'db, M, Out, I>` (line 63) to the `impl<'db, M, Out> …, ()>` block that
      already holds `fetch_all`, `stream`, and `page`. This makes the broken call a compile
      error, matching the design intent already documented on those three methods.
- [ ] **Step 3 — Add the include-aware equivalents.** Step 2 alone would remove the ability
      to fetch one row with relations, which is a core operation. Add `exec_optional` and
      `exec_one` to the `I: IncludeSet<M>` impl beside `exec`, loading includes via
      `self.includes.load(...)`. Reuse `exec`'s decode path rather than duplicating the
      feature-gated `rusqlite` branch.
- [ ] **Step 4 — `is_full_table` must be `false`** for these: a single-row fetch is not a
      full-table scan, and passing `true` would trigger PERF-01's whole-child-table load for
      one parent. Verify explicitly — this is an easy and expensive mistake.
- [ ] **Step 5 — Fix the misleading message.** `Related::get()` currently says *"add an
      `.include()` to the query"* — the exact thing the BUG-04 user did. Reword to also
      mention using `exec`/`exec_one` rather than `fetch_*`.
- [ ] **Step 6 — Compile-fail coverage.** Add a `trybuild` case asserting
      `.include(...).fetch_one()` no longer compiles.
- [ ] **Step 7 — Docs.** Update `docs/QueryGuide.md` and `docs/RelationsGuide.md`; this is a
      breaking API change and belongs in `CHANGELOG.md`.

## FIX-08 · `IncludeList` must handle duplicate parent keys

*Closes BUG-08. 0.5 day.*

- [ ] **Step 1 — Failing test first.** Two parents sharing a join key; assert both receive
      the children. Today the second gets an empty vec.
- [ ] **Step 2.** Replace `HashMap<Key, usize>` + `or_insert` with `HashMap<Key, Vec<usize>>`,
      pushing each child into every matching bucket.
- [ ] **Step 3.** This requires `C: Clone`. `IncludeOne` already carries that bound for the
      same reason; add it to `IncludeList` and confirm the generated client still compiles
      (`cargo xtask` generated-code gate).
- [ ] **Step 4.** Clone only when a key maps to more than one parent — the overwhelmingly
      common single-parent case must not regress. This loader is benchmarked; re-run
      `cargo bench -p ruprizzle --bench end_to_end` and compare.

## FIX-09 · Validate `InsertManyQuery` row shapes

*Closes BUG-09. 0.5 day.*

- [ ] **Step 1 — Failing test first.** Rows with differing column sets; observe the opaque
      driver error or wrong binding.
- [ ] **Step 2.** On `exec`, validate every row against row 0's column set (names and order).
- [ ] **Step 3.** Return an error naming the offending row index and the differing columns.
- [ ] **Step 4.** Keep the check O(rows × cols) with no allocation — this is the bulk-insert
      hot path and `bulk_insert_1000` is a published benchmark. Re-run it.

---

# Phase 4 — Observability and DX

## FIX-07 · Real `PoolStats` for the `rusqlite` backend

*Closes BUG-07. 0.5 day.*

- [ ] **Step 1 — Failing test first.** Assert `stats(&pool).size > 0` for a `rusqlite` pool.
      Today every field is 0.
- [ ] **Step 2.** Implement `size()` from the configured capacity and `num_idle()` from
      `inner.conns.len()`. Both are already tracked; store the capacity on `Inner` at connect
      time.
- [ ] **Step 3.** Use `try_lock` and fall back to a last-known value — a metrics call must
      never block a query.
- [ ] **Step 4.** Extend `crates/runtime/tests/pool_config.rs` to cover `PoolStats` for
      *every* `Pool` variant. The existing test only covers `Any`, which is why this was
      missed.
- [ ] **Step 5.** Cross-check with FIX-01: after the `Drop` fix, `num_idle` must return to
      capacity once abandoned transactions are collected. That assertion is the regression
      test for BUG-01 expressed as a metric.

## FIX-10 · Clear error for a driver parameter without its feature

*Closes BUG-10. 0.5 day.*

- [ ] **Step 1 — Failing test first.** In a default build, `connect` with
      `?driver=rusqlite` returns `"unknown query parameter `driver`"`.
- [ ] **Step 2.** Parse the `driver` parameter unconditionally, outside the `#[cfg]`.
- [ ] **Step 3.** When the matching feature is off, return an error naming the exact Cargo
      feature (`sqlite-rusqlite` / `postgres-tokio-postgres`). When it is on, strip the
      parameter as today.
- [ ] **Step 4.** Reject unknown `driver` values explicitly rather than silently falling
      through to the `sqlx` path — a typo'd `driver=rusqlit` should not quietly cost the user
      the performance they asked for.
- [ ] **Step 5.** Test all four combinations of feature × parameter.

---

# Phase 5 — Performance

## PERF-01 · Bound the full-table include fast path

*1 day. Treat as a correctness fix — it is an unbounded-memory path.*

- [ ] **Step 1.** Add `PoolConfig::full_table_include_limit: Option<u64>`, default
      `Some(100_000)`.
- [ ] **Step 2.** Before taking the fast path, `COUNT(*)` the child table (cheap relative to
      loading it) and fall back to chunked `IN` above the limit.
- [ ] **Step 3.** Apply at every nested level — `child_full_table` propagates the hint down,
      so the ceiling must too.
- [ ] **Step 4.** Test above and below the threshold; assert identical results on both paths.
      Equivalence is the property that matters.
- [ ] **Step 5.** Re-run the include benchmarks; document the threshold in
      `docs/Performance.md` and `docs/RelationsGuide.md`.

## PERF-02 · Cache the dialect instead of re-boxing per statement

*0.5 day.*

- [ ] **Step 1.** Benchmark first — establish the per-statement cost of
      `Box<dyn DbDialect>` before changing anything.
- [ ] **Step 2.** Cache the dialect on `Tx` at `begin()`; return a borrow.
- [ ] **Step 3.** Assess the same pattern on `Executor::dialect()`, which sits on every
      query compile. Larger change; measure before committing to it.
- [ ] **Step 4.** Confirm the gain with `cargo bench`. **Revert if it does not measure** —
      an unmeasured optimisation is churn.

## PERF-03 · `fetch_optional` should not decode rows it discards

*0.25 day.* Replace `v.remove(0)` with `into_iter().next()`, and force `limit = 1`
unconditionally rather than only when unset.

## PERF-04 · Reduce allocation in the include loader

*0.5 day.*

- [ ] **Step 1.** Benchmark first.
- [ ] **Step 2.** Avoid the per-key clone in `dedup` (`HashSet<&Key>` over a borrow, or
      sort-and-dedup where `Key: Ord`).
- [ ] **Step 3.** Consider a flat `Vec` with offsets in place of `parents.len()` separate
      `Vec`s.
- [ ] **Step 4.** Keep only what measures. This loader is a published benchmark result; do
      not regress it chasing an allocation count.

## PERF-05 · Drop the throwaway acquire in `is_postgres`

*0.25 day.* Use `Pool::provider()`, which already knows the backend without touching the
database. Carried from the readiness assessment's finding #11.

---

## Exit criteria

- [ ] All 15 findings closed or explicitly justified as won't-fix with a recorded reason.
- [ ] Every fix has a committed regression test that fails before it and passes after.
- [ ] Test count ≥ 245 (from 218).
- [ ] Full verification command green, including `--features sqlite-rusqlite` and a live
      Postgres with `RUPRIZZLE_REQUIRE_DB=1`.
- [ ] `cargo xtask harden` green with the arithmetic and indexing categories enabled.
- [ ] `cargo bench -p ruprizzle --bench end_to_end` shows no regression beyond noise; any
      change to `docs/BenchmarkResults.md` numbers is re-measured and republished.
- [ ] `CHANGELOG.md` records every fix; the FIX-04 API change is called out as breaking.
- [ ] `ProductionReadiness.md` re-run. Correctness and operability should both rise; expect
      **86–88/100**.

## Release

These fixes warrant `0.1.2-beta.1` rather than waiting for the v1 milestones. BUG-01,
BUG-02, and BUG-03 are data-integrity and availability defects in a published crate.

- [ ] Ship Phase 1 and Phase 2 as `0.1.2-beta.1` as soon as they are green — do not hold
      them behind Phases 3–5.
- [ ] Consider yanking `0.1.1-beta.1` and `0.1.0-alpha.3` if any user is on the `rusqlite`
      path. Downloads are low (43 across all versions), so the cost of yanking is near zero
      and the cost of not yanking is a silent data-integrity bug in the wild.
- [ ] Add a `docs/KnownLimitations.md` note for anyone on an affected version: on the native
      driver paths, always `commit()` or `rollback()` explicitly and never rely on drop.

---

*Effort estimates assume one experienced Rust developer. Every task begins with a failing
test because every finding in `PendingBugs.md` already has a known reproduction — writing
the reproducer first costs nothing and guarantees the fix addresses the actual defect.*
