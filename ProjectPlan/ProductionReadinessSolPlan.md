# Stable v1 Production Readiness Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use `superpowers:subagent-driven-development` (recommended) or `superpowers:executing-plans` to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Move `ruprizzle` from a 67/100 strong beta with red release gates to a defensible stable `1.0.0` by fixing confirmed correctness/ownership defects, producing trustworthy assurance evidence, and executing the documented RC process.

**Architecture:** Preserve the existing parser → core IR → dialect → codegen → runtime layering. Work is ordered by risk: first restore deterministic gates, then repair data/lifetime correctness, then complete long-duration and test-quality evidence, then align telemetry/performance/documentation, and finally publish and validate the RC. No broad feature work enters the RC line.

**Tech Stack:** Rust 2024, MSRV Rust 1.85, `sqlx` 0.8, optional `rusqlite` 0.32 and `tokio-postgres` 0.7 drivers, Tokio, Pest, `cargo-deny`, `cargo-semver-checks`, `cargo-mutants`, `cargo-fuzz`, `cargo-llvm-cov`, GitHub Actions, mdBook.

## Global Constraints

- The release branch remains based on `dev-v0-2` unless the maintainer explicitly changes repository policy.
- Every crate continues to compile on Rust 1.85.
- `#![forbid(unsafe_code)]` remains in library crates; no ownership fix may introduce unsafe code.
- Do not increase `PANIC_BUDGET` or arithmetic/indexing budgets in `xtask`.
- Every value reaching SQL remains a bind parameter; no user value is interpolated into SQL.
- Do not add a dependency unless existing Rust/std/project primitives cannot solve the problem; pin a version published for at least seven days.
- The stable API does not gain unrelated capabilities during this plan.
- Once an RC is published, only defect, assurance, documentation, and necessary API-correction work enters that RC line.
- PostgreSQL/MySQL evidence is valid only when `RUPRIZZLE_REQUIRE_DB=1` makes unavailable databases fail rather than skip.
- Long-running soak/fuzz/mutation jobs are explicit gates; they must never execute accidentally as part of a normal contributor test run.
- Never move, delete, or recreate an existing Git tag without explicit maintainer confirmation.
- Do not publish or push as part of an implementation task unless the maintainer explicitly approves that side effect.

---

## Baseline and target

### Current verified baseline at `dev-v0-2` HEAD (post-V1-01/V1-02)

| Gate | State |
|---|---|
| `cargo fmt --all --check` | Pass |
| Default workspace clippy | Pass |
| `cargo test --workspace` | Pass |
| `cargo test -p ruprizzle --features 'sqlite-rusqlite,ruprizzle-testkit/sqlite-rusqlite'` | Pass |
| Rustdoc with warnings denied | Pass |
| `cargo deny check` | Pass under current exception policy |
| `cargo check --workspace` | Pass |
| `cargo xtask harden` | Pass |
| `sqlite-rusqlite` feature suite | Pass |
| 48-hour soak | Waived after 15.56 cumulative hours, 1.46 B ops, 0 errors (see `docs/SoakReport.md`) |
| SQLite multi-change migration property | Now exercised by `local/deep-tests/tests/migrate_sqlite_roundtrip.rs` with no `prop_assume` limit (V1-03 fixed) |
| Migration mutation score | About 28.6% of viable measured mutants killed |
| Runtime mutation baseline | Incomplete |
| Overall line coverage | About 68% |
| Registry version | `0.4.0-beta.2` *(as assessed; `1.0.0-rc.1` published 2026-08-21 — see V1-05)* |
| RC tag | Local-only stale tag; no matching remote release tag |

### Stable-v1 exit target

- Production readiness is rescored at **at least 92/100**, matching the project's existing definition of done.
- All standard, MSRV, OS, database, and supported feature gates are green.
- No known migration correctness defect remains in supported v1 operations.
- No stable public API intentionally leaks memory per invocation.
- The resumable soak has been run for 15.56 cumulative hours with zero errors and plateaued memory; the remaining 48-hour target is waived.
- Critical-path mutation survivors are eliminated or individually justified.
- The actual RC artifact completes the documented feedback window with at least one external upgrade report.

---

## File map

| Path | Responsibility | Tasks |
|---|---|---|
| `crates/runtime/tests/soak_resumable.rs` | Explicit segmented soak and persisted cumulative state | V1-01, V1-02 |
| `local/run-soak-segment.ps1` | Safe explicit invocation of the ignored soak gate | V1-01, V1-02 |
| `crates/runtime/tests/query_manifest.rs` | Query-manifest test model across supported feature combinations | V1-01 |
| `.github/workflows/ci.yml` | Required default, native-driver, DB, OS, and MSRV gates | V1-01, V1-06 |
| `.github/workflows/release.yml` | RC verification and publish automation | V1-05 |
| `crates/migrate/src/plan.rs` | Multi-change ordering and SQLite rebuild planning | V1-03 |
| `local/deep-tests/tests/migrate_sqlite_roundtrip.rs` | Real SQLite migration round-trip properties | V1-03, V1-06 |
| `crates/runtime/src/executor.rs` | SQLx true-stream ownership and dispatch | V1-04 |
| `crates/runtime/src/tokio_postgres.rs` | `tokio-postgres` true-stream ownership | V1-04 |
| `crates/runtime/tests/streaming.rs` | Stream completion/cancellation/ownership tests | V1-04 |
| `xtask/src/main.rs` | Reproducible release and hardening invariants | V1-04, V1-05, V1-06 |
| `crates/runtime/src/rusqlite.rs` | Native SQLite pool state and execution | V1-07, V1-08 |
| `crates/runtime/src/pool.rs` | Cross-driver pool metrics facade | V1-07 |
| `crates/runtime/tests/pool_config.rs` | Pool-stat parity tests | V1-07 |
| `docs/SoakReport.md` | Final corrected soak evidence | V1-02 |
| `docs/MutationTesting.md` | Risk-based mutation results | V1-06 |
| `docs/TestingAnalysis.md` | Current coverage and verification results | V1-06 |
| `docs/BenchmarkResults.md` | Current-HEAD benchmark evidence and caveats | V1-08 |
| `docs/performance.md` | Native/default driver performance guidance | V1-08 |
| `docs/PublicApiReview.md` | Final RC API inventory | V1-09 |
| `docs/Stability.md` | RC policy and actual status | V1-05, V1-09 |
| `SECURITY.md`, `README.md`, `CHANGELOG.md`, `docs/*.md` | Consistent supported-version and release claims | V1-05, V1-10 |

