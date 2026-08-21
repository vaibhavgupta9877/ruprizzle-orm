# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

_Nothing yet._


## [1.0.0] - 2026-08-21

The first stable release. From this version onward the public API is covered by semantic
versioning as defined in [`docs/Stability.md`](docs/Stability.md).

There are **no API changes** between `1.0.0-rc.1` and `1.0.0` — the surface frozen for the RC
is the surface that ships. Everything below is documentation, packaging, and dependency work.

### Fixed

- **`ruprizzle-cli` produced no documentation on docs.rs.** Its only target was a `[[bin]]`
  carrying `doc = false` and the crate had no library, so rustdoc was asked to document
  nothing and docs.rs recorded `doc_status: false` for `1.0.0-rc.1` while the other nine
  crates succeeded. The crate now carries a documentation-only `src/lib.rs` that embeds its
  README; the binary target keeps `doc = false`, because its target name (`ruprizzle`) would
  otherwise collide with the runtime crate's docs in a shared `target/doc`.
- **Two broken intra-doc links in `ruprizzle`.** `executor.rs` linked to
  `rusqlite::RusqlitePool` (the type is ours, at `crate::rusqlite::RusqlitePool`) and
  `decode.rs` linked to an ambiguous `bytes`, which resolves to both our function and the
  `bytes` crate. Both live behind optional features, so `cargo doc --all-features` failed
  while the default-feature build stayed green.

### Changed

- Workspace version bumped to `1.0.0`.
- **docs.rs now builds every crate with `all-features = true`** via a new
  `[package.metadata.docs.rs]` table in all ten publishable manifests. Previously the
  published API reference silently omitted everything behind `sqlite-rusqlite`,
  `postgres-tokio-postgres`, and `metrics`.
- `criterion` `0.5` → `0.8`, `notify` `7` → `8`, `metrics` `0.23` → `0.24`. All three are
  implementation details rather than public dependencies; see the new "Public dependencies"
  section of `docs/Stability.md` for what that distinction commits us to.
- `Cargo.lock` refreshed to the latest semver-compatible versions across the tree
  (`pest` 2.9, `futures` 0.3.34, `uuid` 1.24.1, and 26 others).
- `crates/runtime/benches/query_construction.rs` uses `std::hint::black_box`;
  `criterion::black_box` is deprecated as of criterion 0.6.
- `SECURITY.md`'s supported-versions table now names `1.x` as the supported line.
- The 15 `ruprizzle-codegen` snapshots pin `RUPRIZZLE_VERSION`, so they move with the
  version bump. The generated code is otherwise byte-identical to `1.0.0-rc.1`'s.

### Docs

- `docs/Stability.md` gains a **"Public dependencies"** section naming `sqlx`, `serde`,
  `serde_json`, and `rusqlite` as part of ruprizzle's own public API, and stating the
  consequence: a major bump of any of them requires a major bump of `ruprizzle`. The 1.0 line
  is therefore pinned to `sqlx 0.8`; the `sqlx 0.9` migration is deferred to `2.0.0`.
- `docs/Stability.md` records a written **waiver of the two-week RC feedback window**, with
  its rationale and the gate matrix that stands in its place, following the same pattern as
  the W4-02 soak waiver in `docs/SoakReport.md`.
- New `ProjectPlan/v1/V1StableRelease.md`: the analysis, the two decisions, and the executable
  plan behind this release.
- New per-crate READMEs for `ruprizzle-check` and `ruprizzle-lsp`, which were the only
  published crates without one, and `readme = "README.md"` added to both manifests.
- Version references swept from `1.0.0-rc.1` to `1.0.0` across `README.md`, `docs/README.md`,
  `docs/quickstart.md`, `docs/Examples.md`, `docs/Operations.md`, `docs/faq.md`,
  `docs/announcement.md`, `docs/SUMMARY.md`, `docs/FeaturesMasterComparison.md`, and
  `docs/MigrationGuideToV1.md`.

