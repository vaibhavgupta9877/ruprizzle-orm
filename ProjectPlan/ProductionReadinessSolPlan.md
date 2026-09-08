# Production Readiness Remediation Plan

**Baseline:** `main` at `0584934`, assessed 2026-09-08

**Current readiness:** **62 / 100 — release blocked**

**Target:** **at least 90 / 100**, with every release-critical gate green on the exact candidate commit

**Companion assessment:** `ProjectPlan/ProductionReadinessSol.md`

## 1. Goal and operating rules

Move the current `1.5.0` tree from a strong beta with red release controls to a traceable stable release without expanding product scope.

Rules:

- Develop from `dev-main` and merge to `main` only for release, following repository policy.
- Do not move, delete, or recreate `v1.5.0`; it is evidence of the failed publish attempt.
- Do not publish, push, change repository secrets, or alter branch protection without explicit maintainer approval.
- Do not suppress tests, semver findings, advisories, or hardening checks merely to obtain green status.
- Preserve `#![forbid(unsafe_code)]`, parameter binding, identifier quoting, and current hardening budgets.
- Use Rust 1.85 for MSRV validation and pinned Rust 1.95 for repository formatting/primary checks.
- Database evidence counts only when missing services are failures (`RUPRIZZLE_REQUIRE_DB=1`).
- Hosted Turso/D1 evidence must use disposable test databases and non-production credentials.
- No new feature work enters this plan.

## 2. Issue register

| ID | Priority | Finding | Release effect | Primary area |
|---|---|---|---|---|
| PR-01 | P0 | Format, trybuild, generated snapshots, all-feature tests, and hardening are red | Blocks release | tests/toolchain |
| PR-02 | P0 | Semver CI reports major-version-level public API breaks | Blocks stable API claim | core/dialect/check |
| PR-03 | P0 | `stream_unbuffered` buffers on most backends despite its contract | Correctness/operability blocker | runtime/adapters |
| PR-04 | P0 | Registry, changelog, README, security policy, and limitations contradict each other | Product-integrity blocker | docs/release state |
| PR-05 | P0 | Failed tag, no stable registry artifact, and apparently empty registry token | Publication blocker | release automation |
| PR-06 | P1 | Fuzz workflow selects stable and performs zero fuzzing | Assurance gap | CI/fuzz |
| PR-07 | P1 | Mutation workflow is red/noisy; runtime measures nothing; migration score is weak | Assurance gap | CI/tests |
| PR-08 | P1 | Coverage baseline predates v1.1–v1.5 and has no CI trend | Assurance gap | tests/CI |
| PR-09 | P1 | Turso and D1 lack real hosted-service qualification | Backend confidence gap | adapters |
| PR-10 | P1 | Studio has no authentication | Deployment limitation | Studio/security |
| PR-11 | P1 | MySQL retains `RUSTSEC-2023-0071` in an authentication path | Accepted security risk | dependencies/docs |
| PR-12 | P1 | Performance evidence does not cover the current feature/backend surface | Adoption evidence gap | benchmarks/docs |
| PR-13 | P2 | GitHub Actions dependencies use mutable version tags | Supply-chain hardening | workflows |
| PR-14 | P2 | Repeated hand-written release facts drift across documents | Recurrence risk | docs/xtask |

## 3. Exit gates

A release candidate is acceptable only when:

- [ ] `cargo fmt --all --check` passes.
- [ ] `cargo clippy --workspace --all-features --all-targets -- -D warnings` passes.
- [ ] `cargo test --workspace --all-features` passes from a clean checkout.
- [ ] `RUSTDOCFLAGS='-D warnings' cargo doc --workspace --no-deps --all-features` passes.
- [ ] `cargo xtask harden` passes.
- [ ] `cargo xtask examples` passes.
- [ ] MSRV tests pass on Rust 1.85.
- [ ] PostgreSQL/MySQL/SQLite integration and supported native-feature matrices pass with required services.
- [ ] The semver report has no unreviewed failure.
- [ ] Both fuzz targets start under nightly and complete the configured bounded smoke run; the scheduled long run is green.
- [ ] Mutation jobs produce complete machine-readable reports and apply an explicit threshold/allowlist policy.
- [ ] `stream_unbuffered` behavior and documentation agree for every executor.
- [ ] Release/version/support claims agree with crates.io reality.
- [ ] A non-publishing release rehearsal succeeds.
- [ ] The exact candidate commit receives a green public CI run before publication.
- [ ] After publication, all publishable crates and an out-of-tree consumer are verified.