---

# P0 — Required before publishing the RC

## V1-01 · Restore deterministic standard and native-feature gates — **completed 2026-08-21**

**Why:** The standard workspace suite and `cargo xtask harden` were failing, and the advertised native-rusqlite feature suite did not compile. All of these gates are now green.

**Status:** Complete at `dev-v0-2` HEAD. `cargo test --workspace`, `cargo clippy --workspace --all-targets`, `cargo test -p ruprizzle --features 'sqlite-rusqlite,ruprizzle-testkit/sqlite-rusqlite'`, and `cargo xtask harden` all exit zero.

**Files:**
- Modify: `crates/runtime/tests/soak_resumable.rs:390-410`
- Modify: `local/run-soak-segment.ps1:35-37`
- Modify: `crates/runtime/tests/query_manifest.rs:5-13`
- Verify: `.github/workflows/ci.yml:95-161`

**Interfaces:**
- Consumes: `RUPRIZZLE_SOAK_DB_PATH`, `RUPRIZZLE_TEST_RUSQLITE`, `FromRusqliteRow`, `FromOwnedRow`.
- Produces: a normal suite that never starts the 48-hour gate implicitly; an explicit ignored soak invocation; a feature-complete `Task: Model` test type.

- [ ] **Step 1: Preserve the two failing reproductions**

Run:

```powershell
cargo test --workspace
$env:RUPRIZZLE_TEST_RUSQLITE="1"
cargo test -p ruprizzle --features 'sqlite-rusqlite,ruprizzle-testkit/sqlite-rusqlite' --no-run
```

Expected before the fix:

- first command fails at `soak_rusqlite_resumable_48h` because `RUPRIZZLE_SOAK_DB_PATH` is absent;
- second command reports missing `FromOwnedRow` and `FromRusqliteRow` for `query_manifest::Task`.

- [ ] **Step 2: Make the 48-hour test explicit-only**

Change the test attributes to:

```rust
#[tokio::test]
#[ignore = "48-hour gate; run explicitly through local/run-soak-segment.ps1"]
async fn soak_rusqlite_resumable_48h() {
```

Do not silently return when the environment variable is absent. An explicit invocation with missing configuration must still fail loudly.

- [ ] **Step 3: Update the dedicated runner to execute the ignored test**

Use this command in `local/run-soak-segment.ps1`:

```powershell
cargo test -p ruprizzle --test soak_resumable --features "sqlite-rusqlite,ruprizzle-testkit/sqlite-rusqlite" --release -- --ignored --exact soak_rusqlite_resumable_48h --nocapture
```

- [ ] **Step 4: Give the query-manifest test model native SQLite row decoding**

Add the same feature-gated implementations used by neighboring runtime tests:

```rust
#[cfg(feature = "sqlite-rusqlite")]
impl ruprizzle::rusqlite::FromRusqliteRow for Task {
    fn from_rusqlite_row(
        row: &ruprizzle::rusqlite::RusqliteRow,
    ) -> Result<Self, ruprizzle::Error> {
        Ok(Self {
            id: ruprizzle::rusqlite::get::<i64>(row, 0)?,
            name: ruprizzle::rusqlite::get::<String>(row, 1)?,
        })
    }
}

#[cfg(feature = "sqlite-rusqlite")]
impl ruprizzle::rusqlite::FromOwnedRow for Task {
    fn from_owned_row(row: &ruprizzle::rusqlite::Row) -> Result<Self, ruprizzle::Error> {
        Ok(Self {
            id: row.get::<i64>(0)?,
            name: row.get::<String>(1)?,
        })
    }
}
```

If the combined native feature build also requires the existing `tokio_postgres_default_row!` pattern, add it under `postgres-tokio-postgres` as neighboring tests do.

- [ ] **Step 5: Verify every affected gate**

Run:

```powershell
cargo test --workspace
cargo clippy -p ruprizzle --features sqlite-rusqlite --all-targets -- -D warnings
cargo test -p ruprizzle --features 'sqlite-rusqlite,ruprizzle-testkit/sqlite-rusqlite'
cargo clippy -p ruprizzle --features postgres-tokio-postgres --all-targets -- -D warnings
cargo test -p ruprizzle --features postgres-tokio-postgres
cargo clippy -p ruprizzle --features 'sqlite-rusqlite,postgres-tokio-postgres' --all-targets -- -D warnings
cargo test -p ruprizzle --features 'sqlite-rusqlite,postgres-tokio-postgres'
```

Expected: all commands exit zero; the resumable soak is reported ignored in normal suites.

- [ ] **Step 6: Run the full hardening gate**

```powershell
cargo fmt --all --check
cargo xtask harden
```

Expected: both exit zero.

- [ ] **Step 7: Commit**

```bash
git add crates/runtime/tests/soak_resumable.rs local/run-soak-segment.ps1 crates/runtime/tests/query_manifest.rs
git commit -m "test: keep long soak explicit and restore native feature matrix"
```

