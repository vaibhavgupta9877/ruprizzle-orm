# Production Readiness and ORM Solution Assessment

**Project:** `ruprizzle-orm`

**Workspace version:** `1.5.0`

**Assessed branch:** local `main` (`main` is one commit ahead of `origin/main`)

**Assessed commit:** `0584934`

**Release tag inspected:** `v1.5.0` at `0e4d96b`

**Assessment date:** 2026-09-08

**Scope:** Rust workspace, generated API, migrations, SQLx/native/HTTP drivers, Studio, CLI/LSP, tests, security, CI, release automation, documentation, registry state, and public GitHub Actions evidence.

## 1. Executive verdict

| Rating | Result | Verdict |
|---|---:|---|
| **Production readiness** | **62 / 100** | **C- — strong beta, release blocked** |
| **ORM design and capability** | **9.0 / 10** | Broad, coherent, and differentiated |
| **Engineering execution** | **7.0 / 10** | Strong implementation work undermined by red final gates |
| **Release/ecosystem maturity** | **2.5 / 10** | No stable artifact is currently available from crates.io |
| **Adoption rating today** | **6.7 / 10** | Suitable for evaluation and controlled use; not a default mission-critical choice |
| **Assessment confidence** | **High** | Current source, local gates, registry output, tags, and public Actions logs were inspected |

> `ruprizzle` has a production-shaped architecture and a substantial ORM surface, but the current `1.5.0` tree is not releasable. The release pipeline, standard tests, semver gate, fuzzing, mutation evidence, streaming contract, and release documentation are not simultaneously truthful and green.

This is not the 2026-08-19 `1.0.0-rc.1` assessment carried forward. The historical v1 blockers around SQLite migration sequencing, the resumable soak accumulator, native feature compilation, and leaked streaming allocations were repaired. The present blockers are newer release-control and assurance regressions introduced or exposed by the v1.1–v1.5 line.

## 2. Release decision

### Decision: **BLOCK `1.5.0`/stable publication from the current tree**

Publication should resume only after all P0 items in `ProductionReadinessSolPlan.md` are closed:

1. restore formatting, snapshot, trybuild, all-feature, and hardening gates;
2. resolve or formally review every semver break reported against the published prerelease baseline;
3. make the `stream_unbuffered` implementation match its public contract, or rename/document the buffered fallback;
4. establish one authoritative registry/release state and remove false published-version claims;
5. verify the crates.io credential and release path without moving the existing tag;
6. obtain a green CI run on the exact commit intended for release.

The existing `v1.5.0` tag must not be moved. It points to a commit whose release run failed at `cargo fmt --all --check` before any crate was uploaded.

## 3. Weighted scorecard

| Dimension | Weight | Score | Current evidence |
|---|---:|---:|---|
| Correctness and runtime reliability | 20 | **13.0** | Broad unit/integration coverage and real Studio SQLite tests are strengths. The workspace test gate is red, generated-code snapshots drift, and `stream_unbuffered` buffers on every backend except native `tokio-postgres`. |
| Data safety and migrations | 15 | **13.0** | Transactional migration execution, checksums, drift checks, destructive classification, SQLite multi-change repair, and real Studio diffing are strong. Live provider adapter validation and mutation strength remain insufficient. |
| Test and assurance evidence | 15 | **6.0** | The suite is large, but current all-feature tests fail; fuzzing never starts; runtime mutation jobs abort on the baseline trybuild failure; migration mutation score is about 30%; the only coverage report is the old ~68% pre-v1.5 baseline. |
| Security and supply chain | 10 | **7.5** | Bound values, identifier quoting, redacted adapter tokens, `forbid(unsafe_code)`, `cargo-deny`, and private reporting are good. Studio has no authentication, MySQL retains `RUSTSEC-2023-0071`, and Actions use floating major tags rather than immutable SHAs. |
| API and semver stability | 10 | **5.0** | CI found major-version-level changes in exhaustive public enums and constructible structs across `core`, `dialect`, and `check`. A prerelease-to-stable waiver may be legitimate, but it has not been reviewed or encoded and the gate remains red. |
| Operability and observability | 10 | **7.0** | Tracing, slow-query events, metrics, pool statistics, cache/routing observability, and operations docs are substantial. Studio exposure and misleading stream memory behavior reduce operational confidence. |
| Performance and scalability | 10 | **6.5** | Benchmark harnesses and native-driver work exist, but the published benchmark narrative predates much of v1.1–v1.5 and no current evidence covers Turso/D1, Studio, replica routing, caching, or networked Postgres/MySQL at release scale. |
| Documentation and DX | 5 | **2.5** | Guides, schema docs, examples, LSP, CLI, and migration documentation are unusually broad. Release status and known-limitations documents materially contradict source and registry reality. |
| Release and ecosystem maturity | 5 | **1.5** | `v1.5.0` exists, but its release run failed. `cargo search` still reports `1.0.0-rc.1`; no stable `1.0.0`, `1.0.1`, or `1.5.0` package was observed. Live hosted adapter round trips are absent. |
| **Total** | **100** | **62.0** | **Strong beta; stable release blocked.** |