---

# P0 — release blockers

## PR-01 · Restore deterministic mechanical and test gates

**Evidence**

- `cargo fmt --all --check` fails in `crates/runtime/tests/insert_validation.rs` and `crates/testkit/src/lib.rs`.
- Public CI's Rust 1.95 trybuild output omits a standard-library context block still present in `crates/runtime/tests/trybuild/col_gt_on_string.stderr`.
- `cargo test --workspace --all-features` generates `snapshots__blog__postgres___generated.snap.new` because the committed schema hash is stale.
- `cargo xtask harden` inherits the workspace test failure.

**Files**

- `crates/runtime/tests/insert_validation.rs`
- `crates/testkit/src/lib.rs`
- `crates/runtime/tests/trybuild/col_gt_on_string.stderr`
- `crates/codegen/tests/snapshots/snapshots__blog__postgres___generated.snap`
- related schema fixtures that produce the hash
- `.github/workflows/ci.yml`
- `rust-toolchain.toml`

**Implementation**

- [ ] Run `cargo fmt --all`; inspect the diff and keep only rustfmt output.
- [ ] Reproduce the trybuild mismatch in a clean Linux environment using the exact pinned toolchain.
- [ ] Confirm the diagnostic still proves `String` does not satisfy `Ordered`; update only compiler-rendering text, not the semantic expectation.
- [ ] Run the complete trybuild test on pinned 1.95 and MSRV 1.85. If one fixture cannot be stable across both compilers, split snapshots by toolchain or replace brittle full-stderr matching with a focused compile-fail assertion that checks the required diagnostic content.
- [ ] Regenerate all codegen snapshots, not only the first failure, and review every generated API/SQL change before accepting.
- [ ] Add a clean-checkout test that fails when generated snapshots or schema hashes drift.
- [ ] Run the complete gate list below; do not stop after the first green package.

**Verification**

```bash
cargo fmt --all --check
cargo test -p ruprizzle --test trybuild
cargo test -p ruprizzle-codegen --test snapshots
cargo test --workspace
cargo test --workspace --all-features
cargo clippy --workspace --all-features --all-targets -- -D warnings
cargo xtask examples
cargo xtask harden
```

Run the workspace tests once with Rust 1.85 as well.

**Acceptance:** no `.snap.new`/`.stderr.new` files, deterministic results from a clean checkout, and all listed commands exit zero.

---

## PR-02 · Resolve the semver break set and future-proof public evolution

**Evidence**

Public CI reports:

- fields added to public constructible structs such as `IndexDef` and `FieldAttrs`;
- variants added to exhaustive `SchemaError`, `ScalarType`, `RustType`, and `QueryCheckError` enums;
- implicit discriminant changes on existing `SchemaError` variants;
- fields added to existing public error variants.

**Files**

- `crates/core/src/ir.rs`
- `crates/core/src/diagnostic.rs`
- `crates/dialect/src/lib.rs`
- `crates/check/src/validate.rs`
- `docs/Stability.md`
- `CHANGELOG.md`
- `.github/workflows/ci.yml`

**Decision required**

The maintainer must choose one explicit policy after reviewing downstream impact:

1. **Compatibility repair:** preserve the prerelease API by avoiding new required struct fields, retaining discriminants, and redesigning additions behind constructors/non-exhaustive extension points; or
2. **One-time prerelease transition waiver:** document that the only registry baseline is `1.0.0-rc.1`, enumerate every intentional break, harden the final stable API, and encode a narrowly scoped one-release exception without disabling future semver checks; or
3. **Major release:** publish the breaking surface as `2.0.0`.