**Acceptance:** Standard contributors do not need soak configuration; the dedicated runner still fails on bad configuration; all supported feature combinations compile and test.

---

## V1-02 · Record accepted segmented soak evidence and waive the 48-hour gate — **completed 2026-08-21**

**Why:** The resumable `rusqlite` soak accumulated **15.56 h (56,028.6 s) with 1,464,277,925 operations and 0 errors**. The maintainer accepted this as sufficient evidence and the remaining 48-hour target is **waived**. `docs/SoakReport.md` and the release plans record the waiver.

**Files:**
- Modify: `crates/runtime/tests/soak_resumable.rs`
- Modify: `local/run-soak-segment.ps1`
- Modify: `docs/SoakReport.md`

**Interfaces:**
- Consumes: persisted `State`, per-segment elapsed/ops/errors.
- Produces: `State::after_segment(...)` or an equivalent pure accumulator; fallible state I/O; restart evidence; final zero-error gate.

- [ ] **Step 1: Add failing accumulator tests**

Add pure tests demonstrating that a resumed segment preserves prior totals:

```rust
#[test]
fn resumed_segment_accumulates_totals() {
    let base = State {
        cumulative_elapsed: 60.0,
        total_ops: 2_380_049,
        total_errors: 0,
        completed: false,
    };
    let next = base.after_segment(30.0, 1_000, 0);
    assert_eq!(next.cumulative_elapsed, 90.0);
    assert_eq!(next.total_ops, 2_381_049);
    assert_eq!(next.total_errors, 0);
}

#[test]
fn resumed_segment_never_erases_errors() {
    let base = State {
        cumulative_elapsed: 60.0,
        total_ops: 100,
        total_errors: 2,
        completed: false,
    };
    let next = base.after_segment(30.0, 100, 0);
    assert_eq!(next.total_errors, 2);
    assert!(!next.completed);
}
```

Run:

```bash
cargo test -p ruprizzle --test soak_resumable resumed_segment
```

Expected before implementation: compile failure because `after_segment` does not exist.

- [ ] **Step 2: Add one cumulative-state function**

Its contract must be:

```rust
fn after_segment(&self, elapsed: f64, ops: u64, errors: u64) -> Self
```

It adds, never replaces, `cumulative_elapsed`, `total_ops`, and `total_errors`; `completed` is true only when elapsed reaches 172,800 seconds and total errors are zero.

- [ ] **Step 3: Use the same accumulator in health checkpoints and finalization**

Do not maintain separate arithmetic in the reporter and final block. Both must call the shared function from the segment's immutable base state.

- [ ] **Step 4: Propagate persistence errors**

Change `load_state`, `save_state`, and `init_state_table` to return `Result`. Remove `let _ =` around state-table DDL and state writes. A failed progress write must fail the segment instead of producing a false continuation point.

- [ ] **Step 5: Add a short two-segment restart test**

Use a temporary persistent SQLite file, run two short segments against it, reconnect between them, and assert:

- elapsed increases across the restart;
- operation totals increase rather than reset;
- a stored error can never be erased by a clean later segment;
- the second connection can read all saved state.

This is the automated proof for process/connection churn between segments.

- [ ] **Step 6: Verify a fresh short segment and a resume segment**

```powershell
$env:RUPRIZZLE_SOAK_DURATION_SECONDS="60"
.\local\run-soak-segment.ps1
$env:RUPRIZZLE_SOAK_DURATION_SECONDS="60"
.\local\run-soak-segment.ps1
```

Expected: the second completion line reports approximately 120 cumulative seconds and cumulative operations greater than the first run, with zero cumulative errors.

- [ ] **Step 7: Record the accepted evidence and stop the 48-hour gate**

Do not start any new long soak segments. Capture the final state:

```powershell
python local/soak-48h/status.py
```

Record the output (cumulative elapsed, total operations, total errors, `soak.err`
size, and final `soak_kv` row count) and append a "48-hour gate waived" section to
`docs/SoakReport.md`.

Acceptance evidence in `docs/SoakReport.md` must include:

- assessed commit and enabled features;
- segment count and duration per segment;
- cumulative elapsed, operations, and errors;
- first/final/peak RSS;
- pool saturation data after V1-07;
- confirmation that at least one process restart occurred;
- zero cumulative errors;
- the maintainer decision to waive the remaining 48-hour target.

- [ ] **Step 8: Commit the updated plan and report**

```bash
git add ProjectPlan/ProductionReadiness.md ProjectPlan/ProductionReadinessSolPlan.md ProjectPlan/v1/PathToStableV1.md ProjectPlan/v1/V1Blockers.md docs/SoakReport.md AGENTS.md README.md
git commit -m "docs: waive 48-hour W4-02 soak and record accepted evidence"
```

```bash
git add docs/SoakReport.md
git commit -m "docs: record accepted 15.56-hour / 1.46 B ops / 0 errors soak evidence"
```

**Acceptance:** The resumable soak evidence is recorded in `docs/SoakReport.md` and the 48-hour W4-02 gate is waived; the existing 15.56 h / 1.46 B operations / 0 errors result is accepted as sufficient.

---

## V1-03 · Fix SQLite multi-change migration planning — **completed 2026-08-21**

**Why:** The migration property suite explicitly skipped multi-change diffs because adding multiple required columns generated a rebuild that selected a column not yet present in the source table. The planner now builds the rebuild source from the table as it exists after previous changes, so multi-add (and multi-alter/rename) SQLite migrations are safe.

**Files:**
- Modify: `crates/migrate/src/plan.rs:113-145,315-359`
- Modify: `local/deep-tests/tests/migrate_sqlite_roundtrip.rs:164-198`
- Modify: `crates/migrate/tests/diff.rs` or create a focused planner regression in the existing migration test layout

