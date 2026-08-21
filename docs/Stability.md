# API Stability Policy

This document defines what ruprizzle commits to under semantic versioning, from `1.0.0`
onward, and how that commitment is enforced. It exists so that "we follow semver" is a
checkable claim rather than a habit — see W6-02 in `ProjectPlan/v1/PathToStableV1.md`.

Until `1.0.0` ships, the crate is on the `0.x` line and semver's normal relaxation applies:
any `0.x -> 0.(x+1)` bump may contain breaking changes, per Cargo's semver rules. This
document describes the target policy for `1.0.0` and later; see `docs/MigrationGuideToV1.md`
for what actually changed getting there from `0.1.1-beta.1`.

## What is covered by semver

Once `1.0.0` ships, the following are covered — a breaking change to any of them requires a
major version bump:

- **`ruprizzle` (the runtime crate).** Its public items as enumerated by `cargo public-api`:
  the query builder (`SelectQuery`, `InsertQuery`, `UpdateQuery`, `DeleteQuery`, `InsertManyQuery`,
  and their terminal methods), `Pool` and `PoolConfig`, `Transaction`, `Error` and its variants'
  matchable `kind()` strings, `Related<T>`, the `raw!` macro re-export, and the `prelude` module.
- **`ruprizzle-migrate`.** The diff engine's public API (`diff`, change-class types), the
  migration runner (`MigrationRunner`, `MigrationMeta`), and squashing/rename-detection entry
  points that `ruprizzle-cli` and third-party tooling can call directly.
- **`ruprizzle-core`, `ruprizzle-dialect`, `ruprizzle-parser`, `ruprizzle-codegen`.**
  These are published, and their public surfaces are semver-covered like any other published
  crate, but they are lower-level than `ruprizzle` and `ruprizzle-migrate` and are expected to
  see fewer direct downstream consumers. `ruprizzle-core::ir`'s IR types are exposed with wide
  field visibility by design (ADR-007) rather than through an accessor API — this means the IR's
  shape is part of the semver surface, not just its module paths. **Exception:** W2-02's join
  result typing is explicitly called out in `PathToStableV1.md` as provisional and reviewed by
  W6-01; if it needs to change shape before `1.0.0`, that change is not held to the RC feedback
  window described below.
- **Generated client code's public shape.** The entity structs, column tokens, and query-builder
  entry points that `ruprizzle generate` emits from a `.ruprizzle` schema are part of the user
  contract: a schema that generates and compiles today must keep generating and compiling
  after a patch or minor bump. The **internal implementation** of the generated code (exact
  token names for private helpers, module layout inside the generated file, exact `TokenStream`
  shape before `prettyplease` formatting) is not covered — only the parts a user's own code
  references (entity field names/types, relation accessors, query builder methods you call from
  application code).
- **The `ruprizzle-cli` binary's command surface** (subcommands, flags, exit codes) is treated
  as semver-covered even though it has no library API: a script that calls
  `ruprizzle migrate deploy` today should still work after a minor bump.

## What is explicitly NOT covered

These may change in any release, including a patch release, without a major version bump:

- **`ruprizzle-testkit`.** Ships `publish = false` and its own README says "not published."
  It exists to give the workspace's own test suites a dual-database harness and is not
  intended for external use.
- **`xtask`.** Repository automation only (`publish = false`), not a crate anyone depends on.
- **Anything under a crate's `benches/` directory**, including the `end_to_end` benchmark
  harness in `crates/runtime/benches`, and any type that exists solely to support it.
- **Generated code's internal helpers** — private (non-`pub`, or `pub` only within the
  generated crate) items in generated output. See "generated client code's public shape"
  above for the boundary.
- **Anything marked `#[doc(hidden)]`.** Used for items that must be `pub` for a macro or trait
  bound to work but are not part of the intended API surface.
- **Anything under an `unstable` Cargo feature flag**, should one be introduced. No crate
  currently defines one; if a genuinely experimental capability needs to ship ahead of a
  stability commitment, it goes behind `--features unstable`, is documented as such in its
  rustdoc, and is exempt from the semver guarantee and from `cargo-semver-checks` gating
  described below.
- **Error message text.** `Error`'s `Display` output and `miette` diagnostic rendering may be
  reworded at any time; only `Error`'s variant shapes and `kind()` strings are semver-covered,
  because those are what calling code is expected to match on.
- **Dependency versions.** Bumping a dependency (e.g. `sqlx`, `chrono`) is not itself a breaking
  change to ruprizzle's own API, even though it can occasionally change what types are
  re-exported through trait bounds; those re-export changes are evaluated case by case against
  the categories above.