Do not label the failures false positives: each reported pattern can break valid downstream Rust source.

**Implementation**

- [ ] Export the full semver JSON/report for every publishable crate and group findings by API type.
- [ ] Search repository and known downstream usage for exhaustive matches, struct literals, and numeric casts.
- [ ] Assign each finding: repair, intentional prerelease break, or unintended break.
- [ ] Add constructors/accessors where public struct literals should no longer be the extension contract.
- [ ] Add `#[non_exhaustive]` to extensible enums/structs where appropriate before the first stable artifact, accounting for the source break this itself creates for prerelease users.
- [ ] Give externally meaningful discriminants explicit values, or document that numeric casting is unsupported and choose a compatible representation.
- [ ] Record approved prerelease breaks in `CHANGELOG.md` and `docs/Stability.md`.
- [ ] Keep semver CI mandatory after the transition release; any waiver must be exact-version and self-removing.

**Verification**

```bash
cargo semver-checks --workspace
cargo test --workspace --all-features
cargo doc --workspace --no-deps --all-features
```

Also compile at least one fixture consumer that uses the previous public shapes.

**Acceptance:** semver CI is green or the exact prerelease transition has a reviewed, version-scoped, documented exception that cannot hide later regressions.

---

## PR-03 · Make unbuffered streaming behavior truthful

**Root cause**

`Executor::stream_unbuffered_raw` defaults to `stream_raw`, and `Pool` delegates only native `tokio-postgres`; every other path buffers the full `RowBatch`. Public query docs still promise incremental SQLx rows and bounded peak memory. Historical limitations prose separately claims leaked allocations that no longer exist.

**Files**

- `crates/runtime/src/executor.rs`
- `crates/runtime/src/query.rs`
- `crates/runtime/src/tokio_postgres.rs`
- `crates/runtime/tests/streaming.rs`
- `crates/turso/src/lib.rs`
- `crates/d1/src/lib.rs`
- `docs/KnownLimitations.md`
- `docs/QueryGuide.md`
- `README.md`

**Preferred implementation**

- [ ] Define an explicit executor capability for true cursor/incremental streaming rather than silently falling back.
- [ ] For SQLx PostgreSQL/MySQL/SQLite, implement ownership-safe streaming without `Box::leak`; query state must be released on completion, error, and cancellation.
- [ ] For native `rusqlite` and buffered HTTP APIs, return a clear unsupported/capability error from the true-stream API unless a genuinely incremental implementation exists.
- [ ] Keep Turso/D1's buffered response behavior explicit; do not call it memory-bounded streaming.
- [ ] If an ownership-safe cross-backend API cannot be delivered compatibly, deprecate the misleading method and add a new capability-checked API rather than preserving false semantics.

**Tests**

- [ ] Add an executor spy that proves the first row can be yielded before the complete source is available.
- [ ] Add cancellation/drop tests proving query state is released.
- [ ] Add a large-result memory/behavior test for each claimed true-stream backend.
- [ ] Add negative tests for unsupported executors.
- [ ] Ensure prepared and projected streaming paths obey the same contract.

**Acceptance:** every backend either yields incrementally with bounded ownership or returns a documented unsupported error; no backend silently buffers behind a method named/documented as unbuffered.

---

## PR-04 · Establish one truthful release and support state

**Contradictions to remove**

- `CHANGELOG.md`: says `1.5.0` is released/latest.
- `README.md` and `docs/README.md`: say `1.0.1` is latest and `1.5.0` is unpublished.
- `SECURITY.md`: says `1.0.0` is current.
- `docs/KnownLimitations.md`: says "Current beta," defers already shipped features, and describes the removed streaming leak.
- crates.io search: exposes `1.0.0-rc.1` as the latest observed package.

**Files**