### CI

- The `docs` job runs `cargo doc --workspace --no-deps --all-features`. Without
  `--all-features` it never compiled the feature-gated code that held the two broken links
  above, which is how they reached a published release.
- `semver-checks` covers `ruprizzle-check` and `ruprizzle-lsp`, which are published and
  semver-covered but were omitted from the package list.


## [1.0.0-rc.1] - 2026-08-21

### Added

- MySQL / MariaDB dialect and driver path through `sqlx`.
- `ruprizzle-lsp` language server and VS Code extension in `editor/`.
- `ruprizzle check` for offline / compile-time query checking.
- Aggregates, `GROUP BY` / `HAVING`, explicit `JOIN`s, CTEs, set operations,
  `EXISTS`/`IN` subqueries, JSON path operators, array operators, prepared
  statements, and nested writes in the query builder.
- Many-to-many relation support through explicit join models.
- `db pull`, `db seed`, `migrate squash`, `migrate resolve`, and `migrate reset`
  CLI commands.
- Native `tokio-postgres` and `rusqlite` driver feature flags.
- Metrics export behind the `metrics` feature.
- `cargo xtask release-check --tag <name>` — fails unless the git tag, the
  workspace version, and the `CHANGELOG.md` heading all agree. Wired into
  `release.yml` ahead of the release gate.

### Changed

- Workspace version bumped to `1.0.0-rc.1`.
- Public API reviewed and frozen for the 1.0 line.
- `.github/workflows/release.yml` now triggers on both `v1.2.3*` and `1.2.3*` tag
  shapes (previously `v*` only, so a tag cut without the prefix was a silent
  no-op), and gained a `workflow_dispatch` trigger whose `publish` input defaults
  to false so the full pipeline can be rehearsed without touching crates.io.
- `cargo xtask release` publishes `ruprizzle-check` and `ruprizzle-lsp`, which it
  previously skipped; its package list now matches `release.yml` exactly.
- `SECURITY.md` supported-versions table now reflects the real published line and
  documents the accepted `RUSTSEC-2023-0071` exception and its mitigation.
- `examples/blog` is an explicit standalone crate (`[workspace]` table of its own)
  rather than an orphan that no `cargo` invocation could reach.

### Docs

- Refreshed README, docs README, announcement, FAQ, SUMMARY, and Operations for
  `1.0.0-rc.1`.
- Added `ProjectPlan/v1/V1DocsFirstShipPlan.md` for the v1 docs + release
  roadmap.
- Reconciled the RC status claims across `README.md`, `docs/README.md`,
  `docs/Stability.md`, and `docs/announcement.md`, which had previously
  contradicted each other about whether the RC was tagged or published.
- Documented the `RUSTSEC-2023-0071` MySQL/MariaDB caveat in
  `docs/KnownLimitations.md` and the README dialect list.
- `docs/FeaturesMasterComparison.md` measured version corrected to `1.0.0-rc.1`,
  matching `docs/BenchmarkResults.md`.
- `ProjectPlan/v1/V1DocsFirstShipPlan.md` Phase 5 corrected: the blog example's
  package name is `ruprizzle-example-blog`, it is not a workspace member, and the
  release covers ten publishable crates, not eight.
- Refreshed `README.md`, `docs/README.md`, `docs/announcement.md`, `docs/faq.md`, `docs/SUMMARY.md`, and `docs/Operations.md` to the `1.0.0-rc.1` release.
- Extended `docs/FeaturesMasterComparison.md` with all 16 end-to-end and 16 query-construction benchmark operations, a new "Advanced query builder & SQL features" table, and an updated best-fit summary.
- Added prax, Prisma, and Drizzle columns and new feature rows to the high-level comparison in `docs/README.md`.
- Fixed and cross-linked internal markdown links across `README.md`, `docs/README.md`, `docs/SUMMARY.md`, `docs/announcement.md`, `docs/BenchmarkResults.md`, `docs/FeaturesMasterComparison.md`, `docs/KnownLimitations.md`, and `ProjectPlan/Enhancements/Performance/Enhancements1.md`.
- `docs/SoakReport.md`, `ProjectPlan/ProductionReadiness.md`, and related plans
  updated to record that the 48-hour W4-02 `rusqlite` soak has been waived after
  15.56 h / 1.46 B ops / 0 errors.

