# Production Readiness and ORM Solution Assessment

> **Note (2026-08-21):** This assessment is a snapshot from 2026-08-19. The
> 48-hour W4-02 `rusqlite` soak has since been **waived** after 15.56 h /
> 1.46 B ops / 0 errors (see `ProjectPlan/ProductionReadiness.md` §14/§15 and
> `docs/SoakReport.md`).
>
> As of the same date, the red `fmt`/`clippy`/`test`/`xtask` gates
> (`ProjectPlan/ProductionReadinessSolPlan.md` V1-01) and the `sqlite-rusqlite`
> feature suite compile failures are **closed**; the `stream_unbuffered` `Box::leak`
> issue (V1-04) is also **closed** because the current source no longer contains
> `Box::leak` in the streaming path and all streaming tests pass. The SQLite
> multi-change migration planner (V1-03) is also **fixed**. The remaining current
> blockers are the RC/publish/feedback window (V1-05) and coverage/mutation
> evidence (V1-06).

**Project:** `ruprizzle-orm`
**Workspace version:** `1.0.0-rc.1`
**Assessed branch:** `dev-v0-2`
**Assessed commit:** `7dd3b6a`
**Assessment date:** 2026-08-19
**Scope:** Rust workspace, published package set, runtime and native drivers, migrations, CLI/DX, tests, release automation, documentation, and available ecosystem evidence.

## 1. Executive verdict

| Rating | Result | Verdict |
|---|---:|---|
| **Stable-v1 production readiness** | **67 / 100** | **C — strong beta, not RC/GA ready** |
| **ORM design and capability** | **8.6 / 10** | Strong and differentiated |
| **Engineering execution** | **7.7 / 10** | Good foundations, inconsistent final-gate discipline |
| **Ecosystem/release maturity** | **3.5 / 10** | Very early; RC process not executed |
| **Overall adoption rating today** | **7.2 / 10** | Attractive for evaluation and bounded use; not yet a default mission-critical choice |
| **Assessment confidence** | **High for local code; medium for remote history** | Local gates and source were inspected live; remote GitHub Actions history was unavailable without `gh` authentication |

The central conclusion is:

> `ruprizzle` is already a credible ORM solution, but the current repository is not a releasable `1.0.0-rc.1` candidate and should not be promoted to stable `1.0.0` yet.

The low production score is not a judgment that the architecture is weak. It reflects fresh release-gate failures, an acknowledged SQLite migration planner defect, an unbounded memory leak in true streaming, an invalid resumable-soak accounting path, incomplete long-duration assurance, and an RC/release state that conflicts with the version and user documentation.

The crate is best described as **feature-complete but assurance-incomplete**.

## 2. Hard-gate result

Stable v1 is currently blocked by all of the following:

1. `cargo xtask harden` fails because `soak_rusqlite_resumable_48h` runs during the normal workspace suite and panics when `RUPRIZZLE_SOAK_DB_PATH` is absent.
2. The documented `sqlite-rusqlite` feature test command does not compile because `crates/runtime/tests/query_manifest.rs::Task` lacks the native row-decoding traits required by `Model` under that feature.
3. The 48-hour W4-02 `rusqlite` soak has been **waived** after the resumable
   segmented run reached **15.56 h / 1.46 B ops / 0 errors**. The original
   continuous run stopped at ~11 h with two I/O errors; the resumable harness
   has since fixed the accounting and logging issues.
4. The resumable segmented soak now accumulates cumulative elapsed, operations,
   and errors across restarts; the accepted 15.56 h / 0-errors result is
   recorded in `docs/SoakReport.md`.
5. SQLite migration planning is known to fail for multi-change diffs such as adding multiple required columns; the property test excludes those cases with `prop_assume!(changes.len() <= 1)`.
6. `stream_unbuffered` permanently leaks owned SQL and bind values with `Box::leak` on the SQLx and `tokio-postgres` paths.
7. The RC is not published on crates.io. A local `1.0.0-rc.1` tag exists, but it resolves 20 commits behind HEAD, is not present on `origin`, and does not match the release workflow's `v*` trigger.
8. The required two-week RC feedback window and external upgrade report have not happened.

These are release blockers, not optional post-v1 polish.

## 3. Weighted production-readiness scorecard

