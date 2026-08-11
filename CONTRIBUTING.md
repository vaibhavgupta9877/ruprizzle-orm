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
  checked-in `PANIC_BUDGET`, and an audit that no `Value` or user-supplied
  identifier is interpolated into SQL. Use it before a release.
- `cargo xtask examples` compiles generated code for all example schemas under
  both Postgres and SQLite, then asserts the output is `clippy::pedantic`-clean.
  Generated code must stay `clippy::pedantic`-clean; if your change makes the
  generator emit code that trips pedantic lints, fix the generator or the lint
  configuration, not the generated output.

## What we enforce

- MSRV is **1.85**.
- Every published crate contains `#![forbid(unsafe_code)]`.
- `cargo clippy --workspace --all-targets -- -D warnings` must be clean.
- New library source (`src/`) must not add `unwrap()`, `expect()`, or new panics
  on paths reachable from user input. Tests may use them freely.
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

## Proposing changes

1. Open an issue before large changes so the direction can be agreed.
2. Keep commits focused on one concern.
3. Run `cargo xtask ci` before opening a PR.
4. Mention the design record if the change contradicts or updates an earlier
   decision.
