# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

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
  - Corrected the stale status sentence in `docs/README.md` that treated docs-site deployment and public announcement as the only remaining work.
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

- CI: stale `generated-code-lint` job (which asserted the code generator was still unimplemented) replaced with the working `generated-code` gate.
- CI: `cargo-deny-action` pinned to `v2.1.1` to avoid the positional-argument regression in `v2.1.0`.
- Docs: security advisory reporting link in `ProductionReadinessPlan.md` now points at the real repository.

## [0.1.0-alpha.2] - 2026-08-10

A quick follow-up to `0.1.0-alpha.1` that adds README files and SEO metadata to every workspace crate, improves the docs site with structured data, sitemap, and `robots.txt`, and refreshes the `book.toml` homepage URL.

### Added

- `README.md` files for every published workspace crate (`ruprizzle-core`, `ruprizzle-parser`, `ruprizzle-dialect`, `ruprizzle-macros`, `ruprizzle`, `ruprizzle-migrate`, `ruprizzle-codegen`, `ruprizzle-cli`, `ruprizzle-testkit`).
- `homepage`, `documentation`, and `readme` metadata to every `Cargo.toml`.
- `theme/head.hbs` with schema.org JSON-LD, Open Graph, and Twitter Card metadata.
- `docs/faq.md` with an FAQPage schema.
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

See `docs/known-limitations.md` for the full list.

[Unreleased]: https://github.com/vaibhavgupta9877/ruprizzle-orm/compare/v0.1.0-alpha.2...HEAD
[0.1.0-alpha.2]: https://github.com/vaibhavgupta9877/ruprizzle-orm/compare/v0.1.0-alpha.1...v0.1.0-alpha.2
[0.1.0-alpha.1]: https://github.com/vaibhavgupta9877/ruprizzle-orm/releases/tag/v0.1.0-alpha.1
