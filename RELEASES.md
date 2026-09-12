# Release notes

For a sectioned, versioned changelog, see [CHANGELOG.md](CHANGELOG.md).

> **Current registry state: `1.0.0-rc.1`.** That is the only version of any
> ruprizzle crate available from crates.io. `ruprizzle-turso` and `ruprizzle-d1`
> have never been published. The `v1.0.0`, `v1.0.1` and `v1.5.0` tags all exist
> without a corresponding package, because their publish runs failed before
> uploading. Run `scripts/check-release-state.sh` before writing anything in this
> file that describes a version as released; a tag is not a release.


## Publishing

Releases are cut from `main`. `dev-main` is the integration branch; it merges into
`main` once per release, carrying the version bump, the `CHANGELOG.md` entry and the
tag. See [CONTRIBUTING.md](CONTRIBUTING.md#branches).

1. On `dev-main`: bump `workspace.package.version` and every internal pin in
   `Cargo.toml`, write the `CHANGELOG.md` entry, and add the section here.
2. `cargo xtask release-check --tag vX.Y.Z` — asserts that the tag, the workspace
   version, the internal pins, the changelog and the VS Code extension all agree.
3. `cargo xtask ci` and `cargo xtask harden` — the full gate, including the publish
   coverage audit that checks the crate list below against `PUBLISH_ORDER` in `xtask`
   and against `.github/workflows/release.yml`.
4. `cargo xtask release` — dry-runs `cargo package` for every crate, in order.
5. Confirm `CARGO_REGISTRY_TOKEN` is set under Settings → Secrets and variables →
   Actions, and that it is visible to the workflow. A `workflow_dispatch` run with
   `publish` left false exercises the whole gate without uploading; the workflow's
   credential preflight fails fast rather than discovering an empty token after the
   gate has run. This step exists because the `v1.5.0` run rendered the token empty.
6. Merge `dev-main` into `main`, then tag `vX.Y.Z` on `main`. Pushing the tag runs
   `.github/workflows/release.yml`, which publishes and then verifies the registry
   actually serves the new version before the job goes green.
7. Run `scripts/check-release-state.sh --expect X.Y.Z` yourself, and only then
   describe the version as released in `README.md`, `docs/README.md`,
   `CHANGELOG.md`, `SECURITY.md`, `llms.txt` or this file.

To publish from a workstation instead: `cargo xtask release --live --wait 60`, from an
interactive shell. The crates go out in dependency order:

1. `ruprizzle-core`
2. `ruprizzle-parser`
3. `ruprizzle-dialect`
4. `ruprizzle-macros`
5. `ruprizzle-check`
6. `ruprizzle-lsp`
7. `ruprizzle`
8. `ruprizzle-migrate`
9. `ruprizzle-codegen`
10. `ruprizzle-cli`
11. `ruprizzle-turso`
12. `ruprizzle-d1`

`ruprizzle-parser` is a dev-dependency of `ruprizzle-dialect`, so it must be indexed
first. `cargo xtask release` passes `--no-verify` internally because `cargo publish`
verification resolves `workspace = true` dependencies against the version on
crates.io, which is stale until the previous crate has been indexed; `--wait 60`
pauses between uploads to let that indexing propagate. Live publishes are refused
when `CI` or `GITHUB_ACTIONS` is set, so the workflow is the only automated path.


## 1.5.0 (2026-09-02)

The v1.1–v1.5 feature line, developed on `dev-v2-x` and cut as one minor version:
the workspace never moved off `1.0.0` while five milestones landed, so the intermediate
numbers were never cut.

v1.1–v1.4 brought Postgres array filters, full-text search, soft deletes
(`@deletedAt`), offline query checking (`ruprizzle check`), LSP 2.0, declarative
seeding, implicit many-to-many, nested relational writes, recursive-CTE tree
hierarchies, OpenTelemetry spans and Metrics 2.0, primary/read-replica routing, a TTL
and tag-invalidated query cache, and PostGIS geospatial types. See
[docs/WhatsNewV1_1ToV1_5.md](docs/WhatsNewV1_1ToV1_5.md).

v1.5 brings **Ruprizzle Studio** (`ruprizzle-cli`, behind the non-default `studio`
feature): an embedded Axum + HTMX workbench with a schema dashboard, interactive ERD,
table browser, inline cell editor, foreign-key drawer, SQL sandbox, `EXPLAIN` viewer
and a migration safety diff.

**No `1.5.0` package exists on crates.io.** Two publish runs were attempted and both
failed before uploading anything: the first in `cargo test --workspace` (SQLite temporary
directories dropped before their pools, fixed in `0e4d96b`), the second in
`cargo fmt --all --check`. The second run also rendered `CARGO_REGISTRY_TOKEN` as empty,
so it could not have published even with a green gate. The `v1.5.0` tag is left where it
is, as the record of a source tree whose release failed; the next attempt takes a new
version number. `scripts/check-release-state.sh` reports what the registry actually
serves.

The pre-release assessment and block is the reason the release looks the way it does.
Studio's data plane returned fabricated values without querying the database, its
migration safety diff reported `SAFE` for every model without opening a connection, and
the three edge adapters were in-memory `HashMap`s with a database's name on them. §8
and §10 of
[ProjectPlan/v2/ProductionReadinessV1_5.md](ProjectPlan/v2/ProductionReadinessV1_5.md)
record what each of those became. In short: Studio queries the database on every
screen and says so plainly when it cannot.

The adapters were rewritten rather than renamed. `ruprizzle-turso` speaks Hrana 2 over
HTTP to Turso or any `sqld`; `ruprizzle-d1` speaks the Cloudflare REST API. Both are
tested end to end against a local HTTP server, so neither the test suite nor a
contributor needs a provider account. Neither has ever been published to crates.io.
`ruprizzle-neon` was deleted:
Neon is ordinary Postgres over TLS, so its connection string goes straight to
`ruprizzle::connect` and an adapter crate would wrap nothing.

A live round-trip against a real Turso database and a real D1 database is still
unperformed; the wire format is pinned by the providers' published specifications and
by the local HTTP tests.

Full detail in [CHANGELOG.md](CHANGELOG.md#150---2026-09-02).


## 1.0.1

Documentation-only patch (2026-08-31, tag `v1.0.1`). No public API changes; released
from `main` in parallel with the v1.1-v1.5 line on `dev-v2-x`.

- Refreshed `README.md`, `docs/README.md`, per-crate READMEs, query/relations/
  migrations/operations/examples/quickstart/FAQ, `RELEASES.md`, and `AGENTS.md`
  for the 1.0.0 feature set.
- Updated `docs/BenchmarkResults.md` with the 2026-08-18 cross-ORM run.
- Fixed `examples/blog` dependencies and `main.rs` so it compiles against the
  generated client.


## 1.0.0

The first stable release, published 2026-08-21 from tag `v1.0.0`; all ten publishable
crates are live at that version. No API changes from `1.0.0-rc.1`, which was published
the same day — the surface frozen for the RC is the surface that shipped. From here on
the public API is covered by semantic versioning, as defined in
[docs/Stability.md](docs/Stability.md).

Two gates were waived in writing rather than by omission: the 48-hour `rusqlite` soak,
accepted on 15.56 h / 1.46 B ops / 0 errors, and the two-week RC feedback window, for
want of an external consumer to collect feedback from. The 1.0 line is pinned to
`sqlx 0.8`, which ruprizzle re-exports as part of its own public API.

Full detail in [CHANGELOG.md](CHANGELOG.md#100---2026-08-21).


## 0.1.0-alpha.2

A quick follow-up to `0.1.0-alpha.1` that adds README files and SEO metadata to
every workspace crate, improves the docs site with structured data, sitemap, and
`robots.txt`, and refreshes the `book.toml` homepage URL.

- Added `README.md` to each crate for crates.io.
- Added `homepage`, `documentation`, and `readme` metadata to every
  `Cargo.toml`.
- Added `theme/head.hbs` with schema.org JSON-LD, Open Graph, and Twitter Card
  metadata.
- Added `docs/Faq.md` with an FAQPage schema.
- Added `sitemap.xml` and `robots.txt` and wired them into the Pages workflow.

## 0.1.0-alpha.1

**ruprizzle-orm 0.1.0-alpha** — a schema-first ORM for Rust. Write a Prisma-style
schema, get typed entities, a Drizzle-style query builder that shows you its SQL,
and automatic migrations generated by diffing your schema. Postgres and SQLite.
No query engine binary. Alpha: the API will change, and the limitations are
documented.

### What we claim

- **Automatic migration diffing from a declarative schema** — no other Rust ORM
  has it.
- **`include` with per-relation filters**, in a bounded query count.
- **Column-token typing** that rejects cross-model and wrong-type filters at
  compile time.
- **Identical Rust API across Postgres and SQLite**.
- **`.to_sql()` on every query**.

### What we do not claim

- Production readiness.
- Performance superiority over sqlx.
- Feature parity with Prisma.

These are honest bounds. Overclaiming would be checked within a day and would
 cost more than the attention gained.

### Supported in this release

- Grammar-driven `.ruprizzle` parser with span-preserving diagnostics.
- IR lowering, fingerprinting, and round-trip serialisation.
- Postgres and SQLite dialects with conformance suites.
- Rust entity and query-builder code generation.
- Runtime CRUD, transactions, pagination, and `include` loading.
- 12 migration change classes with `up.sql` / `down.sql` generation.
- CLI: `init`, `generate`, `generate --watch`, `validate`, `format`,
  `migrate dev`, `migrate deploy`, `migrate status`, `migrate resolve`,
  `migrate reset`, `db push`, and `db seed`.
- `trybuild` compile-fail tests that enforce the type-safe query API.
- `cargo xtask` helpers for CI, examples, hardening, and release dry-runs.

### Known limitations

See `docs/KnownLimitations.md` for the full list.

### Performance notes

Measured on the local machine used for development:

| Benchmark | Result |
|---|---|
| Query construction (select by PK, no I/O) | ~600 ns |
| Query construction (filter + order, no I/O) | ~1.8 µs |
| Codegen, 50-model schema | ~16 ms |

I/O benchmarks against Postgres and generated-crate compile-time benchmarks are
not yet part of the automated suite because they require a running database and
a dedicated compile-time machine.
