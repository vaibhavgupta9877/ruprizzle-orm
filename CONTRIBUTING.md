# Contributing to ruprizzle

Thanks for considering a contribution. This project is pre-1.0, so the goal is
to keep every change small, justified, and aligned with the design record.

## Getting started

You need Rust **1.85** or later. The workspace uses the 2024 edition, declared
in the root `Cargo.toml`.

```bash
# Clone and enter the workspace
cd ruprizzle-orm

# The easiest way to run the same checks CI runs
cargo xtask ci
```

## Build environment

None of this is required to contribute, but the workspace is large (13 crates,
~490 transitive dependencies) and the defaults leave a lot on the table.

**Put `target/` on a fast local filesystem.** If your checkout lives on an
NTFS, exFAT, or network mount, cargo cannot hard-link artifacts and falls back
to copying every one of them, on top of metadata operations several times
slower than ext4. Point every project at one shared directory on a local disk,
in your own `~/.cargo/config.toml` (not in the repo — the path is
machine-specific):

```toml
[build]
target-dir = "/home/you/.cache/cargo-target"
```

Sharing one directory across projects also means a dependency built with the
same version, features, profile, flags, and toolchain is compiled and stored
once rather than once per checkout. `cargo clean` then wipes artifacts for
every project, so prune with `cargo sweep --time 30` instead.

**Use a parallel linker.** Link time dominates the edit-build-test loop here.
[mold](https://github.com/rui314/mold) needs no root — extract its release
tarball over `~/.local` and add:

```toml
[target.x86_64-unknown-linux-gnu]
rustflags = [
    "-C", "link-arg=-B/home/you/.local/bin",   # so gcc finds ld.mold
    "-C", "link-arg=-fuse-ld=mold",
]
```

`-B` rather than relying on `PATH`, so builds launched from an editor or
desktop session resolve mold too.

**Cap `jobs` if RAM is tight.** Each rustc codegen unit and each link job is a
separate multi-hundred-megabyte process. On a machine with less free RAM than
`nproc` gigabytes, `[build] jobs = <cores minus a few>` finishes faster than
letting all cores swap against each other.

**Run tests with [`cargo nextest`](https://nexte.st).** It runs each test in
its own process with real per-test timeouts, which matters for the
dual-database suites below:

```bash
cargo nextest run --workspace
```

The dev and test profiles already drop debuginfo for dependencies
(`[profile.dev.package."*"] debug = false`) and emit our own crates' debuginfo
unpacked. That is what keeps the shared target directory in the low gigabytes
instead of the low tens, and it shortens every link. Workspace crates keep
`debug = 1`, so stepping through ruprizzle code is unaffected.

## Running tests

Most tests are dual-database: each case runs against SQLite and, when a
Postgres instance is available, against Postgres.

```bash
# SQLite-only, with Postgres tests skipped and a printed notice
cargo test --workspace

# Full matrix; requires the database used by ruprizzle-testkit
docker compose up -d       # or a local Postgres at ruprizzle_test
export RUPRIZZLE_REQUIRE_DB=1
export RUPRIZZLE_TEST_PG_URL=postgres://ruprizzle:ruprizzle@localhost:5432/ruprizzle_test
cargo test --workspace
```

`RUPRIZZLE_REQUIRE_DB=1` is what CI sets. Without it, a missing Postgres is
silently skipped and the suite still reports green, so the skip can hide real
breakage. Set it whenever a database is reachable.

## The `cargo xtask` gates

- `cargo xtask ci` runs `fmt`, `clippy --workspace --all-targets -D warnings`,
  `test --workspace`, and `doc --workspace --no-deps`. This is the gate every
  PR must pass.
- `cargo xtask harden` runs the same checks plus `cargo-deny`, MSRV `cargo check`,
  a dry-run `cargo publish` for every crate, a panic/unwrap audit against the
  checked-in `PANIC_BUDGET`, an arithmetic/indexing audit against the checked-in
  `BUDGETS`, and an audit that no `Value` or user-supplied identifier is
  interpolated into SQL. Use it before a release.
- `cargo xtask examples` compiles generated code for all example schemas under
  both Postgres and SQLite, then asserts the output is `clippy::pedantic`-clean.
  Generated code must stay `clippy::pedantic`-clean; if your change makes the
  generator emit code that trips pedantic lints, fix the generator or the lint
  configuration, not the generated output.
- `cargo xtask bench-client` regenerates the end-to-end benchmark client from
  `crates/runtime/benches/end_to_end/schema.ruprizzle`. Run it after changing
  the benchmark schema or the generator.

## What we enforce

- MSRV is **1.85**.
- Every published crate contains `#![forbid(unsafe_code)]`.
- `cargo clippy --workspace --all-targets -- -D warnings` must be clean.
- New library source (`src/`) must not add `unwrap()`, `expect()`, new panics,
  or new `/`, `%`, and `x[i]` operations on non-constant values on paths
  reachable from user input. The checked-in `BUDGETS` in `xtask` ratchet the
  arithmetic and indexing counts down; if you add a new one, the budget must go
  down elsewhere or the operation must be provably safe. Tests may use them freely.
- Every value that reaches SQL must be a bind parameter. Do not use `format!` to
  interpolate `Value`s, column names, table names, or other user-supplied
  identifiers into SQL strings.

## Adding `trybuild` cases

Schema DSL or query-builder changes that affect compile-time guarantees must
include a `trybuild` case in `crates/runtime/tests/trybuild/`. Each `.rs` file
there is a compile-fail example; the matching `.stderr` file is the expected
error output. Update the `.stderr` when the diagnostic text changes, and add a
new pair for new error classes.

## Where design decisions live

`ProjectPlan/ImplementationPlan/` is the design record. If your change touches
scope, architecture, or an explicit deferral to 0.2, check
`ImplPlan10AppendixDecisions.md` and the relevant phase document first. For
production-readiness work, see `ProjectPlan/ProductionReadinessPlan.md`.

## Branches

`dev-main` is the integration branch. Branch from it, and open your pull request
against it — not against `main`.

```
feature/*  ->  dev-vN-x  ->  dev-main  --(release)-->  main  --> crates.io
```

- **`dev-main`** — where all development lands. Always green: `cargo xtask ci`
  must pass on it.
- **`dev-vN-x`** — a version line (`dev-v2-x` and friends) for a multi-milestone
  feature run. Merges into `dev-main` when the line is done.
- **`main`** — the release line. It receives one merge from `dev-main` per
  release, carrying the version bump, the `CHANGELOG.md` entry and the `vX.Y.Z`
  tag. Crates are published from `main` only.

## Proposing changes

1. Open an issue before large changes so the direction can be agreed.
2. Keep commits focused on one concern.
3. Run `cargo xtask ci` before opening a PR.
4. Mention the design record if the change contradicts or updates an earlier
   decision.