- `CHANGELOG.md`
- `README.md`
- `docs/README.md`
- `SECURITY.md`
- `RELEASES.md`
- `docs/Stability.md`
- `docs/KnownLimitations.md`
- `docs/WhatsNewV1_1ToV1_5.md`
- crate READMEs

**Implementation**

- [ ] Treat crates.io/API output as the authority for "published" status.
- [ ] Correct all documents to say that the observed registry release is `1.0.0-rc.1` and the `v1.5.0` publish attempt failed, until publication actually succeeds.
- [ ] Separate "workspace version," "tagged candidate," and "published version" so one cannot imply another.
- [ ] Update `SECURITY.md` to identify what is actually downloadable and what source lines receive security fixes.
- [ ] Rewrite `KnownLimitations.md` for the current implementation: remove completed deferrals, remove the historical leak statement, and document actual per-backend streaming behavior.
- [ ] Add an `xtask` release-state audit that rejects a changelog/README claim of publication unless a supplied registry check confirms it. Keep network checks out of ordinary offline builds; run them in release/Docs CI.
- [ ] Search all tracked Markdown and crate metadata for stale version/status phrases before release.

**Acceptance:** a reader receives the same version, support, adapter, and limitation state from the registry, root README, docs README, changelog, security policy, stability policy, release notes, and crate READMEs.

---

## PR-05 · Rebuild the release path without rewriting history

**Evidence**

- `v1.5.0` points to `0e4d96b`.
- release run 33633504583 passed `release-check` but failed formatting before publication.
- every publish step was skipped.
- the run rendered `CARGO_REGISTRY_TOKEN` as empty.
- local `main` has moved beyond the tag.

**Implementation**

- [ ] Leave `v1.5.0` untouched.
- [ ] Close PR-01 through PR-04 on `dev-main` and merge through the documented release process.
- [ ] Ask the maintainer to verify `CARGO_REGISTRY_TOKEN` in GitHub repository/environment secrets. Never print or copy the token.
- [ ] Run `workflow_dispatch` with publishing disabled and require the complete release job to pass, including package creation for every crate.
- [ ] Inspect `cargo package --list` and unpacked manifests for all publishable crates.
- [ ] Choose a new traceable version/tag (normally `1.5.1`, since `v1.5.0` already identifies a failed source tree) and update workspace pins, extension version, changelog, and release notes together.
- [ ] Require a green ordinary CI run on that exact commit before creating the new tag.
- [ ] Obtain explicit maintainer approval immediately before tag push/publication.
- [ ] After publishing, verify all twelve crates through registry metadata and compile an out-of-tree consumer using only registry dependencies.
- [ ] Announce publication only after verification.

**Acceptance:** immutable tag-to-source provenance, successful release workflow, all intended packages present at matching versions, docs.rs builds queued/complete, and a registry-only consumer compiles and runs a SQLite smoke test.

---

# P1 — assurance and bounded production use

## PR-06 · Repair fuzz execution

**Root cause:** `dtolnay/rust-toolchain@nightly` installs nightly, but `rust-toolchain.toml` selects 1.95 for the unqualified `cargo fuzz` command.

**Files:** `.github/workflows/fuzz.yml`, optional local fuzz runbook.

- [ ] Invoke `cargo +nightly fuzz run ...` explicitly, or set `RUSTUP_TOOLCHAIN=nightly` for the fuzz step.
- [ ] Add a short pull-request/manual smoke mode (for example 60 seconds per target) before relying on four-hour schedules.
- [ ] Preserve artifacts and seed corpora even when no crash occurs so corpus growth is observable.
- [ ] Confirm both `parser` and `migrate_splitter` execute inputs under sanitizer instrumentation.
- [ ] Run the scheduled four-hour jobs and record executions, corpus growth, and crashes.

**Acceptance:** logs show actual executions and elapsed fuzz time, not toolchain setup failure; both short and scheduled jobs are green in the no-crash case.

---

## PR-07 · Make mutation testing measurable and improve critical survivors

**Files:** `.github/workflows/mutants.yml`, `docs/MutationTesting.md`, runtime/migration tests, optional `xtask` report checker.