### Interpretation

- **90–100:** defensible stable production release;
- **80–89:** production-capable release candidate with bounded process risk;
- **70–79:** strong beta suitable for controlled production use;
- **60–69:** technically credible, but blocked by confirmed release or assurance defects;
- **below 60:** substantial correctness or product-integrity remediation required.

## 4. Fresh verification

### 4.1 Local checks at `0584934`

| Check | Result | Evidence |
|---|---|---|
| `cargo fmt --all --check` | **FAIL** | Formatting drift in `crates/runtime/tests/insert_validation.rs` and `crates/testkit/src/lib.rs`. |
| `cargo clippy --workspace --all-features --all-targets -- -D warnings` | **PASS** | Completed locally with Rust 1.95. |
| `cargo test --workspace --all-features` | **FAIL** | `crates/codegen` generated snapshot hash drift; a `.snap.new` was produced. Public CI also fails `runtime` trybuild output for `col_gt_on_string.stderr`. |
| `RUSTDOCFLAGS='-D warnings' cargo doc --workspace --no-deps --all-features` | **PASS** | All workspace documentation generated successfully. |
| `cargo xtask harden` | **FAIL** | Reaches the workspace test stage and inherits the current test/snapshot failures. |
| `cargo search ruprizzle --limit 20` | **FAIL release claim** | Registry output reports `ruprizzle` and the publishable v1 crates at `1.0.0-rc.1`; no `1.5.0` result appears. |

Local PostgreSQL, MySQL, Turso-hosted, and D1-hosted services were not supplied. Tests named for Postgres/MySQL may skip when URLs are absent, so they are not counted as live-database evidence.

### 4.2 Public CI and release evidence

The latest public runs on `origin/main`/`v1.5.0` are red:

- **Release run 33633504583:** `release-check` passed, then formatting failed; every publish step was skipped. Its environment showed an empty `CARGO_REGISTRY_TOKEN`, which must be verified as repository configuration before the next release attempt.
- **CI run 33631928247:** format failed; workspace, native-driver, integration, MSRV, feature-matrix, semver, and hardening jobs failed. The dominant deterministic test failure was a trybuild stderr mismatch under the pinned Rust 1.95 compiler.
- **Semver job:** reported major-version-level breaks, including new fields on publicly constructible structs, variants added to exhaustive enums, and shifted enum discriminants in `ruprizzle-core`, `ruprizzle-dialect`, and `ruprizzle-check`.
- **Fuzz run 34008573174:** both targets failed before execution. `rust-toolchain.toml` selected stable 1.95 despite the workflow installing nightly, so sanitizer `-Z` flags were rejected.
- **Mutation run 34082283290:** all runtime shards aborted because the baseline trybuild suite was already red. The migration job completed 626 mutants: 169 caught, 391 missed, 53 unviable, and 13 timed out.

A green historical run cannot substitute for a green run on the release commit.

## 5. Architecture and solution quality

### 5.1 Architecture assessment

The architecture remains one of the project's strongest attributes:

1. parser and diagnostics lower into a shared core schema IR;
2. dialect capabilities isolate backend-specific SQL/DDL behavior;
3. code generation builds a typed model/column/client surface;
4. runtime builders compile visible SQL plus bind values through an `Executor` abstraction;
5. migrations separate planning, rendering, introspection, drift, and application;
6. CLI/LSP/check consume the same schema semantics;
7. SQLx, native drivers, HTTP adapters, routing, caching, and instrumentation sit behind explicit boundaries.

The highest-blast-radius contracts remain the core IR, `DbDialect`, compiler/query builders, `Executor`, migration planner/runner, generated API shape, and public diagnostic enums. The semver failures confirm that these central types need stronger evolution rules than they currently have.

### 5.2 Capability scorecard

