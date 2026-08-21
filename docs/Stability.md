# API Stability Policy

This document defines what ruprizzle commits to under semantic versioning, from `1.0.0`
onward, and how that commitment is enforced. It exists so that "we follow semver" is a
checkable claim rather than a habit — see W6-02 in `ProjectPlan/v1/PathToStableV1.md`.

**`1.0.0` shipped on 2026-08-21, so this policy is now in force** — it is no longer a
description of a target. Everything below applies to the published `1.x` line. The `0.x`
prereleases are superseded; see `docs/MigrationGuideToV1.md` for what changed getting here
from `0.1.1-beta.1`.

Two things about how `1.0.0` was reached are recorded rather than glossed: the release-candidate
feedback window was **waived** (see the waiver at the end of this document), and the 1.0 line is
**pinned to `sqlx 0.8`** because ruprizzle re-exports it (see "Public dependencies").

## What is covered by semver

The following are covered — a breaking change to any of them requires a
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

## Public dependencies

Some of ruprizzle's dependencies are part of its own public API, because their types appear in
signatures users write. `crates/runtime/src/lib.rs` re-exports them so that a caller's version
and ours are guaranteed to be the same crate:

| Crate | Version on the 1.0 line | Where it surfaces |
|---|---|---|
| `sqlx` | `0.8` | `pub use sqlx;`, `Pool`, `Tx`, `Executor`, every `FromRow` impl |
| `serde` | `1` | `pub use serde;`, derives on generated entities |
| `serde_json` | `1` | `pub use serde_json;`, the `Json` column type |
| `rusqlite` | `0.32` | `pub use ::rusqlite::{Row, types}` behind `sqlite-rusqlite` |

**A major bump of any crate in this table is a breaking change to `ruprizzle` and requires a
major version bump of `ruprizzle` itself.** This is not a policy choice — it is a consequence
of the type system. If your application depends on `sqlx 0.9` and `ruprizzle 1.x`, Cargo will
link two incompatible copies of `sqlx` and your `PgPool` will not be our `PgPool`.

Two consequences worth stating plainly, because they will surprise someone:

- **The 1.0 line is pinned to `sqlx 0.8`.** `sqlx 0.9.0` shipped 2026-05-06 and was considered
  for `1.0.0`. It was deferred: it makes all `query*()` functions take `impl SqlSafeStr`
  (133 call sites here build SQL dynamically), makes `SqliteValue` `!Sync` and `SqliteValueRef`
  `!Send` on the decode path, removes lifetimes from `AnyArguments` and the `Arguments` trait,
  changes MySQL text/blob conversion behaviour, and raises the toolchain floor to 1.86. That is
  a `2.0.0`-sized change, and doing it hastily on the way out the door was judged worse than
  doing it deliberately. See `ProjectPlan/v1/V1StableRelease.md` D2 for the full reasoning, and
  `ProjectPlan/v2/V2FeaturesPlan.md` for where it is tracked.
- **Non-public dependencies carry no such promise.** `pest`, `syn`, `quote`, `prettyplease`,
  `miette`, `tracing`, `clap`, `notify`, `criterion`, and `metrics` are implementation details.
  They may be bumped across major versions in a minor release of ruprizzle, because none of
  their types reach a user's code.

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
published to crates.io on **2026-08-21** from tag `v1.0.0-rc.1`. See the W6-04/W6-05 status
note in `ProjectPlan/v1/PathToStableV1.md`.

### Waiver: the `1.0.0-rc.1` feedback window (2026-08-21)

**The two-week window described above was waived for `1.0.0`.** `1.0.0` was cut the same day
`1.0.0-rc.1` was published, rather than on or after 2026-09-04. This is recorded here rather
than left implicit, because a policy the project did not follow is worse than a policy it
amended on purpose. The decision, its rationale, and the alternative considered are written up
as decision **D1** in `ProjectPlan/v1/V1StableRelease.md`; it follows the same pattern as the
W4-02 soak waiver in `docs/SoakReport.md`.

- **Why.** The window's purpose is external eyes on a frozen API. Across seven `0.x`
  prereleases the project has 43 total downloads and no known external consumer, so the two
  weeks would have bought calendar time and no feedback. The related definition-of-done item —
  "at least one external project reporting a successful upgrade" — is waived for the same
  reason: there is no such project to report one.
- **What stands in its place.** The full gate matrix in `V1StableRelease.md` §6, run against
  the tagged commit: `fmt`, `clippy -D warnings` across the feature matrix, the workspace test
  suite against a live PostgreSQL, `cargo doc --all-features` with `-D warnings`,
  `cargo deny check`, `cargo xtask harden`, and `cargo xtask release-check --tag`. Plus
  `cargo-semver-checks`, which continues to compare every published crate's API against the
  last release on crates.io — from `1.0.0` onward that comparison *is* the semver gate.
- **What this waiver does not do.** It does not repeal the policy. A future `2.0.0-rc.1`, or
  any RC published once real downstream users exist, gets the full window. It also does not
  weaken the semver commitment: if an API defect surfaces in the field, the answer is `1.1.0`
  for an addition and `2.0.0` for a break, under the deprecation process above — not a
  retroactive redefinition of what `1.0.0` promised.
