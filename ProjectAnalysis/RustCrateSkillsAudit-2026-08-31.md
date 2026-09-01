# ruprizzle-orm — Crate Architecture, Perf/Unsafe, and Publishing Audit

**Date:** 2026-08-31
**Commit reviewed:** `19b0fe6` (`main`, clean tree, in sync with `origin/main`)
**Workspace version:** `1.0.1` — published on crates.io
**Scope:** 11 crates, 242 Rust files, ~87.7k LOC
**Rubrics applied:** `rust-crate:crate-architecture`, `rust-crate:perf-and-unsafe`, `rust-crate:publish-open-source`

---

## Verdict

| Rubric | Score | One-line |
|---|---|---|
| Crate architecture | **8.0 / 10** | Textbook `lib.rs`, universal `forbid(unsafe_code)`, disciplined errors — undercut by a very wide `pub mod` surface and wholesale dependency re-exports. |
| Perf & unsafe | **7.0 / 10** | Outstanding unsafe posture; benchmarking infrastructure is real but the release profile is entirely untuned. |
| Publish / open source | **8.5 / 10** | Near-exemplary metadata, MSRV policy, CI matrix and changelog — spoiled by no `exclude` and one live pipeline break. |
| **Overall** | **8.0 / 10** | A genuinely well-engineered 1.0. One P0 regression is currently on `main`. |

This is upper-decile work for a solo-maintained Rust ORM. The engineering practices —
`cargo-semver-checks` in CI, a fuzz workflow, a mutation-testing workflow, an MSRV job that reads
`rust-version` from the manifest, a written stability policy with *documented waivers* — are ahead of
most crates at this size. The findings below are refinements against a high bar, with one exception
that needs fixing today.

---

## P0 — `cargo xtask` is broken on `main` right now

**File:** `.cargo/config.toml` · **Introduced by:** `19b0fe6` · **Severity: Critical**

The most recent commit replaced the file's entire contents. The `[alias]` block was **deleted**, not
appended to:

```diff
-[alias]
-# Makes `cargo xtask ci` work from anywhere in the workspace, so the README's
-# instructions and the repository's automation are the same command.
-xtask = "run --quiet --package xtask --"
+[build]
+target-dir = "G:/cargo-target"
```

`.cargo/config.toml` is tracked (`git ls-files` confirms it; the `.gitignore` rule `/.cargo/*` is
overridden by the file already being in the index). The `xtask` package builds a binary named
`xtask`, **not** `cargo-xtask`, so `cargo xtask …` resolves *only* through that alias. With the alias
gone, every one of these fails with `no such subcommand: xtask`:

| Location | Command | Impact |
|---|---|---|
| `.github/workflows/ci.yml:225` | `cargo xtask examples` | `generated-code` job fails |
| `.github/workflows/ci.yml:297` | `cargo xtask harden` | `harden` job fails on every push to `main` |
| `.github/workflows/release.yml:53` | `cargo xtask release-check --tag …` | **release pipeline fails** |
| `.github/workflows/release.yml:74` | `cargo xtask harden` | release pipeline fails |
| `.github/workflows/release.yml:84` | `cargo xtask release` | **`cargo publish` never runs** |
| `CONTRIBUTING.md:16,41,44,49,54,91` | `cargo xtask ci` | every contributor's first command fails |
| `AGENTS.md:60`, `README.md:591` | `cargo xtask harden` / `ci` | documented workflow is dead |
| `README.md:7` | CI badge links to `cargo xtask ci` | badge describes a command that no longer exists |

**Fix — restore the alias alongside the new build setting:**

```toml
[alias]
# Makes `cargo xtask ci` work from anywhere in the workspace, so the README's
# instructions and the repository's automation are the same command.
xtask = "run --quiet --package xtask --"

[build]
target-dir = "G:/cargo-target"
```

### P0b — the hardcoded `G:/` path should not be in a tracked file

`target-dir = "G:/cargo-target"` is a machine-specific absolute Windows path committed to a public
repository. CI is protected because `CARGO_TARGET_DIR: target` was added to `ci.yml`, `fuzz.yml`,
`mutants.yml`, and `release.yml` — but that override is a workaround for a setting that should never
have been shared in the first place, and it must now be remembered for every future workflow.