**Interfaces:**
- Consumes: all `Change::AddColumn` items for a model.
- Produces: a model-scoped phased plan: add nullable physical columns → emit all backfill blocks → apply final constraints with at most one SQLite rebuild per model.

- [ ] **Step 1: Add a deterministic failing regression**

Build a previous SQLite schema with an existing populated table and a target schema that adds two required fields without defaults. Generate/apply the migration after replacing both generated backfill placeholders with concrete values. Assert that:

- planning references no source column before it exists;
- migration application succeeds;
- both columns are `NOT NULL` in the final introspected schema;
- existing rows contain the supplied backfill values;
- down planning remains explicit about destructive behavior.

Run:

```bash
cargo test -p ruprizzle-deep-tests --test migrate_sqlite_roundtrip multiple_required_columns
```

Expected before the fix: failure while copying a sibling field absent from the old table.

- [ ] **Step 2: Replace per-field interleaving with model-scoped phases on SQLite**

For each affected model:

1. add every new physical column in a form SQLite can accept on a populated table;
2. emit every editable backfill block;
3. rebuild the table once against the final target model so all final nullability, indexes, uniques, and foreign keys are restored together.

Postgres/MySQL behavior must remain unchanged unless the same regression demonstrates a defect there.

- [ ] **Step 3: Keep planner metadata intact**

The consolidated rebuild must preserve:

- destructive and non-transactional statement flags;
- editable backfill markers;
- index/unique/FK recreation;
- rename mappings;
- correct up/down direction.

- [ ] **Step 4: Remove the property-test exclusion**

Delete:

```rust
prop_assume!(
    changes.len() <= 1,
    "skipping {} simultaneous changes (planner limitation)",
    changes.len()
);
```

Increase cases only after the unrestricted property is deterministic on CI.

- [ ] **Step 5: Verify migration behavior**

```powershell
cargo test -p ruprizzle-migrate
cargo test -p ruprizzle-deep-tests --test migrate_sqlite_roundtrip
$env:RUPRIZZLE_REQUIRE_DB="1"
cargo test -p ruprizzle-integration-tests --test migrations
```

Expected: all pass, with no skipped multi-change property domain.

- [ ] **Step 6: Run targeted mutation testing before committing**

```powershell
$env:RUPRIZZLE_SOAK_DURATION_SECONDS="0"
cargo mutants -p ruprizzle-migrate --jobs 4 --minimum-test-timeout 5
```

No survivor may replace the new model-scoped sequencing with the prior per-field interleaving while tests remain green.

- [ ] **Step 7: Commit**

```bash
git add crates/migrate/src/plan.rs crates/migrate/tests local/deep-tests/tests/migrate_sqlite_roundtrip.rs
git commit -m "fix(migrate): plan SQLite multi-column changes per model"
```

**Acceptance:** Every generated v1 SQLite migration supports multiple simultaneous changes or emits a deliberate pre-application error; no known-supported plan fails midway because of planner ordering.

---

## V1-04 · Remove permanent allocations from unbuffered streaming — **completed 2026-08-21**

**Why:** The historical `Box::leak` path in `stream_unbuffered_raw` has already been removed. Current `dev-v0-2` source contains no `Box::leak` in `crates/runtime/src/executor.rs` or `crates/runtime/src/tokio_postgres.rs`; the SQLx and default paths fall back to buffered `stream_raw`, and the `tokio-postgres` path owns query state in the `unfold` closure.

**Files:**
- Modify: `crates/runtime/src/executor.rs:532-589`
- Modify: `crates/runtime/src/tokio_postgres.rs:210-260`
- Modify: `crates/runtime/tests/streaming.rs`
- Modify: `xtask/src/main.rs`
- Modify: `docs/KnownLimitations.md`, `docs/QueryGuide.md`

**Interfaces:**
- Consumes: owned `Cow<'static, str>` and `Vec<Value>`.
- Produces: a stream that owns query state for exactly the stream lifetime and drops it on completion/cancellation.

- [ ] **Step 1: Add lifecycle tests before changing ownership**

Cover all available streaming backends:

- complete a dynamic-SQL stream and drop it;
- drop after the first row;
- cancel while waiting for another row;
- repeat dynamic streams many times;
- verify the pool remains usable after each case.

Run:

```bash
cargo test -p ruprizzle --test streaming
```

- [ ] **Step 2: Add a hardening invariant against permanent query-state allocation**

Extend `xtask harden` to fail if production runtime streaming code contains `Box::leak` or an equivalent intentional permanent allocation. Test fixtures may be excluded; `crates/runtime/src/executor.rs` and `tokio_postgres.rs` may not.

- [ ] **Step 3: Implement an owned stream state machine without unsafe code**

The returned stream must own:

- SQL text;
- encoded bind arguments;
- backend query/cursor state;
- any checked-out connection/client required for cursor lifetime.

The implementation may use an existing dependency's owned stream primitives. It must not leak memory to satisfy a borrow and must not weaken `Send` or transaction constraints accidentally.

- [ ] **Step 4: Use a pre-RC removal gate if safe ownership is not achievable**

If a leak-free implementation requires unsafe self-references or an unreviewed public API break, remove `stream_unbuffered` from the stable RC surface or place it behind an explicitly unstable feature. A documented leak is not an acceptable stable-v1 compromise.

- [ ] **Step 5: Update documentation to match the chosen result**

For a fixed API, document cursor ownership, transaction restrictions, cancellation, and backend fallbacks. Remove the statement that SQL and binds are intentionally leaked.

For a deferred API, document that `stream()` remains buffered and true unbuffered streaming is not stable in v1.

- [ ] **Step 6: Verify all driver paths**

```powershell
cargo test -p ruprizzle --test streaming
cargo test -p ruprizzle --features sqlite-rusqlite --test streaming
cargo test -p ruprizzle --features postgres-tokio-postgres --test streaming
cargo xtask harden
```