## [0.4.0-beta.2] - 2026-08-17


### Fixed

- `crates/runtime/tests/soak.rs` now logs per-operation errors, making it
  possible to diagnose SQLite `database is locked` failures in long soak runs.

### Changed

- `ruprizzle-testkit` is now a path-only dev-dependency in `crates/runtime`, so
  `cargo publish` for the runtime crate does not require the unpublished
  `ruprizzle-testkit` to exist on crates.io.
- Bumped the workspace version to `0.4.0-beta.2` and published all crates
  (`ruprizzle-core`, `ruprizzle-parser`, `ruprizzle-dialect`, `ruprizzle-macros`,
  `ruprizzle`, `ruprizzle-migrate`, `ruprizzle-codegen`, `ruprizzle-cli`) to
  crates.io.

### Documentation

- `docs/SoakReport.md` now records the 48-hour `rusqlite` soak as stopped early
  due to sustained `database is locked` / busy-timeout errors under concurrent
  writers.
- `docs/MutationTesting.md` now records the `ruprizzle-migrate` mutation
  baseline (28.6 % score) and documents the in-progress runtime baseline.
- `ProjectPlan/v1/V1Blockers.md` updated with the current status of the 48-hour
  soak and mutation testing gates.

## [0.4.0-beta.1] - 2026-08-17


Pre-1.0 milestone covering W2 (query surface), W3 (migrations/CLI), and W5
(operability). The runtime, CLI, migration, and codegen crates are now
competitive with Prisma/Drizzle on the measured feature set.

### Fixed

- **Transaction lifecycle on the native drivers (pre-v1 Phase 1).** Neither hand-written
  native transaction type implemented `Drop`, so a transaction abandoned rather than
  explicitly committed or rolled back — which `?` does on every early return — was
  mishandled on both. The `sqlx`-backed variants were never affected.
  - `rusqlite`: an abandoned transaction lost its connection from the pool permanently.
    After `max_connections` such drops every `begin()` failed with an exhaustion error and
    the process had to be restarted. `RusqliteTransaction` now rolls back and returns the
    connection on drop, and does so without ever panicking. (BUG-01)
  - `rusqlite`: `RusqlitePool::acquire` computed `next % conns.len()` unguarded, so holding
    `max_connections` transactions open and running any ordinary query panicked with a
    divide-by-zero. It now returns the new `Error::PoolExhausted` variant. (BUG-02)
  - `tokio-postgres`: an abandoned transaction did *not* leak a connection — it recycled one
    with `BEGIN` still open, so the next request to receive it ran inside the previous
    request's transaction, silently. Reproduced against PostgreSQL 17.10 before fixing.
    `Drop` now spawns a `ROLLBACK` with the pooled object moved into the task, so the
    connection is released only after the rollback resolves. (BUG-03)
  - `rusqlite`: a failed `COMMIT` and a failure between checkout and `BEGIN` both leaked
    their connection; both now return it.
  - `RusqliteTransaction` no longer derives `Clone`, which would have let one connection be
    returned to the pool twice. (BUG-06)

### Added

- `Error::PoolExhausted { backend }` — a typed, matchable replacement for the previous
  stringly-typed pool-exhaustion message, with a stable `kind()` of `"pool_exhausted"`.
- `PoolConfig::reset_on_recycle` (default `false`) selects `deadpool`'s `Clean` recycling
  for the native `tokio-postgres` backend, discarding session state on every checkout. It
  is off by default because it measured roughly 2× the per-query latency against a local
  database (144–178 µs versus 72–78 µs per checkout+query) and is not needed for
  correctness — abandoned transactions are rolled back before their connection is released.