Worse, for a contributor cloning on Linux or macOS, `G:/cargo-target` is a *relative* path. Cargo
will create a directory literally named `G:` inside the checkout. Neither `.gitignore` rule
(`/target`, `**/target/`) matches it, so it appears in their `git status` as untracked build output.

**Fix:** move the setting out of the tracked file. Either `~/.cargo/config.toml` on the dev machine,
or a `CARGO_TARGET_DIR` environment variable, or `.cargo/config.local.toml` added to `.gitignore`.
Then drop the four `CARGO_TARGET_DIR` workarounds from the workflows.

---

## 1. Crate architecture — 8.0 / 10

### What is right

**`lib.rs` is a table of contents.** `crates/runtime/src/lib.rs` is 115 lines: crate docs, lint
attributes, module declarations, re-exports, and a `prelude`. No implementation. This is exactly what
the rubric asks for, and it holds across all eleven crates.

**The workspace split is justified, crate by crate.** `macros` must be separate (proc-macro, compiler
requirement). `cli` and `lsp` are binaries whose dependency weight — `clap`, `notify`, `tower-lsp`,
`tokio` — consumers of the runtime should not pay. `core`/`parser`/`dialect`/`codegen`/`migrate` form
the compile-time pipeline that the runtime does not link. `testkit` is `publish = false`. This is not
splitting for tidiness.

**`[workspace.package]` and `[workspace.dependencies]` are used properly** — version, edition, MSRV,
licence, repository, homepage, and authors declared once and inherited with `.workspace = true`.

**Error strategy is correct and deliberate.** `thiserror` throughout, zero `anyhow` in any library
manifest (verified), and `#[non_exhaustive]` on both primary error enums — each with a doc comment
explaining *why* it is non-exhaustive and telling callers to add a `_ =>` arm. `Display` messages are
lowercase, no trailing period, and name the failing input:
`unique constraint violated on {table}.{columns}`. The `Error::kind()` method returning a stable
`&'static str` for telemetry is a genuinely good addition beyond the rubric.

**Feature flags are additive and minimal.** `default = []`. No `no-*` or `disable-*` inversions.
`dep:` syntax used correctly so feature names are not silently dependency names
(`sqlite-rusqlite = ["dep:rusqlite", "rusqlite/bundled"]`). Every feature is documented in
`crates/runtime/README.md` under `## Features`. The feature matrix is tested in CI across seven
combinations crossed with three databases — including a dedicated `metrics` job added specifically
because that feature had no coverage and could rot silently.

### Findings

**A-1 — 26 `pub mod` declarations create a permanent dual public path. (Medium)**

`crates/runtime/src/lib.rs:31-59` declares `aggregate`, `col`, `compile`, `counting`, `error`,
`executor`, `filter`, `include`, `join`, `json`, `m2m`, `metrics`, `model`, `order`, `page`, `pool`,
`query`, `query_manifest`, `rel`, `related`, `tx`, `value`, `types`, `decode`, `prelude`, and two
feature-gated driver modules — all `pub`. Nearly every item in them is *also* re-exported at the
root.

The result is that `ruprizzle::Column` and `ruprizzle::col::Column` are both public, supported, and
covered by semver for the life of 1.x. The rubric's position is direct: *"`pub mod` is a commitment
to that path forever. Use it only when the module is genuinely part of the surface."* Roughly 430
public items across the runtime now have two blessed names each, and the internal module layout can
never be reorganised without a major bump.

`error` and `types` earn their `pub mod` — consumers match on those types. Most of the rest do not.

*Mitigating:* this cannot be fixed within 1.x, and the root re-exports mean idiomatic code already
uses the short path. Record it as a 2.0 item rather than churning now. `docs/Stability.md` should
state explicitly which paths are supported, so the intent survives even if the surface cannot shrink.

**A-2 — `#[non_exhaustive]` missing on two published error enums. (Medium)**

- `crates/dialect/src/lib.rs:183` — `pub enum DialectError`
- `crates/testkit/src/lib.rs:99` — `pub enum TestDbError` (mitigated: `publish = false`)

`DialectError` ships in `ruprizzle-dialect`, which is in the `cargo-semver-checks` package list.
Adding a variant is now a **major** bump — and this is an enum for a trait explicitly designed so
that "more backends are additive" (README). The two enums that *did* get the attribute prove the
policy is understood; these two were missed.