| Dimension | Weight | Score | Evidence and deduction |
|---|---:|---:|---|
| Correctness and runtime reliability | 20 | **13.5** | Broad default-path tests and compile-fail tests exist, but the normal workspace gate is red, the native-rusqlite matrix does not compile, and true streaming leaks memory per call. |
| Data safety and migrations | 15 | **10.5** | Transactional application, checksums, locking, drift detection, destructive gating, and dev/deploy separation are strong. The known SQLite multi-change planner defect is a stable-v1 blocker. |
| Test and assurance evidence | 15 | **7.0** | Good test breadth, property tests, fuzz/mutant workflows, and short soaks. The 48-hour W4-02 run has been waived on 15.56 h / 1.46 B ops / 0-errors evidence, the resumable segmented accounting has been fixed, migration mutation score is about 28.6%, runtime mutation baseline is incomplete, and measured line coverage is about 68%. |
| Security and supply chain | 10 | **8.5** | Parameter binding, injection tests, `forbid(unsafe_code)`, `cargo-deny`, hardening audits, and private reporting are good. `RUSTSEC-2023-0071` remains excepted through the MySQL dependency path and `SECURITY.md` is stale for the current release line. |
| API and semver stability | 10 | **8.0** | Public API review, stability policy, MSRV policy, and semver CI are strong. The actual RC artifact/window is absent, the local tag is stale, and final API review must be rerun against the artifact that will be published. |
| Operability and observability | 10 | **6.5** | Tracing, slow-query warnings, metrics hooks, pool configuration, and operations documentation exist. Native `rusqlite` pool stats report zeros, segmented soak progress writes can fail silently, and the true-streaming API leaks. |
| Performance and scalability | 10 | **7.0** | Reproducible cross-ORM SQLite data and low query-construction cost are valuable. The headline native-rusqlite results predate the current `spawn_blocking` implementation, so current docs describe a different execution path; cross-ORM Postgres evidence is absent. |
| Documentation and DX | 5 | **4.0** | The schema DSL, guides, ADRs, examples, LSP, CLI, offline checking, and limitations documentation are unusually complete. Several release/install/status claims are currently false or stale. |
| Release and ecosystem maturity | 5 | **2.0** | Beta packages exist and automation is designed, but crates.io still serves `0.4.0-beta.2`, no remote RC tag exists, the release workflow has not published the RC, and there is no RC feedback evidence. |
| **Total** | **100** | **67.0** | **Strong beta; stable-v1 gates not met.** |

### Score interpretation

- **90–100:** stable production release with completed assurance and real-world validation.
- **80–89:** production-capable RC with limited remaining process risk.
- **70–79:** strong beta; suitable for bounded production use with explicit risk ownership.
- **60–69:** technically promising but blocked by confirmed release/correctness/assurance gaps.
- **Below 60:** not suitable for production evaluation without substantial remediation.

`ruprizzle` lands at the top of the 60–69 band because the architecture and default paths are strong, but multiple red gates are current and reproducible.

## 4. Live verification performed

All commands were run on Windows at `7dd3b6a` with Rust `1.95.0`. This does not replace the CI job on the declared MSRV, Rust `1.85`.

| Check | Command | Fresh result |
|---|---|---|
| Format | `cargo fmt --all --check` | **Pass** |
| Default clippy | `cargo clippy --workspace --all-targets -- -D warnings` | **Pass** |
| Standard workspace tests | `cargo test --workspace` through `cargo xtask harden` | **Fail**: `soak_rusqlite_resumable_48h` panics because `RUPRIZZLE_SOAK_DB_PATH` is missing |
| Remainder of workspace tests | `cargo test --workspace -- --skip soak_rusqlite_resumable_48h` | **Pass** |
| Docs | `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps` | **Pass** |
| Dependency policy | `cargo deny check` | **Pass**, with configured warnings/exception policy |
| Workspace check | `cargo check --workspace` | **Pass** |
| Full hardening | `cargo xtask harden` | **Fail** at the standard workspace test stage; later audits are therefore not reached by this command |
| Native rusqlite feature | `RUPRIZZLE_TEST_RUSQLITE=1 cargo test -p ruprizzle --features 'sqlite-rusqlite,ruprizzle-testkit/sqlite-rusqlite'` | **Compile failure** in `query_manifest.rs`: missing `FromOwnedRow` and `FromRusqliteRow` for `Task` |
| Registry state | `cargo search ruprizzle --limit 10` | Latest public runtime/CLI/internal crates are `0.4.0-beta.2`; `1.0.0-rc.1` is not published |
| Tag state | `git show-ref --tags`, `git ls-remote --tags origin`, `git rev-list --count 1.0.0-rc.1..HEAD` | Local RC tag only; no remote tags returned; local tag is 20 commits behind HEAD |

### Verification limitations