- **CI**
  - Added a `native-drivers` job that builds and tests `sqlite-rusqlite` and
    `postgres-tokio-postgres`, separately and together, against a Postgres service with
    `RUPRIZZLE_REQUIRE_DB=1`. No CI job compiled either native driver before this, which is
    why the defects above reached a published release.
- **CI / supply chain**
  - Added a `deny` job that runs `cargo-deny` (advisories, licences, bans, sources) on every pull request.
  - Added `dependabot.yml` for weekly `cargo` dependency updates and monthly GitHub Actions updates.
  - Added a `harden` CI job that runs `cargo xtask harden` on pushes to `main` and on manual dispatch.
  - Added a multi-platform `test` matrix for Ubuntu, Windows, and macOS.
  - MSRV job now runs the full `cargo test --workspace` instead of only `cargo build --workspace`.
- **Generated-code gate**
  - Wired the real generated-code tests into CI through `cargo xtask examples`; the two ignored `compile` tests now run sequentially to avoid a `target/generated-check` race.
- **Panic budget**
  - `cargo xtask harden` now counts every `unwrap()`, `expect()`, `panic!`, `todo!`, and `unimplemented!` in library `src/` and fails if a crate exceeds its checked-in ceiling (`PANIC_BUDGET`).
- **Release hardening**
  - Added a CI-environment guard to `cargo xtask release` so live `cargo publish` cannot run from `CI` or `GITHUB_ACTIONS` even if `--live` is passed.
  - Publish dry-runs use `--no-verify` so they do not resolve workspace path dependencies against stale versions already on crates.io.
- **Repository metadata**
  - Added `CHANGELOG.md` and `repository.workspace` / `homepage.workspace` metadata to all workspace crates.
- **Governance**
  - Added `SECURITY.md` with supported versions, private vulnerability reporting, and an explicit scope.
  - Added `CONTRIBUTING.md` covering the CI gate, database testing requirements, `clippy::pedantic`-clean generated code, `trybuild` cases for compile-time guarantees, and `ProjectPlan/ImplementationPlan/` as the design record.
  - Linked `Contributing`, `Security policy`, and `Changelog` from the `README.md` Development section.
  - Corrected the stale status sentence in `docs/Readme.md` that treated docs-site deployment and public announcement as the only remaining work.
- **End-to-end benchmark**
  - Added `crates/runtime/benches/end_to_end` comparing ruprizzle against hand-written `sqlx` for select, include, and bulk-insert workloads, with a `cargo xtask bench-client` generator for the benchmark schema.
- **Diff-engine property tests**
  - Added `crates/migrate/tests/roundtrip_prop.rs` with `proptest` coverage of self-diff emptiness, SQL generation for schema changes, and DB-backed Postgres migration round-trips.
- **`raw!` macro / `RawFragment` escape hatch**
  - Added the `raw!` proc macro and `RawFragment` predicate for injection-safe raw SQL fragments with bound parameters.
- **ADR-009 sqlx::Any decision record**
  - Documented the runtime dialect-selection trade-off and the costs of routing all queries through `sqlx::Any`.

### Changed

- **Workspace links**
  - Updated every GitHub repository link from `https://github.com/ruprizzle/ruprizzle-orm` to `https://github.com/vaibhavgupta9877/ruprizzle-orm`.
  - Updated every GitHub Pages link from `https://ruprizzle.github.io/ruprizzle-orm` to `https://vaibhavgupta9877.github.io/ruprizzle-orm`.
- **cargo-deny configuration**
  - `unsound` now set to `"none"` and `unmaintained` scoped to workspace direct dependencies because `cargo-deny` 0.20 no longer supports `unsound = "warn"`.
  - Added `Unicode-3.0` and `Zlib` to the licence allowlist.
  - Added `allow-wildcard-paths = true` to keep `wildcards = "deny"` from flagging the internal, `publish = false` `ruprizzle-testkit` path dependency.