**A-3 — the `source()` chain is dropped on the two wrapped foreign errors. (Medium)**

`crates/runtime/src/error.rs:48-53`:

```rust
#[error("sqlx error: {0}")]
Sqlx(sqlx::Error),

#[cfg(feature = "postgres-tokio-postgres")]
#[error("tokio-postgres error: {0}")]
TokioPostgres(tokio_postgres::Error),
```

Neither field carries `#[source]` or `#[from]`, so `std::error::Error::source()` returns `None` for
both. The `Display` text is interpolated, so the *message* survives — but the structured error is
unreachable through the trait. A caller using `anyhow`'s chain printing, `tracing`'s `error.source`
recording, or a `.downcast_ref::<sqlx::Error>()` walk gets nothing. For a database library, the
underlying driver error is frequently the only actionable information (SQLSTATE, constraint name,
server detail).

**Fix:** `Sqlx(#[source] sqlx::Error)` — a non-breaking change, since the variant shape is unchanged.

**A-4 — three third-party crates are re-exported wholesale into the public API. (Medium — known)**

`crates/runtime/src/lib.rs:105-107`:

```rust
pub use serde;
pub use serde_json;
pub use sqlx;
```

Plus `pub mod types { pub use sqlx::types::*; }` and `Error::Sqlx(sqlx::Error)`.

Under semver this makes **`sqlx 0.8` → `0.9` a breaking change for `ruprizzle`**, requiring
`ruprizzle 2.0`. The same applies to `serde_json`. `sqlx` is pre-1.0 and bumps its minor as its
breaking version, so this is a matter of when, not if — and it will force a major bump driven purely
by a dependency, not by any design decision of yours.

**To the project's credit this is already recognised in writing** — the README status block says
*"the 1.0 line is pinned to `sqlx 0.8`, which ruprizzle re-exports as part of its own public API."*
That is the right way to handle a constraint you cannot remove. It is still the single largest
architectural liability in the crate, and the 2.0 plan should have "shrink the sqlx re-export to the
specific types generated code actually needs" as a headline item.

**A-5 — `#[allow(missing_docs)]` on the public error enum. (Low)**

`crates/runtime/src/error.rs:9` blanket-allows `missing_docs` for `Error`, so eleven of its thirteen
variants ship undocumented. The crate sets `#![warn(missing_docs)]` and CI sets
`RUSTFLAGS: -D warnings`, so the lint is genuinely enforced everywhere *except* here. Two variants
(`PoolExhausted`, and the enum itself) do have good doc comments, which shows the allow is covering
for the rest rather than a deliberate policy. The `#[error(...)]` strings are decent substitutes but
do not appear as item documentation on docs.rs.

**A-6 — the `no_std` stance is never stated. (Low)**

No mention of `no_std` in the README, `docs/`, or any crate root. An ORM built on `sqlx` and `tokio`
obviously cannot support it, but the rubric's point stands: *"Silence reads as 'maybe', and someone
will open an issue after wiring you in."* One sentence in `README.md` under Known limitations.

---

## 2. Perf & unsafe — 7.0 / 10

### What is right

**The unsafe posture is the strongest part of this codebase.** `#![forbid(unsafe_code)]` on **all
eleven crate roots**, plus both binary `main.rs` files and two bench targets. Not `deny` — `forbid`,
which cannot be locally overridden. A full grep of `crates/` finds no `unsafe` block in any library
source; the only hits are the string literal `"unsafe"` in the codegen keyword-escaping table
(`crates/codegen/src/emit.rs:1595`) and the parser's Rust-keyword list
(`crates/parser/src/naming.rs:159`) — both correct uses.

For a database library that hands rows to user code, this is the right trade, and it is the claim
consumers actually look for.

**Benchmarking infrastructure is real, not decorative.** Four `criterion` targets with
`harness = false`, plus a cross-ORM comparison harness under `local/cross-orm-bench/`, a
`bench-client` xtask that regenerates the end-to-end client, and results recorded in
`docs/BenchmarkResults.md` with dates. `crates/runtime/benches/query_construction.rs` is textbook:
`std::hint::black_box` on eight sites, setup outside the timed closure, documented as not touching a
database.