- PostgreSQL/MySQL were not made mandatory locally with `RUPRIZZLE_REQUIRE_DB=1`; local green database-named tests are not treated as proof of live cross-database execution.
- Remote Actions history could not be queried because `gh` is unauthenticated. Workflow configuration was inspected, but successful historical execution is not assumed.
- The full 48-hour soak and multi-hour fuzz/mutation jobs were not rerun as part of this assessment.
- No claim is made that repository benchmark results generalize from local SQLite to networked production databases.

## 5. Graphify and Rust-aware architecture analysis

### 5.1 Graph corpus

The current Graphify report covers the 79 production source/document files under `crates/`:

- **1,739 nodes**
- **3,077 edges**
- **141 communities**
- **93% extracted edges**
- **7% inferred edges**
- **0% ambiguous edges in the summary count**

A fresh detector sees 161 supported crate files when tests and benches are included: 152 code files and 9 documents, about 143,257 words. Production architecture was taken from the committed graph; test evidence was reviewed separately so test fixtures did not distort centrality.

### 5.2 Central abstractions and blast radius

Graphify identifies the query builder and database abstraction layer as the highest-connectivity production core. Rust Analyzer confirms the important definitions and reference spread.

| Abstraction | Role | Risk if changed |
|---|---|---|
| `SelectQuery<'db, M, Out, I>` | Main typed read/query surface | High: used across runtime, deep tests, integration tests, examples, and benchmarks |
| `DbDialect` | SQL/DDL capability and rendering contract | Very high: referenced by all dialects, compiler, executor, transactions, counting, and conformance tests |
| `Compiler<'d>` | Converts typed query structures into dialect SQL | Very high: a defect affects many query builders and all backends |
| `Planner<'a>` | Orders and renders migration changes | Very high for data safety, but internally contained within `migrate` |
| `Tx` / `Executor` | Pool/transaction substitution and lifecycle | High: shared by CRUD, nested writes, migration execution, and native drivers |
| `Column<M, T>` | Model/type-scoped compile-time safety token | High API importance; changes affect generated clients and filters |
| `Pool` | Driver dispatch and operational statistics | High: covers SQLx Any/native plus optional `rusqlite` and `tokio-postgres` paths |
| Core `Schema` IR | Contract between parser, dialect, codegen, migrations, and tooling | Highest cross-crate semver blast radius |

The architecture is modular, but these hubs deserve disproportionately strong property, mutation, feature-matrix, and compatibility testing. The graph's isolated/thin nodes are treated as extraction/documentation gaps, not automatically as code defects.

### 5.3 Healthy architecture decisions

1. **Build-time/runtime separation.** Parser and code generation do not need to enter the normal application runtime dependency graph.
2. **Stable central IR.** Parser, dialect, migration, codegen, CLI, and LSP share one schema model instead of translating among unrelated representations.
3. **Typed column tokens.** `Column<M, T>` prevents wrong scalar types and cross-model filters at compile time without representing the entire SQL AST in Rust's type system.
4. **SQL transparency.** Builders expose SQL and binds through `to_sql`; raw fragments preserve parameter binding.
5. **Explicit capabilities.** Backend differences are represented by `DbDialect` and `Capabilities` rather than scattered backend-name checks alone.
6. **Executor substitution.** The same builders operate against pools and transactions.
7. **Production-safe migration command split.** `migrate deploy` applies existing files and does not generate new migration plans.
8. **No proprietary sidecar.** Runtime execution stays in-process and builds on established database drivers.

### 5.4 Architecture risks

1. **Feature cross-product.** Query types × dialects × driver implementations × pool/transaction modes create a larger behavioral matrix than the default build demonstrates.
2. **Wide semver surface.** Runtime builders, lower-level crates, generated client shape, CLI behavior, and core IR are all promised as stable. This is an ambitious commitment for a new single-maintainer project.
3. **Object-safe trait constraints.** `DbDialect` and `Executor` improve substitution but make ownership-heavy APIs such as streaming harder to evolve safely.
4. **Migration planner sequencing.** Planning changes independently is insufficient when one operation rebuilds a table using final-schema state while sibling changes have not yet executed.
5. **Native-path parity.** Optional drivers add meaningful performance and deployment choices, but every test model and operational API must satisfy additional traits and lifecycle rules.
6. **Documentation drift.** The large planning/document corpus contains conflicting state claims. Generated or centrally checked release facts are preferable to repeated manual status prose.

## 6. ORM solution-quality rating

### 6.1 Capability scorecard