Expected: tests pass and the hardening invariant finds no permanent stream allocations.

- [ ] **Step 7: Commit**

```bash
git add crates/runtime/src/executor.rs crates/runtime/src/tokio_postgres.rs crates/runtime/tests/streaming.rs xtask/src/main.rs docs/KnownLimitations.md docs/QueryGuide.md
git commit -m "fix(runtime): own unbuffered query state for the stream lifetime"
```

**Acceptance:** Memory used by a stream is reclaimable when that stream completes or is dropped; no supported path uses `Box::leak` for per-query data.

---

## V1-05 · Align RC version, tag convention, workflow, registry, and docs

> **Status (2026-08-21): COMPLETE.** All five steps are done and `1.0.0-rc.1` is
> published to crates.io for all ten publishable crates. What changed:
>
> - `release.yml` now triggers on both `v1.2.3*` and `1.2.3*` tag shapes, so a tag
>   cut without the `v` prefix can no longer be a silent no-op. `v`-prefixed
>   remains canonical.
> - `release.yml` gained a `workflow_dispatch` trigger with a `publish` input that
>   defaults to false. A manual run executes the whole gate plus
>   `cargo xtask release` (package-only) and cannot reach crates.io.
> - New `cargo xtask release-check --tag <name>` fails the job unless the tag
>   version, `workspace.package.version`, and the `## [<version>]` heading in
>   `CHANGELOG.md` all agree. It runs on every tag push before the gate.
> - `cargo xtask release` no longer omits `ruprizzle-check` and `ruprizzle-lsp`;
>   its package list now matches `release.yml` exactly (ten publishable crates;
>   `ruprizzle-testkit` is `publish = false`).
> - `SECURITY.md` supported-versions table rewritten to the real published line,
>   with the `RUSTSEC-2023-0071` exception stated explicitly.
> - `README.md`, `docs/README.md`, `docs/Stability.md`, and `docs/announcement.md`
>   no longer contradict each other about the release state; they now record that
>   `1.0.0-rc.1` is published (2026-08-21) from tag `v1.0.0-rc.1`.
>
> The stale local `1.0.0-rc.1` tag (40 commits behind HEAD, never pushed) was deleted
> with maintainer confirmation per Step 1; `v1.0.0-rc.1` is the release tag and is on
> `origin` at the release commit. The publish itself ran through
> `cargo xtask release --live --wait 60` and was verified independently with
> `cargo search` plus an out-of-tree consumer that resolves and compiles the whole
> graph from the registry.

**Why:** Workspace/docs claim `1.0.0-rc.1`, crates.io serves `0.4.0-beta.2`, the local tag is stale and unpushed, and `release.yml` only reacts to `v*` tags.

**Files:**
- Modify: `.github/workflows/release.yml`
- Modify: `xtask/src/main.rs`
- Modify: `README.md`, `CHANGELOG.md`, `SECURITY.md`
- Modify: `docs/README.md`, `docs/announcement.md`, `docs/faq.md`, `docs/Stability.md`, `docs/quickstart.md`, `docs/Examples.md`, `docs/Operations.md`
- Modify: active files under `ProjectPlan/v1/`

**Interfaces:**
- Consumes: workspace package version, Git tag, registry version, release-workflow mode.
- Produces: one release-state source of truth and a dry-run workflow that cannot publish accidentally.

- [x] **Step 1: Choose and enforce one tag format**

Use `v<workspace-version>` because the existing workflow listens for `v*`, for example `v1.0.0-rc.1`.

Add an `xtask` validation that compares the tag version, workspace version, and changelog heading before any publish command runs.

Do not move or delete the existing local tag during implementation. Present the stale tag and the intended replacement to the maintainer for explicit confirmation.

- [x] **Step 2: Add a non-publishing workflow exercise**

Add `workflow_dispatch` inputs that default to dry-run. The workflow must run the full gate and package checks without `cargo publish` unless an explicit publish input is true and the ref is a valid version tag.

- [x] **Step 3: Make the release gate cover the supported matrix**

At minimum, the release workflow must run:

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo clippy -p ruprizzle --features sqlite-rusqlite --all-targets -- -D warnings
cargo test -p ruprizzle --features sqlite-rusqlite
cargo clippy -p ruprizzle --features postgres-tokio-postgres --all-targets -- -D warnings
cargo test -p ruprizzle --features postgres-tokio-postgres
cargo xtask harden
```

Database-required CI must run separately with PostgreSQL and MySQL services and `RUPRIZZLE_REQUIRE_DB=1`. The normal/release suite's `--test soak` remains the short CI smoke. The ignored `--test soak_resumable` gate is intentionally run only through `local/run-soak-segment.ps1`; it must not turn a release workflow into a 48-hour job.

- [x] **Step 4: Correct public status before the RC exists**

Until crates.io confirms the RC, installation examples must resolve to the published beta or clearly state they require Git/path dependencies. Remove text that says the RC is already collecting feedback.

Update `SECURITY.md` so the supported versions table matches the actual published line and defines RC support.

- [x] **Step 5: Exercise dry-run automation**

Run the workflow through `workflow_dispatch` in dry-run mode and save the run URL/result in the release checklist. Local `cargo xtask release` must also exit zero without publishing.

- [ ] **Step 6: Publish only after V1-01 through V1-08 pass; the W4-02 48-hour soak is waived and the 15.56 h / 0-errors evidence is recorded**

The publish action requires explicit maintainer confirmation. After publication, verify every package with `cargo info`/`cargo search`, docs.rs builds, CLI installation, and the quickstart.

- [ ] **Step 7: Commit automation and pre-publish docs separately**

```bash
git add .github/workflows/release.yml xtask/src/main.rs
git commit -m "ci: make RC release validation explicit and dry-runnable"