**A fuzz workflow and a mutation-testing workflow**, both on weekly cron with `workflow_dispatch`.
`cargo-mutants` in particular is rare and is the right tool for a query compiler.

**Miri's absence is defensible here** — with `forbid(unsafe_code)` on every crate there is no UB for
it to find in library code. Worth one sentence in `CONTRIBUTING.md` recording that reasoning, so a
future contributor does not read it as an oversight.

### Findings

**P-1 — there is no `[profile.release]` section at all. (Medium)**

The workspace `Cargo.toml` tunes `[profile.dev]` and `[profile.test]` (`debug = 1`, with a comment
explaining why) but defines **no `[profile.release]`**. Every release build, and every `criterion`
run, therefore uses cargo defaults: `lto = false`, `codegen-units = 16`.

Two consequences:

1. The numbers published in `docs/BenchmarkResults.md` and the README's Performance section were
   measured on an untuned profile. For a project whose pitch includes SQL transparency and low
   overhead, and which maintains a cross-ORM comparison, the competitors' defaults may differ.
2. `lto = "thin"` and `codegen-units = 1` are usually a free single-digit-percent win on exactly this
   kind of workload — heavy generic monomorphisation across crate boundaries, which is what a
   generated typed client *is*. Cross-crate inlining is currently limited to 16 codegen units per
   crate with no LTO to recover it.

**Fix — add and then measure, one change at a time:**

```toml
[profile.release]
lto           = "thin"    # measure "fat" too; slower build, sometimes faster code
codegen-units = 1
```

Do **not** set `panic = "abort"` — this is a library others link, and the rubric is explicit that it
breaks `catch_unwind`.

Re-run `cargo bench` before and after and record the delta in `docs/BenchmarkResults.md`. If the win
is under noise, say so in a comment and keep the defaults — a measured "no" is a result.

**P-2 — `crates/codegen/benches/codegen.rs` uses no `black_box`. (Low)**

```rust
b.iter(|| {
    let schema = parse("schema.ruprizzle", &src).unwrap();
    let files = generate_all(&schema);
    assert!(files.len() >= 50, "expected at least one file per model");
})
```

The input `src` is not black-boxed and the result is only consumed by an assertion on `.len()`. The
`assert!` and the cross-crate opaque calls make total elimination unlikely in practice, so this is
not a silently-zero benchmark — but it depends on inlining decisions rather than on a guarantee, and
`sample_size(10)` means a regression would need to be large to be visible.

```rust
b.iter(|| {
    let schema = parse("schema.ruprizzle", black_box(&src)).unwrap();
    black_box(generate_all(&schema))
})
```

*Explicitly not a finding:* `concurrency.rs` and the `end_to_end/*` benches lack `black_box` too, but
they are I/O-bound against a real database. Dead-code elimination is not a plausible failure mode
there and adding it would be cargo-culting.

**P-3 — an unjustified-by-convention `transmute` to `'static` in a shipped example. (Low–Medium)**

`crates/runtime/examples/blocking_floor.rs:123`:

```rust
let stmt = cache.entry(sql.clone()).or_insert_with(|| unsafe {
    // The connection outlives every statement: it is owned
    // by this thread and dropped only when the loop ends,
    // after the cache. The transmute launders the borrow so
    // the cache can be held alongside it.
    std::mem::transmute::<rusqlite::Statement<'_>, rusqlite::Statement<'static>>(
        conn.prepare(&sql).unwrap(),
    )
});
```

The reasoning is stated and, as far as I can trace it, **correct**: `conn` is declared before `cache`
in the same scope, and Rust drops locals in reverse declaration order, so `cache` (and every
`Statement` in it) is destroyed before `conn`. But:

- The comment does **not** use the `// SAFETY:` prefix the rubric requires as a blocking bar. It
  reads as explanation, not as a stated invariant, and grep-based audits of this repo will not find
  it — a search for `// SAFETY` across `crates/` returns only two hits, both in
  `tests/soak_resumable.rs`.
- The invariant it relies on is **drop order of two adjacent local bindings**. Reordering those two
  `let` statements — an edit no reviewer would flag — makes it use-after-free.
- This is a *published example*: `crates/runtime/examples/` ships in the `.crate` (see B-1), so it is
  code strangers will copy. `forbid(unsafe_code)` on the crate root does not apply to example
  targets.