| Capability | Score | Assessment |
|---|---:|---|
| Architecture | **9.2 / 10** | Strong layering and extensibility; central public IR increases semver blast radius. |
| Type safety | **9.1 / 10** | Typed columns, model-scoped filters, generated clients, and compile-fail tests are excellent. |
| SQL transparency | **9.5 / 10** | Visible SQL, bind preservation, raw-fragment binding, and explicit dialect behavior remain standout features. |
| Query and mutation surface | **9.2 / 10** | CRUD, aggregates, joins, CTEs, set operations, arrays, search, soft deletes, nested writes, trees, caching, and routing form a serious ORM surface. |
| Relations | **8.9 / 10** | Batched includes, explicit joins, nested writes, M:N support, and hierarchy helpers are broad. |
| Migrations and data safety | **8.8 / 10** | Mature safety model and repaired SQLite sequencing; mutation evidence remains weak. |
| Backend portability | **8.5 / 10** | Three SQL dialects, two native paths, and Turso/D1 HTTP adapters; hosted-service compatibility remains unverified. |
| Developer experience | **9.0 / 10** | Schema DSL, generator, CLI, formatter, LSP, Studio, introspection, seeding, and offline checking are unusually complete. |
| Performance evidence | **7.0 / 10** | Harnesses exist, but evidence has not kept pace with the current product surface. |
| Maintainability | **7.2 / 10** | Crate boundaries are sound; duplicated status prose, exhaustive public enums, snapshots, and a wide feature matrix impose high maintenance cost. |
| **Overall ORM design/capability** | **9.0 / 10** | A differentiated full ORM, not a thin SQL wrapper. |

### 5.3 Competitive position

- **Diesel:** safer ecosystem maturity and long production history; ruprizzle offers a more approachable schema-first generated workflow.
- **SeaORM:** broader market validation; ruprizzle offers stronger SQL visibility and centralized schema/codegen semantics.
- **SQLx:** preferable for handwritten database-checked SQL; ruprizzle adds ORM relations, migrations, generated clients, and higher-level workflows.
- **Prisma/Drizzle:** ruprizzle combines schema ownership and transparent in-process Rust execution, but lacks their ecosystem size and production evidence.

The defensible differentiator remains: **a Prisma-style schema and generated Rust client with visible bound SQL and no sidecar engine**.

## 6. Confirmed strengths

1. Real and broad ORM functionality across query, relation, mutation, migration, tooling, and observability layers.
2. Strong parameter-binding posture and explicit identifier quoting on dynamic Studio paths.
3. Compile-time model/type scoping and negative compile tests.
4. Transaction/savepoint support and migration checksums, locks, drift detection, and destructive classification.
5. Real Studio data access and migration comparison rather than the earlier fabricated implementation.
6. Honest Turso/D1 limitations in current adapter source, plus redacted credentials and local protocol tests.
7. MSRV declaration, cross-platform CI design, native-feature matrices, docs checks, hardening audits, and semver automation exist even though several are currently red.
8. No `TODO`, `FIXME`, `todo!`, or `unimplemented!` markers were found in production crate Rust sources.
9. Library crates retain a no-unsafe policy and dependency licensing/advisory checks.
10. Documentation breadth and architectural decisions remain substantially above average for a project at this ecosystem maturity.

## 7. Confirmed issues and risks

### 7.1 P0: the release commit fails its first mechanical gate

The tag-triggered release stopped at formatting. Current HEAD still fails the same check. A release process that cannot pass its deterministic first gate is correctly blocked.

### 7.2 P0: standard and all-feature tests are not green

The Rust 1.95 trybuild output no longer matches the committed stderr fixture, which breaks default, MSRV, native-feature, integration, mutation, and hardening jobs that transitively execute it. Independently, all-feature execution finds a stale generated-code snapshot hash. These are test-maintenance defects, but their breadth means there is no trusted green baseline.

### 7.3 P0: semver enforcement reports unreviewed breaking changes

Public exhaustive enums gained variants, public constructible structs gained fields, and implicit enum discriminants shifted. These are real source-compatibility breaks for downstream exhaustive matches, struct literals, and numeric casts. Because crates.io currently exposes only the prerelease, a one-time prerelease-to-stable decision may be defensible; silently ignoring the gate is not.

### 7.4 P0: `stream_unbuffered` does not honor its documented contract

`Executor::stream_unbuffered_raw` defaults to buffered `stream_raw`; `Pool` delegates true unbuffered behavior only to native `tokio-postgres`. SQLx PostgreSQL, SQLx MySQL, SQLx SQLite, native `rusqlite`, Turso, and D1 therefore materialize a complete result before yielding despite API docs promising incremental SQLx rows. The former `Box::leak` defect is gone, but the current fallback can cause unexpected peak memory and invalidates the method name and documentation.