| Capability | Score | Assessment |
|---|---:|---|
| Architecture | **9.2 / 10** | Excellent separation and clear core contracts; wide surface increases maintenance cost |
| Type safety | **9.0 / 10** | Strong generated model/column safety and compile-fail coverage; not identical to database-validated handwritten SQL |
| SQL transparency and escape hatches | **9.5 / 10** | One of the strongest parts of the design |
| Query and mutation surface | **8.8 / 10** | CRUD, aggregates, joins, CTEs, subqueries, set ops, pagination, prepared queries, conditional building, nested writes |
| Relations | **8.5 / 10** | Batched includes, explicit joins, nested writes, self relations, and explicit M:N are substantial; some ergonomic helpers are deliberately deferred |
| Migration design | **8.5 / 10** | Strong model and safety posture; current SQLite sequencing bug reduces implementation confidence |
| Dialect/driver portability | **8.8 / 10** | Postgres, SQLite, MySQL plus optional native drivers; parity evidence is currently red for `sqlite-rusqlite` |
| Developer experience | **8.7 / 10** | Schema DSL, generator, CLI, formatter, LSP, diagnostics, introspection, seeding, offline checking, guides |
| Performance evidence | **7.5 / 10** | Useful and reproducible SQLite harness; current native implementation has drifted from the benchmark narrative |
| Maintainability | **7.5 / 10** | Good crate boundaries and tests; high surface area, a large central runtime, and state-document duplication raise cost |
| **Design/capability result** | **8.6 / 10** | A serious ORM proposition, not a toy or thin query-builder wrapper |

### 6.2 Competitive position

Based on the repository's reproducible evidence and public API, not unverified market claims:

- **Versus Diesel:** `ruprizzle` is more approachable for schema-first/codegen users and has automatic schema diffing; Diesel remains the safer maturity choice and can outperform automatic relation loading with hand-tuned joins.
- **Versus SeaORM:** `ruprizzle` offers a stronger single-source schema/codegen story and more SQL transparency; SeaORM has broader production history and ecosystem confidence.
- **Versus SQLx:** `ruprizzle` adds a full ORM, migrations, relations, and generated client; SQLx remains preferable when handwritten, database-checked SQL is the primary requirement.
- **Versus Prisma/Drizzle:** `ruprizzle` combines a Prisma-like schema workflow with Drizzle-like visibility without a sidecar, but lacks their ecosystem scale and production exposure.

The strongest differentiator is not a universal speed claim. It is:

> Prisma-style schema ownership plus a generated Rust client, visible SQL, bound parameters, and no hidden query engine.

## 7. Confirmed current strengths

1. Broad typed query surface, including advanced SQL features.
2. Compile-time wrong-type and cross-model rejection tests.
3. Batched relation loading that avoids uncontrolled N+1 behavior.
4. Explicit transaction, savepoint, and nested-write support.
5. Migration checksums, locking, drift detection, destructive gating, and deploy/dev separation.
6. Postgres/SQLite/MySQL dialect model with optional native paths.
7. Tracing, slow-query signals, metrics hooks, and pool configuration.
8. LSP, formatter, watch mode, diagnostics, introspection, seeding, and offline query manifests.
9. No unsafe library code and strong parameter-binding posture.
10. High-quality guides, ADRs, limitations, benchmark harnesses, and project history.

## 8. Confirmed blockers and material risks

### 8.1 Standard release gate is red

`crates/runtime/tests/soak_resumable.rs` defines an unconditional `#[tokio::test]`. A normal `cargo test --workspace` invokes it, but the test immediately requires `RUPRIZZLE_SOAK_DB_PATH`. This makes both the documented full suite and `cargo xtask harden` fail outside the dedicated runner.

**Root cause:** a manually invoked, environment-dependent, 48-hour gate was added as a normal test instead of an ignored/explicit test target.

### 8.2 Native rusqlite feature matrix is red

`query_manifest.rs::Task` derives only `sqlx::FromRow`. Under `sqlite-rusqlite`, `Model: RowDecode` also requires `FromRusqliteRow` and `FromOwnedRow`. Comparable runtime test models use the project-provided native row macros; this test does not.

**Root cause:** a new hand-written test model was added without compiling every supported feature combination.

### 8.3 Segmented soak result is not yet trustworthy

The resumable harness loads cumulative state, but reporter/final updates assign:

- `state.total_ops = current_ops`
- `state.total_errors = current_errors`

Those counters start at zero per process. On resume, prior totals are overwritten rather than incremented. Earlier errors can therefore disappear from the persisted result. In addition, state-write errors are discarded, and the harness does not implement the forced failover named by the project exit gate.

The harness must be corrected and tested before accumulating the official 48 hours.

### 8.4 SQLite multi-change migrations can fail

The local round-trip property excludes any diff with more than one change. The source comment records that multiple required-column adds can make the first SQLite rebuild select a sibling column that does not yet exist.

This is a planner sequencing problem: table rebuilds are generated from the final model while add/backfill/alter operations are interleaved per field.