- The rubric's published-crate bar applies: *"If safe code is within about 10% and the crate is
  published, prefer safe."* Here the safe alternative is straightforward — key the cache by SQL and
  call `conn.prepare_cached(&sql)`, which `rusqlite` provides for exactly this purpose and which has
  no lifetime problem at all.

**Fix (preferred):** replace the hand-rolled cache with `Connection::prepare_cached`.
**Fix (minimum):** rewrite the comment to start `// SAFETY:` and state the drop-order invariant as a
requirement, plus a `// NOTE: do not reorder these bindings` on the two `let`s.

**P-4 — `env::set_var` from tests that run in parallel. (Low)**

`crates/runtime/tests/query_manifest.rs:78,98` mutate `RUPRIZZLE_RECORD_QUERIES` inside `unsafe`
blocks with **no `// SAFETY:` comment** (the two sites in `soak_resumable.rs:523,604` do have them —
so the convention exists and was applied inconsistently).

Rust 2024 made `set_var` `unsafe` precisely because it is not thread-safe: `cargo test` runs test
functions on parallel threads in one process, so this races with any concurrent `getenv` anywhere in
the process, including inside C libraries. The soak-test sites are on `#[ignore]`d tests, which is
why their SAFETY comments can honestly claim the variables "are only consumed by the ignored" tests.
`query_manifest.rs` has no such protection — it is an ordinary `#[tokio::test]`.

**Fix:** drive the flag through a process-global that the runtime already owns rather than the
environment, or serialise the affected tests behind a `static Mutex`. At minimum add the `// SAFETY:`
comments so the two files agree.

**P-5 — no profiling artefacts recorded. (Informational)**

`docs/BenchmarkResults.md` records end-to-end timings, and `crates/runtime/examples/` contains real
attribution harnesses (`hotspots.rs`, `layer_attribution.rs`, `bottlenecks.rs`, `perrow.rs`,
`row_buffer.rs`) — which is more than most projects do. What is missing is the layer above:
`debug = true` on a temporary profiling profile plus a `samply` or `cargo flamegraph` run, and a
`dhat-rs` pass for allocation churn. The rubric's ranked list of what actually dominates real Rust
hot spots — allocation churn, stray `.clone()`, `String` where `&str` works, `Vec` without
`with_capacity`, `format!` in hot paths — maps almost exactly onto a SQL string builder. The
harnesses exist; a flamegraph would tell you which of those five is costing you.

---

## 3. Publish / open source — 8.5 / 10

### What is right

**`Cargo.toml` completeness is near-perfect.** All ten published crates carry `description`,
`readme`, `documentation`, `keywords` (≤5, each ≤20 chars), and `categories` drawn from the crates.io
list. `license = "MIT OR Apache-2.0"` with **both** `LICENSE-MIT` and `LICENSE-APACHE` present.
Every crate has `[package.metadata.docs.rs] all-features = true`, added deliberately in the 1.0.0
cut with a comment pointing at the plan item that motivated it.

**The docs.rs failure modes were hit, diagnosed, and fixed properly.** `crates/cli/src/lib.rs` is a
documentation-only library target created because the binary's rustdoc could not be published — and
the file's own doc comment explains the name collision that caused it. The `docs` CI job now runs
`--all-features` with a comment recording that two broken intra-doc links reached a published release
without it. That job sets `RUSTDOCFLAGS: -D warnings`, so a broken link now fails CI rather than
warning. This is the single most common post-publish regret in the rubric, and it is closed.

**MSRV is declared, enforced, and governed.** `rust-version = "1.85"` inherited workspace-wide; a
dedicated `msrv` CI job that *reads the value out of the manifest* rather than hardcoding it; a badge
in the README; and `docs/Stability.md:77-92` gives an actual written policy — minor-release bumps
only, called out under its own changelog heading, roughly once per six months, with the CI job named
as the enforcement mechanism.

**Semver is taken seriously and mechanically enforced.** `cargo-semver-checks` runs in CI against
nine published crates, with a comment explaining why `testkit` and `xtask` are excluded.
`docs/Stability.md` defines the policy the tool enforces.