### Fixed

- `SelectQuery::fetch_one()` and `fetch_optional()` are now only available on
  queries without `.include(...)`. Queries with includes must use the new
  `exec_one()` / `exec_optional()` methods, which load the requested relations
  and return the single matching row. This prevents a silent `Related::Absent`
  result when a user added an include but called the non-include terminal.
  (BUG-04)
- The panic message in `Related::get()` now points at `.exec()` / `.exec_one()`
  rather than only mentioning `.include()`, which was itself the source of the
  confusion.
- `IncludeList` now correctly distributes children to every parent that shares
  a join key, rather than only the first matching parent. This requires `C:
  Clone` on the `IncludeSet` impl, matching `IncludeOne`. (BUG-08)
- `InsertManyQuery::exec` and nested `with_related` child inserts now validate
  that every row has the same columns in the same order as row 0, returning an
  error that names the offending row and column instead of silently producing
  the wrong SQL or an opaque driver error. (BUG-09)

### Changed

- CI: stale `generated-code-lint` job (which asserted the code generator was still unimplemented) replaced with the working `generated-code` gate.
- CI: `cargo-deny-action` pinned to `v2.1.1` to avoid the positional-argument regression in `v2.1.0`.
- Docs: security advisory reporting link in `ProductionReadinessPlan.md` now points at the real repository.
- Published `ruprizzle-core`, `ruprizzle-parser`, `ruprizzle-dialect`, `ruprizzle-macros`, `ruprizzle`, `ruprizzle-migrate`, `ruprizzle-codegen`, and `ruprizzle-cli` version `0.4.0-beta.1` to crates.io.

## [0.1.1-beta.1] - 2026-08-13


A beta milestone that closes the remaining alpha.3 beta blockers: clippy warnings, broken doc links, panic-audit failures in `crates/runtime`, and stale performance documentation. It also refreshes the production-readiness assessment for `0.1.0-alpha.3` and the `rusqlite` backend.

### Added

- `Pool` gained typed `as_any`, `as_sqlite`, `as_postgres`, and feature-gated `as_rusqlite` / `as_tokio_postgres` accessors.
- `crates/runtime/benches/end_to_end` now creates an `sqlx::Any` pool explicitly so the PostgreSQL benchmark path is like-for-like with hand-written `sqlx`.

### Changed

- `Pool::options`, `Pool::postgres_options`, and `Pool::sqlite_options` now return `Option<&_>` instead of panicking for the wrong variant.
- `Pool::acquire` now returns `Error::NotImplemented` for native driver-specific pools.
- The `sqlx::Executor` implementation on `&Pool` now returns a clear `sqlx::Error` for native variants instead of `unimplemented!()`.
- `Performance.md` now reports fresh PostgreSQL `sqlx::Any` numbers and the previously unmeasured bulk-insert case.

### Fixed

- Clippy warnings in `pg_any_types.rs`, `bottlenecks.rs`, `layer_attribution.rs`, `cross_orm_bench.rs`, `crates/runtime/tests/crud.rs`, `local/deep-tests`, and `crates/runtime/src/rusqlite.rs`.
- Broken intra-doc links in `crates/runtime/src/executor.rs` and `crates/testkit/src/lib.rs`.
- `rusqlite` mutex `unwrap()` calls replaced with error paths, satisfying the `crates/runtime` panic budget.
- Examples and benchmarks that passed `&Pool` to `sqlx::query` now use an explicit `Pool::Any` wrapper where appropriate.
- Updated crate-level rustdocs and user-facing docs (`README.md`, `docs/QueryGuide.md`, `docs/RelationsGuide.md`, `docs/Quickstart.md`, `docs/MigratingFrom.md`, `docs/KnownLimitations.md`) to the `0.1.1-beta.1` API and backend features.
- Moved `crates/dialect/tests/conformance.rs` into `tests/integration/tests/dialect_conformance.rs` and removed the `ruprizzle-testkit` dev-dependency from `ruprizzle-dialect` so the crate can be packaged and published.
- Published `ruprizzle-core`, `ruprizzle-parser`, `ruprizzle-dialect`, `ruprizzle-macros`, `ruprizzle`, `ruprizzle-migrate`, `ruprizzle-codegen`, and `ruprizzle-cli` version `0.1.1-beta.1` to crates.io.