- [ ] Make a baseline test pass mandatory before mutation starts.
- [ ] Capture `mutants.json` from every shard and merge runtime shard results.
- [ ] Distinguish "completed with survivors" from infrastructure/test-baseline failure.
- [ ] Define a checked policy: minimum score plus zero unjustified survivors in migration application, destructive classification, SQL splitting, transaction boundaries, bind handling, and stream dispatch.
- [ ] Add focused tests for the 391 current migration survivors, prioritizing `split_statements`, `dollar_tag_len`, `matches_at`, diff omission, and `Migrator::apply_all` replacement.
- [ ] Classify equivalent mutants in a reviewed allowlist with reasons; never blanket-ignore files.
- [ ] Re-run all runtime shards after PR-01 and publish a single baseline summary.
- [ ] Update `docs/MutationTesting.md`; remove the 2026-08-17 "run in progress" state.

**Acceptance:** complete reports, no infrastructure-red shards, explicit policy enforcement, and no unreviewed survivor in release-critical paths.

---

## PR-08 · Refresh and continuously track coverage

**Files:** `.github/workflows/ci.yml` or a dedicated coverage workflow, `docs/TestingAnalysis.md`.

- [ ] Collect `cargo llvm-cov --workspace --all-features --lcov` from the current tree.
- [ ] Include Studio, adapters, generated-code checks, and supported feature paths where technically possible.
- [ ] Publish the report as an artifact and summarize line/function/region deltas.
- [ ] Set a no-regression floor based on the new honest baseline; do not reuse the historical 68.08% value.
- [ ] Add focused tests for low-coverage release-critical modules before raising a global vanity threshold.

**Acceptance:** current reproducible baseline, CI artifact, no-regression enforcement, and risk-based coverage for migrations, decoding/binding, routing/cache invalidation, Studio writes, and adapter protocol errors.

---

## PR-09 · Qualify Turso and D1 against hosted services

**Files:** adapter integration tests, secret-backed scheduled/manual workflow, adapter READMEs, `docs/KnownLimitations.md`.

- [ ] Create opt-in live tests that create a table, bind representative values, insert, read, update/delete where supported, and surface statement/transport errors.
- [ ] Use disposable provider resources and least-privilege credentials supplied through GitHub secrets.
- [ ] Validate actual endpoint/path/auth behavior, row/affected-count decoding, null/numeric/text values, and provider error envelopes.
- [ ] Keep local fake-server tests for deterministic protocol coverage.
- [ ] Record date, adapter version, API/protocol version, and results.
- [ ] Do not claim transactions, true streaming, embedded replicas, Worker binding, or WASM support.

**Acceptance:** one successful live qualification per provider plus a repeatable opt-in workflow; limitations remain explicit.

---

## PR-10 · Bound Studio's security posture

**Near-term release scope**

- [ ] Keep loopback binding as the default.
- [ ] Keep non-loopback writes refused without explicit acknowledgement.
- [ ] Add an unmissable startup/UI banner when bound non-loopback or writes are enabled.
- [ ] Document deployment only behind an authenticated reverse proxy/tunnel; do not imply the name heuristic protects production.
- [ ] Add CSRF/origin review for mutation routes even on loopback.
- [ ] Ensure logs and error pages never expose database credentials.

**Post-release option:** implement first-party authentication and CSRF protection before describing Studio as remotely deployable.

**Acceptance:** local-development positioning is consistent and tests pin every exposure guard.

---

## PR-11 · Revalidate the MySQL advisory exception

- [ ] Run current `cargo deny check advisories` and identify the exact dependency chain/version.
- [ ] Check for a patched `sqlx`/`rsa` path compatible with MSRV and public API constraints.
- [ ] If unresolved, retain the narrow exception, verify TLS/Unix-socket mitigation guidance, and keep MySQL out of unqualified production claims.
- [ ] Add a scheduled dependency/advisory check so the exception is removed promptly when a patch exists.

**Acceptance:** patched dependency or current, bounded, consistently documented exception with mitigation.