**`CHANGELOG.md` is Keep a Changelog, correctly.** `[Unreleased]` section present and honest
(`_Nothing yet._`), dated releases, `### Breaking` / `### Added` / `### Fixed` / `### Docs` headings,
and — most importantly — entries written for *consumers*, naming the migration. The 1.0.0 entry
explicitly states there are no API changes from `1.0.0-rc.1` and that everything below is packaging
work. That is the kind of entry that earns trust.

**The contribution surface is complete:** `README.md`, `CONTRIBUTING.md`, `SECURITY.md`,
`CODE_OF_CONDUCT.md`, `.github/pull_request_template.md`, both licence files, an mdBook under
`docs/`, and `RELEASES.md`.

**CI matrix against the rubric's table:**

| Rubric job | Present |
|---|---|
| stable — build, test, doctest | ✅ `test`, matrix over OS |
| beta | ❌ **missing** |
| MSRV pinned toolchain | ✅ `msrv`, reads `rust-version` from manifest |
| `--no-default-features` | ✅ via `feature-combination` (`default = []`, so `""` ≡ none) |
| `--all-features` | ✅ `docs`, `native-driver-workspace` |
| clippy `-D warnings` | ✅ (see B-3 for the `--all-features` gap) |
| `cargo fmt --check` | ✅ |
| `cargo deny check` | ✅ advisories, licenses, bans, sources |
| `cargo doc` with `-D warnings` + all-features | ✅ |
| `cargo semver-checks` | ✅ |
| *(beyond rubric)* integration on PG + MySQL + SQLite | ✅ |
| *(beyond rubric)* fuzz, mutants, generated-code compile | ✅ |

### Findings

**B-1 — no `exclude`; the published `.crate` ships mostly non-library code. (Medium)**

No crate declares `exclude` or `include`. For `ruprizzle` (the runtime, the crate everyone depends
on), the tracked file breakdown is:

| Directory | Tracked files |
|---|---|
| `src/` | 27 |
| `tests/` | **67** |
| `benches/` | **12** |
| `examples/` | **12** |
| `README.md` + `Cargo.toml` | 2 |
| **Total** | **120** |

**93 of 120 files — 78% — are tests, benchmarks, and research harnesses**, roughly 644 KB against
564 KB of actual source. The `examples/` directory in particular holds performance-investigation
harnesses — `cross_orm_bench.rs`, `hotspots.rs`, `layer_attribution.rs`, `bottlenecks.rs`,
`sqlx_floor.rs`, `blocking_floor.rs`, `row_buffer.rs`, `pg_any_types.rs` — that are development
artefacts, not consumer documentation. `blocking_floor.rs` is the file carrying the `transmute` in
P-3, shipped to every consumer.

The rubric is direct: *"`exclude` keeps the package small — check what actually ships with
`cargo package --list`."*

```toml
exclude = ["/tests", "/benches", "/examples", "/wip", "/.github"]
```

Keep one or two genuinely illustrative examples if you want them on docs.rs; exclude the harnesses.
Then run `cargo package --list` and read it — the rubric's step 4, and the last moment you can look.

**B-2 — no `beta` toolchain job. (Low)**

The one row missing from the CI table. A `beta` job catches a regression six weeks before it reaches
users on stable. Cheap to add; copy the `test` job and change the toolchain.

**B-3 — the main `clippy` job is not `--all-features`. (Low)**

`.github/workflows/ci.yml:36` runs `cargo clippy --workspace --all-targets -- -D warnings` — no
`--all-features`. The rubric's row is `clippy --all-targets --all-features -- -D warnings`.

The `feature-combination` job covers the feature-gated code, but scoped `-p ruprizzle` only, so
feature-gated code in `migrate`, `codegen`, or `dialect` is linted by neither job. A one-word change
to line 36 closes it.

**B-4 — no `cargo public-api` diff. (Low)**

`cargo-semver-checks` catches mechanical breakage; `cargo public-api diff --deny=all` is the
complementary check that shows a *human* the full surface delta. Given finding A-1 — ~430 public
items reachable by two paths each — an explicit surface listing reviewed at release time is worth
more here than in a typical crate. It is also the mechanism that would have caught A-1 before 1.0
froze it.

**B-5 — `ruprizzle-testkit` advertises a docs.rs URL it can never have. (Trivial)**