## [0.1.0-alpha.2] - 2026-08-10


A quick follow-up to `0.1.0-alpha.1` that adds README files and SEO metadata to every workspace crate, improves the docs site with structured data, sitemap, and `robots.txt`, and refreshes the `book.toml` homepage URL.

### Added

- `README.md` files for every published workspace crate (`ruprizzle-core`, `ruprizzle-parser`, `ruprizzle-dialect`, `ruprizzle-macros`, `ruprizzle`, `ruprizzle-migrate`, `ruprizzle-codegen`, `ruprizzle-cli`, `ruprizzle-testkit`).
- `homepage`, `documentation`, and `readme` metadata to every `Cargo.toml`.
- `theme/head.hbs` with schema.org JSON-LD, Open Graph, and Twitter Card metadata.
- `docs/Faq.md` with an FAQPage schema.
- `sitemap.xml` and `robots.txt` wired into the GitHub Pages workflow.

## [0.1.0-alpha.1] - 2026-08-10


Initial alpha release of **ruprizzle-orm**: a schema-first ORM for Rust. Write a Prisma-style schema, get typed entities, a Drizzle-style query builder that shows you its SQL, and automatic migrations generated by diffing your schema. Postgres and SQLite. No query engine binary.

### Added

- Grammar-driven `.ruprizzle` parser with span-preserving diagnostics.
- IR lowering, fingerprinting, and round-trip serialisation.
- Postgres and SQLite dialects with conformance suites.
- Rust entity and query-builder code generation.
- Runtime CRUD, transactions, pagination, and `include` loading.
- 12 migration change classes with `up.sql` / `down.sql` generation.
- CLI: `init`, `generate`, `generate --watch`, `validate`, `format`, `migrate dev`, `migrate deploy`, `migrate status`, `migrate resolve`, `migrate reset`, `db push`, and `db seed`.
- `trybuild` compile-fail tests that enforce the type-safe query API.
- `cargo xtask` helpers for CI, examples, hardening, and release dry-runs.

### What we explicitly do not claim

- Production readiness.
- Performance superiority over raw `sqlx`.
- Feature parity with Prisma.

See `docs/KnownLimitations.md` for the full list.

[Unreleased]: https://github.com/vaibhavgupta9877/ruprizzle-orm/compare/v1.0.0...HEAD
[1.0.0]: https://github.com/vaibhavgupta9877/ruprizzle-orm/compare/v1.0.0-rc.1...v1.0.0
[1.0.0-rc.1]: https://github.com/vaibhavgupta9877/ruprizzle-orm/compare/v0.4.0-beta.2...v1.0.0-rc.1
[0.4.0-beta.2]: https://github.com/vaibhavgupta9877/ruprizzle-orm/compare/v0.4.0-beta.1...v0.4.0-beta.2
[0.4.0-beta.1]: https://github.com/vaibhavgupta9877/ruprizzle-orm/compare/v0.1.1-beta.1...v0.4.0-beta.1
[0.1.1-beta.1]: https://github.com/vaibhavgupta9877/ruprizzle-orm/compare/v0.1.0-alpha.2...v0.1.1-beta.1
[0.1.0-alpha.2]: https://github.com/vaibhavgupta9877/ruprizzle-orm/compare/v0.1.0-alpha.1...v0.1.0-alpha.2
[0.1.0-alpha.1]: https://github.com/vaibhavgupta9877/ruprizzle-orm/releases/tag/v0.1.0-alpha.1