---

## PR-12 · Refresh performance evidence for the actual release surface

- [ ] Re-run the existing cross-ORM SQLite benchmark on the candidate commit and record environment/commit.
- [ ] Add representative networked Postgres/MySQL latency/throughput tests without claiming universal ORM superiority.
- [ ] Measure buffered versus true-stream behavior and peak memory after PR-03.
- [ ] Add focused cache hit/miss/invalidation and replica-routing overhead/failover tests.
- [ ] Add Turso/D1 request-per-statement latency only after PR-09; clearly separate local fake and hosted measurements.
- [ ] Update `docs/BenchmarkResults.md` and `docs/performance.md`, preserving historical results as historical.

**Acceptance:** every headline claim names the commit, environment, driver, dataset, and limitation; no result describes a superseded execution path.

---

# P2 — hardening and recurrence prevention

## PR-13 · Pin GitHub Actions immutably

- [ ] Resolve every third-party action tag to a reviewed full commit SHA.
- [ ] Keep the human-readable release tag in a comment.
- [ ] Configure dependency automation to propose action SHA updates.
- [ ] Review action permissions and set workflow/job `permissions` to the minimum required.

**Acceptance:** no executable third-party action is referenced only by a mutable tag.

---

## PR-14 · Generate/check repeated release facts

- [ ] Define one structured release-state source containing workspace version, published version, candidate tag, support line, and release status.
- [ ] Have `xtask` validate internal dependency pins, extension version, changelog heading, tag, README status, security support line, and release workflow order.
- [ ] Run offline structural checks on every PR and registry-backed truth checks only in release/docs jobs.
- [ ] Keep historical assessments clearly marked as snapshots; current documents should not retain superseded verdict prose as active guidance.

**Acceptance:** intentionally changing one version/status fact causes a deterministic check failure until all authoritative surfaces agree.

## 4. Execution order

1. **PR-01** — restore a trustworthy baseline.
2. **PR-02** — decide the stable API before more implementation changes.
3. **PR-03** — correct the public streaming contract.
4. **PR-04** — make present state truthful.
5. **PR-06 and PR-07** — restore assurance jobs once baseline tests are green.
6. **PR-08, PR-09, PR-10, PR-11, PR-12** — produce bounded production evidence.
7. **PR-13 and PR-14** — prevent recurrence.
8. **PR-05 last** — rehearse and execute release only after every prerequisite passes.

## 5. Rerating criteria

| Milestone | Expected readiness effect |
|---|---:|
| PR-01 closed; local and CI gates green | +8 to +10 |
| PR-02 closed with stable API policy | +5 to +7 |
| PR-03 behavior made truthful | +3 to +5 |
| PR-04/PR-05 truthful, verified publication | +5 to +7 |
| Fuzz/mutation/coverage operational with reviewed evidence | +5 to +8 |
| Hosted adapters, security boundaries, and performance refreshed | +3 to +5 |

Scores are not awarded for checking boxes alone. The companion assessment must be rerun against the final commit and public evidence. A stable release requires **at least 90/100 and no open P0**, regardless of arithmetic.

## 6. Final candidate checklist

```bash
cargo fmt --all --check
cargo clippy --workspace --all-features --all-targets -- -D warnings
cargo test --workspace --all-features
RUSTDOCFLAGS='-D warnings' cargo doc --workspace --no-deps --all-features
cargo xtask examples
cargo xtask harden
cargo semver-checks --workspace
```

Then require:

- Rust 1.85 workspace tests;
- mandatory PostgreSQL/MySQL/SQLite integration;
- native `rusqlite` and `tokio-postgres` workspace feature matrices;
- Studio feature lint/tests;
- short fuzz smoke and completed mutation reports;
- non-publishing release rehearsal;
- green public CI on the exact candidate SHA;
- explicit maintainer approval for publication;
- post-publish registry and out-of-tree consumer verification.

Until that checklist is complete, do not retag, publish, or describe `1.5.0` as released.