### 8.5 True streaming leaks memory

Both the SQLx pool implementation and `tokio-postgres` implementation use `Box::leak` for owned SQL and bind collections. Every dynamic unbuffered stream permanently consumes memory, even after completion or cancellation.

Documenting this does not make it production-safe. Before freezing v1, the implementation must own and release query state or the API must be removed/held behind an unstable feature.

### 8.6 Native pool metrics are inaccurate

`Pool::size()` and `Pool::num_idle()` return zero for `Pool::SqliteNative`, and `num_waiters()` reports zero for every backend except native Postgres. The 60-second soak report explicitly shows zero pool statistics for `rusqlite`.

This does not break queries, but it weakens overload diagnosis on the backend currently receiving the most performance emphasis.

### 8.7 Benchmark narrative is stale

`docs/BenchmarkResults.md` says native `rusqlite` runs on the calling Tokio task with no `spawn_blocking`. Current `RusqlitePool::fetch_all_raw` and `execute_raw` dispatch through `tokio::task::spawn_blocking`.

The previous result files remain useful historical measurements, but they are not evidence for current-HEAD native performance until rerun.

### 8.8 Release state is internally inconsistent

- Workspace and docs use `1.0.0-rc.1`.
- crates.io search returns `0.4.0-beta.2` as latest.
- A local RC tag exists but is 20 commits behind HEAD.
- No tag is visible on `origin` through `git ls-remote --tags origin`.
- The local tag is `1.0.0-rc.1`; `release.yml` triggers only tags matching `v*`.
- User docs contain `ruprizzle = "1.0.0-rc.1"`, which cannot currently resolve from crates.io.
- Some docs say the RC is tagged/staged while others say no RC has been tagged or published.

This must be corrected before inviting external users into an RC feedback window.

### 8.9 Security posture requires release-specific clarification

The dependency exception for `RUSTSEC-2023-0071` is documented and bounded to the MySQL authentication path, but it remains an accepted risk. `SECURITY.md` still says only `0.1.x` is supported, while the registry's latest package is `0.4.0-beta.2` and the workspace claims an RC.

## 9. Use-case recommendation

| Use case | Recommendation today |
|---|---|
| Learning, evaluation, prototype | **Yes** |
| Internal tool with SQLite/Postgres and controlled migrations | **Conditional yes**: pin the exact beta/commit, test generated migrations, avoid `stream_unbuffered`, and own upgrade risk |
| New non-critical service | **Conditional** after the two mechanical gates are repaired and application tests cover the chosen driver |
| Mission-critical production database | **No stable endorsement yet** |
| Native `rusqlite` under sustained concurrent load | **Conditional yes** for the workloads covered by the accepted 15.56 h / 0-errors evidence; the full 48-hour target is waived |
| Workload requiring true unbuffered streaming | **Do not use the current API** because each dynamic stream leaks memory |
| MySQL deployment using non-TLS RSA key exchange | **Avoid until the advisory path is resolved or explicitly mitigated** |
| Team prioritizing mature ecosystem over schema-first DX | Prefer Diesel, SeaORM, or SQLx according to query style |

## 10. What should and should not enter v1

### Must enter v1

- Green standard and native-feature release gates.
- Correct segmented-soak accounting and accepted 15.56 h / 0-errors evidence (the full 48-hour target is waived).
- Correct SQLite multi-change migration planning.
- Leak-free true streaming, or removal from the stable API.
- Accurate RC artifact/tag/workflow/docs state.
- Final API/generated-client/CLI compatibility review against the actual RC.
- Risk-based test improvements for migration/runtime critical paths.
- Accurate native pool telemetry and current benchmark claims.

### Should not expand v1

Do not add full-text search, PostGIS, soft deletes, polymorphic relations, implicit many-to-many tables, recursive tree APIs, MSSQL, vector search, multi-tenancy/RLS abstractions, or a hosted Studio before the current blockers close. These features increase the semver and test matrix while doing nothing to repair the release evidence.

## 11. Final assessment

`ruprizzle` has a stronger design than its current readiness score suggests. Its schema-first workflow, typed columns, SQL visibility, migration model, multi-dialect architecture, and tooling make it one of the more interesting Rust ORM designs in the repository's comparison set.

The correct next move is not more feature breadth. It is to make the existing promise true:

1. restore every release gate,
2. remove known correctness and lifetime defects,
3. produce trustworthy long-duration and mutation evidence,
4. publish an internally consistent RC,
5. collect real external feedback,
6. then freeze and ship `1.0.0`.

Until those steps are complete, the honest label is **strong beta**, not production-stable v1.