## MSRV policy

Current MSRV: **Rust 1.85** (`rust-version = "1.85"` in the workspace `Cargo.toml`,
inherited by every crate via `rust-version.workspace = true`, and checked in CI's `msrv` job
in `.github/workflows/ci.yml`, which reads the same field and runs `cargo test --workspace`
against it).

Policy from `1.0.0` onward:

- MSRV may be bumped in a **minor** release (e.g. `1.1.0`), not a patch release.
- An MSRV bump is called out explicitly in `CHANGELOG.md` under its own heading, not buried in
  "Changed."
- MSRV is bumped no more than roughly once every six months, and only to pick up a
  toolchain feature actually used by the codebase or a dependency's own MSRV bump that cannot
  reasonably be pinned around.
- The CI `msrv` job is the enforcement mechanism: it fails the build if any crate's code
  requires a newer toolchain than the declared `rust-version`.

## Deprecation process

- An item slated for removal is marked `#[deprecated(since = "X.Y.Z", note = "...")]` with a
  note that says what to use instead (or that there is no replacement and why).
- Deprecated items remain functional — not just present but working — for **at least one
  full minor version** after the deprecation is introduced, and are only removed in the next
  **major** version after that. Concretely: an item deprecated in `1.2.0` stays working through
  at least `1.3.x`, and can only be removed starting at `2.0.0`.
- Every deprecation and every removal of a previously-deprecated item is recorded in
  `CHANGELOG.md`, under "Deprecated" and "Removed" respectively (Keep a Changelog's standard
  headings), with the same before/after treatment `docs/MigrationGuideToV1.md` uses for the
  0.x-to-1.0 transition.
- `cargo-semver-checks` (see below) does not currently model deprecation windows — it flags
  true breaking changes (removed items, changed signatures). It does not stop a deprecated
  item's removal at a major version; that discipline is a matter of the PR review process
  documented in `CONTRIBUTING.md`, not a mechanical gate.

## Enforcement: `cargo-semver-checks` in CI

Mechanical enforcement of everything above lives in the `semver-checks` CI job (see
`.github/workflows/ci.yml`), added under W6-03. It runs `cargo-semver-checks` against every
published, semver-covered crate on every pull request, comparing the working tree's public API
to the latest version published on crates.io. A detected breaking change without a
corresponding major (pre-1.0: minor) version bump fails the job.

`cargo-semver-checks` does not (yet) understand the "generated code" or "CLI command surface"
carve-outs above by construction — it checks Rust-level API surface. Breaking changes confined
to those categories are still expected to be called out by hand in `CHANGELOG.md` and reviewed
in PR, same as any other user-facing change.

## Release candidates (W6-04)

`1.0.0` is not cut directly from a `0.x` release. Before it, the project publishes
`1.0.0-rc.1` (and, if feedback requires changes, `1.0.0-rc.2`, etc.) as an ordinary crates.io
prerelease.

- **What `1.0.0-rc.1` means:** the public API is believed frozen — everything in "What is
  covered by semver" above is intended to be the final `1.0.0` shape — but the release is not
  yet a stability *commitment*. A breaking change discovered during the RC window is still
  fixed by breaking the API again in `1.0.0-rc.2`, not by shipping the mistake into `1.0.0` and
  deprecating it.
- **Feedback window:** at least **two weeks** of real-world use between publishing
  `1.0.0-rc.1` and cutting `1.0.0`, and longer if substantive feedback arrives near the end of
  the window (each round of feedback that produces an API change restarts a shorter, focused
  window rather than the full two weeks). The plan's rationale (`PathToStableV1.md`) is
  explicit that the current download count (43, across four `0.x` prereleases) is not enough
  exposure to freeze an API on without external eyes on the RC specifically.
- **What reviewers should focus on:** exactly the semver-covered surface above — does the
  query builder read naturally from application code, does `Error` matching work the way its
  `kind()` strings imply, does a generated schema from `0.1.1-beta.1` still compile after
  following `docs/MigrationGuideToV1.md`. Feedback on the generated-code internals or `xtask`
  is out of scope for the RC gate, since those are not semver-covered.
- **What this repository does not do during an RC window:** merge new features into the RC
  branch. Only fixes to bugs found during the window and, if needed, API corrections land;
  anything else waits for `1.1.0`.

This section documents the process; it does not itself cut a release. `1.0.0-rc.1` was
published to crates.io on **2026-08-21** from tag `v1.0.0-rc.1`, so the two-week feedback
window described above is running from that date. See the W6-04/W6-05 status note in
`ProjectPlan/v1/PathToStableV1.md`.
