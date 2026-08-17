# Public API Review (W6-01)

**Date:** 2026-08-17
**Tool:** [`cargo-public-api`](https://github.com/cargo-public-api/cargo-public-api) v0.52.0, run with `+nightly` (required for `rustdoc --output-format=json`).
**Scope:** every workspace member under `crates/*` (see root `Cargo.toml` `[workspace] members`).

## Method

For every library crate in the workspace:

```
cargo +nightly public-api -p <crate> --simplified
```

`crates/cli` and `xtask` were excluded from the tool run because they are binary-only
(`[[bin]]` targets, no `src/lib.rs`) — their surface is the process's CLI argument parsing,
not a Rust API, and `xtask` additionally carries `publish = false`.

## Crates reviewed

| Crate | Public items (approx.) | Published to crates.io | Notes |
|---|---:|:---:|---|
| `ruprizzle-core` | ~700 (IR types, names, span, diagnostics) | yes | Foundation IR shared by parser/dialect/codegen/migrate. Large but intentional — this is the schema IR every other crate consumes. |
| `ruprizzle-dialect` | ~190 | yes | Postgres/SQLite dialect traits and SQL rendering. |
| `ruprizzle-parser` | ~150 | yes | `.ruprizzle` grammar, lowering, validation. |
| `ruprizzle-codegen` | 5 | yes | Deliberately tiny: `generate_all` plus an `emit` module re-export. |
| `ruprizzle-migrate` | ~280 | yes | Diff engine, migration runner, squashing, rename detection. |
| `ruprizzle-macros` | 2 | yes | `raw!` proc macro only. |
| `ruprizzle` (runtime) | ~2200 (after macro expansion of derives) | yes | The main runtime crate: query builder, pool, transactions, relations, types. Largest surface by a wide margin, as expected for the crate applications depend on directly. |
| `ruprizzle-testkit` | ~40 | **no** (`publish = false`) | Dual-database test harness. Not a semver-covered crate; see `docs/Stability.md`. |
| `ruprizzle-cli` | n/a (binary) | yes (binary only) | No library surface to review. |
| `xtask` | n/a (binary, `publish = false`) | no | Repository automation only. |

Raw `cargo public-api` output for each crate is not checked in (it is generated, and would
immediately go stale); reproduce it with the command above.

## Findings

1. **No accidental internal leakage found.** A targeted search of every crate's surface for
   `bench`, `internal`, `debug_`, `test_util`, `__`-prefixed identifiers, and similar markers
   turned up only expected serde/hash/clone/debug trait-impl methods (`__D`, `__S`, `__H`
   are `serde`'s and `core::hash`'s own generic parameter names, not project internals) and
   `ruprizzle_testkit::Backend`/`TestDb`, which is already excluded from publishing.
2. **`ruprizzle-testkit` is already correctly scoped.** It ships `publish = false` in its
   `Cargo.toml` and its own README says "not published." No code change needed; `docs/Stability.md`
   makes this exclusion explicit so it is a documented policy rather than an implicit one.
3. **`xtask` and `crates/cli`'s binary have no library surface** to review — their public
   contract is their CLI argument shape, not a Rust API, and is out of scope for
   `cargo-public-api`. `docs/Stability.md` covers CLI compatibility separately.
4. **`ruprizzle-codegen`'s public surface is minimal and intentional** — a single
   `generate_all(&Schema) -> BTreeMap<String, String>` entry point plus an `emit` module that
   re-exports the same function and a `format` helper. This is generator-facing infrastructure
   consumed by `ruprizzle-cli`; there is nothing here that looks like it should be hidden
   further without breaking the CLI's own use of it.
5. **`ruprizzle-core::ir`'s IR types are broadly `pub`,** including fields on structs like
   `Schema`, `Model`, `Field`, `RelationRef`. This is a deliberate design choice (ADR-007,
   "snapshot-serialised IR") to let `ruprizzle-migrate`, `ruprizzle-codegen`, and downstream
   tooling consume the IR directly rather than through an accessor API. It is called out
   explicitly in `docs/Stability.md` as a wider-than-usual semver commitment, and W2-02's join
   result typing is flagged there as still provisional per the `PathToStableV1.md` sequencing
   note.
6. **No `#[doc(hidden)]` additions were made.** Every `pub` item found under `crates/*/src`
   corresponds to something a user of the crate (application code, or in `ruprizzle-migrate`'s
   case, `ruprizzle-cli`) can legitimately reach and is expected to use. Per the task brief's
   guidance to prefer documentation over invasive changes when the finding is not clearly
   low-risk, no crate's public surface was altered by this review. Any future finding of a
   clearly-internal `pub` item should be hidden as a normal PR, gated behind this document
   being updated, not folded into unrelated feature work.

## Outcome

No code changes were required. This document is the review's record; re-run the
`cargo public-api` command above before cutting `1.0.0-rc.1` (W6-04) to catch anything added
between now and the RC, and diff the RC's surface against `1.0.0` before final release using
`cargo public-api diff` (see `docs/Stability.md` for how this feeds `cargo-semver-checks` in CI).