`crates/testkit/Cargo.toml` sets `documentation = "https://docs.rs/ruprizzle-testkit"` alongside
`publish = false`. The link is permanently 404. Remove the `documentation` key.

**B-6 — tool output is committed to the repository. (Trivial)**

`graphify-out/` is tracked (`GRAPH_REPORT.md`, `cost.json`, `graph.html`, `graph.json`,
`manifest.json`), and the working tree carries untracked build debris at the root:
`const_test3.pdb`, `const_test4.pdb`, `const_test5.pdb`, `enc_test3.pdb`, `enc_test4.pdb`,
`__pycache__/`, `mutants.out/`, `mutants.out.old/`, plus `crates/runtime/wip/` holding two orphaned
trybuild `.stderr` files. None of this ships (it is outside any crate directory, and `.gitignore`
covers most of it), so the impact is limited to what a visitor sees on GitHub — but the root
directory is part of the contribution surface. Add `*.pdb` and `/graphify-out/` to `.gitignore` and
`git rm -r --cached graphify-out/`.

**B-7 — maintenance intent is not stated. (Trivial)**

The rubric asks for an honest line about maintenance, because *"abandoned crates with confident
READMEs waste other people's time."* This README is confident and the crate is at 1.0 with a written
stability policy — but it is single-authored, and the status block itself notes that the RC feedback
window was waived *"for want of any external consumer to collect feedback from."* One sentence
setting response-time expectations costs nothing and is the honest complement to that admission.

---

## Prioritised action list

| # | Action | Rubric | Effort |
|---|---|---|---|
| 1 | **Restore `[alias] xtask` in `.cargo/config.toml`** — CI `harden`, `generated-code`, and the entire release pipeline are broken | P0 | 2 min |
| 2 | **Move `target-dir = "G:/cargo-target"` out of the tracked file**, then drop the four `CARGO_TARGET_DIR` workarounds | P0b | 15 min |
| 3 | Add `exclude = ["/tests", "/benches", "/examples", "/wip", "/.github"]`; verify with `cargo package --list` | B-1 | 30 min |
| 4 | Add `#[non_exhaustive]` to `DialectError` (before it needs a major bump) | A-2 | 5 min |
| 5 | Add `#[source]` to `Error::Sqlx` and `Error::TokioPostgres` | A-3 | 5 min |
| 6 | Add `[profile.release]` with `lto = "thin"`, `codegen-units = 1`; re-benchmark and record the delta | P-1 | 1 h |
| 7 | Replace the `transmute` in `blocking_floor.rs` with `prepare_cached`, or give it a real `// SAFETY:` | P-3 | 30 min |
| 8 | `--all-features` on the main clippy job; add a `beta` job | B-3, B-2 | 15 min |
| 9 | `// SAFETY:` on the `set_var` sites in `query_manifest.rs`; serialise those tests | P-4 | 20 min |
| 10 | `black_box` in the codegen benchmark | P-2 | 5 min |
| 11 | Remove `documentation` from `testkit`; untrack `graphify-out/`; ignore `*.pdb` | B-5, B-6 | 10 min |
| 12 | README: `no_std` stance, maintenance intent | A-6, B-7 | 10 min |
| 13 | Add `cargo public-api diff` to the release checklist | B-4 | 30 min |
| 14 | **2.0 backlog:** narrow the `pub mod` surface; shrink the `sqlx` re-export to the types generated code needs | A-1, A-4 | large |

Items 1–2 are today. Items 3–5 and 8–12 are a single afternoon and would move the publishing score to
roughly 9.5. Item 14 is the real architectural work and belongs in `ProjectPlan/v2/V2FeaturesPlan.md`.

---

## Closing note

Nothing in this report suggests the 1.0 release was premature. The error design, the feature-flag
discipline, the universal `forbid(unsafe_code)`, the MSRV governance, and the CI matrix are all at or
above the bar the three rubrics set. The two genuinely structural findings — the width of the `pub`
surface (A-1) and the `sqlx` re-export (A-4) — are both semver-frozen until 2.0, and A-4 is already
documented in the README as a known constraint, which is the correct handling for something you
cannot change.

The urgent item is entirely separate from all of that: a single commit removed a build alias that six
CI steps and every line of the contributor documentation depend on. Fix that first.