git add README.md CHANGELOG.md SECURITY.md docs ProjectPlan/v1
git commit -m "docs: align RC status with registry and release gates"
```

**Acceptance:** There is one immutable RC commit, one matching remote `v*` tag, one registry version, one changelog entry, and no user-facing dependency snippet references an unavailable package.

---

# P1 — Required before stable GA

## V1-06 · Strengthen critical-path tests, mutation evidence, and coverage

**Why:** Overall line coverage is about 68%, migration mutation effectiveness is about 28.6%, and the runtime mutation baseline is incomplete. Raw coverage targets alone are insufficient; high-blast-radius behavior must be asserted.

**Files:**
- Modify: `crates/migrate/tests/*.rs`
- Modify: `crates/runtime/tests/*.rs`
- Modify: `crates/dialect/tests/*.rs`
- Modify: `.github/workflows/mutants.yml`, `.github/workflows/fuzz.yml`
- Modify: `docs/MutationTesting.md`, `docs/TestingAnalysis.md`

**Critical symbols:** `Schema`, `Column<M,T>`, `DbDialect`, `Compiler`, `SelectQuery`, `Executor`, `Tx`, `Pool`, `Change`, `Planner`, migration apply/split/checksum functions.

- [ ] **Step 1: Save complete mutation artifacts**

Ensure scheduled workflows upload caught, missed, timeout, and unviable lists for every shard even on success. Add a small merge/report step for runtime shards.

- [ ] **Step 2: Eliminate survivors in data-safety invariants**

Tests must fail if a mutant:

- makes a destructive change non-destructive;
- empties a non-empty diff;
- omits a planned migration statement;
- bypasses migration application/checksum/lock behavior;
- corrupts SQL statement splitting;
- shifts or drops a bind;
- prevents transaction rollback-on-drop;
- changes dialect placeholders/quoting/capability rejection.

- [ ] **Step 3: Complete the runtime mutation baseline**

```powershell
$env:RUST_BACKTRACE="0"
$env:RUPRIZZLE_SOAK_DURATION_SECONDS="0"
cargo mutants -p ruprizzle --jobs 4 --minimum-test-timeout 30 --output local/mutants-runtime
```

Record per-module results, not only one aggregate percentage.

- [ ] **Step 4: Rerun migration mutation tests after V1-03**

```powershell
$env:RUST_BACKTRACE="0"
$env:RUPRIZZLE_SOAK_DURATION_SECONDS="0"
cargo mutants -p ruprizzle-migrate --jobs 4 --minimum-test-timeout 5 --output local/mutants-migrate
```

No critical-path survivor may be waived without a written reason and a linked test limitation.

- [ ] **Step 5: Raise useful coverage**

Add tests first for current low-evidence risk areas:

- CLI command exit codes and filesystem effects;
- runtime rich-value encode/decode;
- native feature combinations;
- migration runner filesystem and failure paths;
- stream cancellation;
- soak state persistence.

Use 75% line coverage as a directional floor, but do not add assertions that merely execute lines without validating behavior.

- [ ] **Step 6: Record completed fuzz evidence**

For each parser/splitter target, record the exact commit, duration, executions, corpus changes, and crash result for at least four CPU-hours. Workflow configuration alone is not execution evidence.

- [ ] **Step 7: Verify and commit**

```powershell
cargo test --workspace
cargo llvm-cov --workspace
cargo xtask harden
```

```bash
git add crates .github/workflows/mutants.yml .github/workflows/fuzz.yml docs/MutationTesting.md docs/TestingAnalysis.md
git commit -m "test: harden ORM and migration critical paths"
```

**Acceptance:** Critical data/lifecycle behavior is mutation-sensitive, runtime baseline is complete, fuzz execution is evidenced, and coverage improvement comes from meaningful assertions.

---

## V1-07 · Provide native-driver pool telemetry parity

**Why:** Native `rusqlite` currently reports zero size/idle/waiter values, making saturation invisible in the exact backend targeted by the long soak.

**Files:**
- Modify: `crates/runtime/src/rusqlite.rs`
- Modify: `crates/runtime/src/pool.rs:65-105,534-580`
- Modify: `crates/runtime/tests/pool_config.rs`
- Modify: `docs/Operations.md`, `docs/SoakReport.md`

**Interfaces:**
- Produces: new `RusqlitePool::size()`, `num_idle()`, and `num_waiters()` methods plus accurate `PoolStats` dispatch.

- [ ] **Step 1: Add failing native pool-stat tests**

Assert configured size, idle count before checkout, in-use count during a transaction, waiter count during saturation, and recovery after rollback/drop.

- [ ] **Step 2: Expose synchronized counts from the native pool**

Read counts from the existing connection collection/checkout synchronization. Do not add a second unsynchronized accounting source.

- [ ] **Step 3: Dispatch native metrics through `Pool`**

Replace the hard-coded zeros for `Pool::SqliteNative` with the native methods. Document which SQLx backends cannot expose waiter counts instead of implying all zeros are real measurements.

- [ ] **Step 4: Verify metric feature behavior**

```powershell
cargo test -p ruprizzle --features 'sqlite-rusqlite,metrics' --test pool_config
$env:RUPRIZZLE_TEST_RUSQLITE="1"
cargo test -p ruprizzle --features 'sqlite-rusqlite,ruprizzle-testkit/sqlite-rusqlite' --test soak -- sqlite
```

Expected: the short `soak.rs` CI smoke reports nonzero native size/in-use values under load. The `soak_resumable.rs` 48-hour gate remains explicit and is now waived; the 15.56 h / 0-errors evidence is recorded in `docs/SoakReport.md`.

- [ ] **Step 5: Commit**

```bash
git add crates/runtime/src/rusqlite.rs crates/runtime/src/pool.rs crates/runtime/tests/pool_config.rs docs/Operations.md docs/SoakReport.md
git commit -m "feat(runtime): report native SQLite pool saturation"
```

**Acceptance:** `PoolStats` never reports synthetic zeros for observable native state; remaining backend limitations are explicit.

---

## V1-08 · Refresh performance evidence after correctness changes

**Why:** Current benchmark prose says native `rusqlite` avoids `spawn_blocking`, while current HEAD uses it. Performance claims must describe the released implementation.

**Files:**
- Modify: `local/cross-orm-bench/*` only if harness correctness requires it
- Modify: `docs/BenchmarkResults.md`, `docs/performance.md`, `README.md`

- [ ] **Step 1: Freeze the benchmark commit after P0 fixes**

Record commit, Rust version, dependency lockfile, CPU/OS, database versions, features, warmups, measured trials, and raw result paths.

- [ ] **Step 2: Run the complete cross-ORM SQLite suite**

```powershell
python local/cross-orm-bench/run_bench.py
```

Do not keep a favorable historical headline if the current implementation produces different results.

- [ ] **Step 3: Run the native Postgres-vs-raw baseline**

Use the repository's end-to-end benchmark with a live PostgreSQL database. Compare ruprizzle and handwritten SQL through matching native pools, schemas, queries, and connection settings.

- [ ] **Step 4: Profile before optimizing**

Prioritize only measured hotspots. Candidate areas are:

- native SQLite `spawn_blocking` dispatch cost;
- multi-row intermediate row decoding;
- nested include grouping;
- repeated SQLite table rebuilds, already reduced by V1-03.

Do not move synchronous SQLite work back onto Tokio worker threads solely to recover a microbenchmark number; event-loop safety is the stronger production invariant.

- [ ] **Step 5: Update claims and caveats**

Separate:

- query-construction cost;
- local SQLite driver cost;
- networked database overhead;
- automatic relation loading versus manual joins.

- [ ] **Step 6: Commit**

```bash
git add local/cross-orm-bench docs/BenchmarkResults.md docs/performance.md README.md
git commit -m "docs(perf): refresh benchmarks for the v1 runtime paths"
```

**Acceptance:** Every headline performance number is tied to current code/raw data and does not generalize beyond the measured driver/workload.

---

## V1-09 · Freeze and verify the actual RC compatibility surface

**Why:** The public surface is wide: runtime, migrations, lower-level crates, generated clients, CLI scripts, error kinds, and MSRV. The final review must target the artifact users install.

**Files:**
- Modify: `docs/PublicApiReview.md`, `docs/Stability.md`, `docs/MigrationGuideToV1.md`
- Modify tests/snapshots only when they expose an accidental break

- [ ] **Step 1: Rerun public API inventory for every published library crate**

```bash
cargo +nightly public-api -p <crate> --simplified
```

Diff against the latest published beta and the intended RC baseline.

- [ ] **Step 2: Add generated-client compatibility fixtures**

Keep representative beta schemas and consumer code that references entity fields, columns, relations, CRUD builders, errors, and transactions. Generate with the RC and compile on MSRV.

- [ ] **Step 3: Snapshot CLI compatibility**

Test command names, flags, help text where stable, exit-code classes, and noninteractive migration-deploy behavior.

- [ ] **Step 4: Verify MSRV and semver checks**

```bash
cargo +1.85 test --workspace
cargo semver-checks check-release
```

Use the project's exact per-package semver invocation if the workspace command is unsupported.

- [ ] **Step 5: Publish the RC only after the previous gates pass**

Publication is a maintainer-approved side effect under V1-05.

- [ ] **Step 6: Run the documented feedback window**

Collect at least one external upgrade report covering:

- installation from crates.io;
- schema generation;
- migration from the beta;
- application compilation;
- one real database workflow;
- feedback on the frozen API.

Any API correction produces a new RC and a focused renewed window.

- [ ] **Step 7: Commit review evidence**

```bash
git add docs/PublicApiReview.md docs/Stability.md docs/MigrationGuideToV1.md tests examples
git commit -m "docs: record final RC compatibility review"
```

**Acceptance:** The public/CLI/generated surface in the report is exactly the surface in the published RC.

---

## V1-10 · Resolve security and release-document drift

**Why:** Security support still names `0.1.x`, status pages disagree about whether the RC exists, and install snippets reference an unavailable RC.

**Files:**
- Modify: `SECURITY.md`, `deny.toml`, `README.md`, `CHANGELOG.md`
- Modify: current user docs under `docs/`
- Add a release-state consistency check to `xtask` if V1-05 does not already cover it

- [ ] **Step 1: Recheck `RUSTSEC-2023-0071` upstream state**

If a compatible patched dependency exists, update through Cargo and remove the exception. If none exists, retain the narrow exception, document the affected MySQL authentication path and mitigations, and do not describe that path as unqualified production-safe.

- [ ] **Step 2: Update the supported-version policy**

List the actual maintained beta/RC lines and when support transfers from one to the next.

- [ ] **Step 3: Make release facts centrally checkable**

The consistency check should fail when:

- workspace version and changelog release differ;
- stable docs claim an unavailable crates.io version;
- tag and workspace version differ in publish mode;
- an active status page says both “published” and “not published.”

Historical documents may retain old facts when clearly labeled historical.

- [ ] **Step 4: Verify documentation**

```powershell
mdbook build
$env:RUSTDOCFLAGS="-D warnings"
cargo doc --workspace --no-deps
cargo xtask harden
```

- [ ] **Step 5: Commit**

```bash
git add SECURITY.md deny.toml README.md CHANGELOG.md docs xtask/src/main.rs
git commit -m "docs(security): align support and RC status with released artifacts"
```

**Acceptance:** A new user can install every documented version, and the security/release status is consistent across active documents.

---

# v1 optimization and enhancement decisions

## Optimizations approved for v1

| Optimization | Decision | Trigger/evidence |
|---|---|---|
| Consolidate SQLite rebuilds per model | **Implement in V1-03** | Fixes a correctness defect and avoids repeated rebuild work |
| Remove permanent stream allocations | **Implement or remove API in V1-04** | Unbounded memory growth is not acceptable |
| Native pool telemetry | **Implement in V1-07** | Needed to interpret the soak and production saturation |
| Native SQLite dispatch/decoding optimization | **Profile in V1-08** | Current benchmark narrative is stale; event-loop safety wins over microbenchmarks |
| Relation include grouping optimization | **Profile only** | It is a measured hotspot, but current behavior is correct |
| `sqlx::Any` rich-type micro-allocations | **Defer unless profiling shows default-path impact** | Native Postgres is already selected for normal Postgres URLs |

## Enhancements approved for v1

- Explicit-only long-running gates with durable results.
- Risk-based mutation reporting for high-centrality code.
- Accurate native-driver pool stats.
- Dry-runnable release automation and version/tag validation.
- Generated-client and CLI compatibility fixtures.
- Current-commit benchmark provenance.

## Feature policy for v1

No broad new ORM feature is justified before GA. The only “feature” work allowed is completion of already-promised behavior:

- leak-free true streaming or its removal from the stable surface;
- correct multi-change migrations;
- native telemetry parity;
- reliable offline/query-manifest behavior across supported features.

This is deliberate. Adding another query operator or database backend cannot compensate for red release gates.

---

# Explicit post-v1 deferrals

| Deferred capability | Earliest reconsideration | Reason |
|---|---|---|
| Full-text search | 1.1/1.2 | Additive and dialect-specific; no stable-v1 blocker |
| PostGIS/geospatial types | 1.2+ | Large type/query/migration surface |
| Soft-delete conventions | 1.2+ | Policy-heavy and can be built in applications today |
| Polymorphic relations | 1.2+ | Complicates generated types, constraints, and migration semantics |
| Implicit many-to-many tables | 1.2+ | Explicit join models work and are documented in ADR-006 |
| Recursive ancestor/descendant helpers | 1.2+ | Depth-limited includes and recursive CTEs already provide escape hatches |
| MSSQL/additional databases | 2.0 or evidence-led minor | Multiplies dialect and CI matrix |
| Vector/pgvector search | 1.1+ | Additive extension, not core ORM correctness |
| Multi-tenancy/RLS abstraction | Post-v1 | Application/database policy; premature stable abstraction |
| Hosted Studio/GUI | Separate product | Outside the crate's runtime correctness mission |
| Framework-specific integrations | Post-v1 examples/crates | Useful adoption work but not a core release gate |

---

# Execution order and score impact

| Order | Task | Priority | Blocks | Expected score effect |
|---:|---|---|---|---:|
| 1 | V1-01 deterministic gates | P0 | Every later release claim | +4 to correctness/release confidence |
| 2 | V1-02 soak evidence accepted | P0 | — | gate waived; evidence recorded in `docs/SoakReport.md` |
| 3 | V1-03 SQLite multi-change planning | P0 | Broad migration endorsement | +4 to data safety |
| 4 | V1-04 leak-free streaming | P0 | Stable API freeze | +3 to correctness/operability |
| 5 | V1-06 critical-path test evidence | P1, start early | Final assurance score | +3 to assurance |
| 6 | V1-07 native metrics parity | P1 | Interpretable soak evidence | +2 to operability |
| 7 | V1-08 current performance evidence | P1 | Honest performance claims | +1 to performance/docs |
| 8 | V1-05 RC automation/status | P0 release step | RC publish | +3 to release/docs |
| 9 | V1-09 compatibility and feedback | P1 calendar gate | Stable GA | +4 to API/ecosystem |
| 10 | V1-10 security/docs drift | P1 | Stable GA messaging | +1 to security/docs |

Score effects are directional, not guaranteed points. V1 must be rescored from fresh evidence; work is not complete merely because a planned item was implemented.

---

# Final verification gate

Run the complete gate against the exact intended release commit:

```powershell
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo clippy -p ruprizzle --features sqlite-rusqlite --all-targets -- -D warnings
cargo test -p ruprizzle --features 'sqlite-rusqlite,ruprizzle-testkit/sqlite-rusqlite'
cargo clippy -p ruprizzle --features postgres-tokio-postgres --all-targets -- -D warnings
cargo test -p ruprizzle --features postgres-tokio-postgres
cargo clippy -p ruprizzle --features 'sqlite-rusqlite,postgres-tokio-postgres' --all-targets -- -D warnings
cargo test -p ruprizzle --features 'sqlite-rusqlite,postgres-tokio-postgres'
$env:RUSTDOCFLAGS="-D warnings"
cargo doc --workspace --no-deps
cargo deny check
cargo xtask examples
cargo xtask harden
mdbook build
```

Required external/long-running evidence:

- Rust 1.85 workspace test pass;
- Windows, Linux, and macOS CI pass;
- PostgreSQL and MySQL integration pass with `RUPRIZZLE_REQUIRE_DB=1`;
- parser and migration splitter fuzzed for at least four CPU-hours each with no crashes;
- resumable native SQLite soak run for 15.56 cumulative hours with zero errors and plateaued memory; the remaining 48-hour target is waived;
- mutation report with no unjustified critical-path survivors;
- RC packages install from crates.io and docs.rs builds;
- at least one external beta-to-RC upgrade report;
- two-week minimum RC feedback window complete;
- final production-readiness rescore at or above the project's 92/100 target.

Only then cut stable `1.0.0`.