### 7.5 P0: release and registry truth is contradictory

`CHANGELOG.md` says `1.5.0` is released and latest. The README says `1.0.1` is latest and `1.5.0` is unpublished. `SECURITY.md` says `1.0.0` is current. `docs/KnownLimitations.md` calls the project a beta and defers features already implemented. Registry search reports only `1.0.0-rc.1`. Users cannot determine what artifact exists or what is supported.

### 7.6 P1: fuzz assurance is nonfunctional

The workflow installs nightly but the repository toolchain file selects stable 1.95 for the actual `cargo fuzz` invocation. Both targets fail before consuming one input, so the four-hour comments describe a gate that currently provides zero fuzz time.

### 7.7 P1: mutation evidence is both weak and operationally misleading

Migration mutation testing catches roughly 30% of tested viable outcomes and leaves core splitter behavior mutable. Runtime shards currently measure nothing because baseline tests fail first. The workflow marks expected survivors as a generic failed run without a checked threshold or durable summarized trend.

### 7.8 P1: coverage evidence is stale and incomplete

The documented 68.08% line coverage predates Studio and most v1.1–v1.5 functionality, and no coverage workflow exists. Current coverage is unknown; release-critical low-coverage areas cannot be tracked for regression.

### 7.9 P1: hosted adapter compatibility is unverified

Turso and D1 are implemented and tested against local HTTP fixtures, but neither has completed a real hosted round trip. Their lack of interactive transactions and fully buffered streaming are documented limitations, not defects, but production compatibility remains an evidence gap.

### 7.10 P1: Studio is intentionally unauthenticated

Loopback defaults and the non-loopback write confirmation reduce accidental exposure, but `--yes-i-know` can expose mutation routes with no authentication. Studio should remain a local development tool until authenticated remote operation exists.

### 7.11 P1: accepted MySQL dependency risk remains

`sqlx-mysql` still reaches `rsa 0.9.x` and `RUSTSEC-2023-0071`. TLS or Unix sockets avoid the affected key-exchange path. MySQL should not be marketed as production-grade without that mitigation until the dependency path is patched.

### 7.12 P2: CI action references are mutable

Actions use major/version tags such as `actions/checkout@v4` and `Swatinem/rust-cache@v2`, not immutable commit SHAs. This is common but weaker than the repository's otherwise strong supply-chain posture.

## 8. Use-case recommendation

| Use case | Recommendation now |
|---|---|
| Learning, evaluation, prototype | **Yes** |
| Controlled internal SQLite/Postgres tool | **Conditional**: pin a commit, run application-specific migrations/tests, and avoid relying on `stream_unbuffered` memory bounds |
| New non-critical service | **Conditional after P0 closure** |
| Mission-critical production database | **No stable endorsement yet** |
| MySQL production | **Only with TLS/Unix socket and explicit advisory acceptance** |
| Large-result true streaming | **Use only verified native `tokio-postgres`; other current paths buffer** |
| Turso/D1 production | **Pilot only after a real hosted-service qualification run** |
| Studio on a reachable network | **Do not expose without an external authenticated tunnel/proxy; prefer loopback** |

## 9. Path to a defensible release

1. Close PR-01 through PR-05 in the implementation plan and obtain a completely green local release gate.
2. Push through `dev-main` according to repository policy and require green CI on the exact candidate commit.
3. Repair fuzz and mutation workflows, record current coverage, and qualify hosted adapters.
4. Resolve documentation from one machine-readable release-state source.
5. Verify the crates.io secret through a non-publishing rehearsal; do not move `v1.5.0`.
6. Publish under a new, traceable version/tag if the existing failed tag cannot identify the final source.
7. Verify every published package and an out-of-tree consumer before announcing availability.

## 10. Final assessment

`ruprizzle` is technically ambitious and much of that ambition is implemented well. Its architecture, typed API, SQL transparency, migration model, Studio remediation, observability, caching, routing, and edge adapters justify a high ORM capability score.

Production readiness is lower because release truth is binary: the tagged artifact was not published, the current gates are red, the semver policy rejects the surface, scheduled assurance jobs do not provide the evidence they claim, and one public streaming contract is false for most backends. These are fixable without redesigning the ORM.

The honest current label is **strong beta, release blocked**. Closing the focused plan can raise readiness quickly; adding more features before then would lower confidence rather than increase product value.
