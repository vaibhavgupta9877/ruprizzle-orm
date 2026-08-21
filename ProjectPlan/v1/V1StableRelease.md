# V1 Stable Release — `1.0.0-rc.1` → `1.0.0`

> **Status:** S1-S3 COMPLETE (2026-08-21) — the workspace is at `1.0.0` and every
> local gate is green. S4 (push, tag, publish, docs.rs re-verify, rescore) is maintainer
> action and remains open. Created 2026-08-21. This plan covers the *last mile only*: the
> engineering work that must land between the published `1.0.0-rc.1` and a stable `1.0.0`,
> plus the two decisions (RC feedback window, public dependency freeze) that cannot be
> resolved mechanically.

**From:** `1.0.0-rc.1` — published to crates.io 2026-08-21 from tag `v1.0.0-rc.1`, all ten
publishable crates live, none yanked.
**To:** `1.0.0` — a stable release under the semver commitment in `docs/Stability.md`.

**Created:** 2026-08-21
**Owner:** Vaibhav Gupta
**Scope relationship:** narrows `PathToStableV1.md` to its open tail (W6-05 and the release
cut). W0–W5 and W6-01…W6-04/W6-06 are complete and are not re-opened here. This plan does
**not** supersede `PathToStableV1.md`; it is the executable remainder of it, plus a docs
defect class that plan never covered.

---

## 1. Why this plan exists

`PathToStableV1.md` says the only thing left before `1.0.0` is calendar time plus a rescore.
That is not quite true. A fresh analysis of the published RC found three things that are
engineering work, not calendar time:

1. **`ruprizzle-cli` has no documentation on docs.rs at all.**
   `https://docs.rs/crate/ruprizzle-cli/1.0.0-rc.1/status.json` returns
   `{"doc_status":false}`, while all nine other crates return `true`. This is not a queue
   delay — the crate is not in the docs.rs build queue and its build page lists no attempt.
2. **`cargo doc --all-features` fails on `ruprizzle`.** Two broken intra-doc links live in
   code that only compiles under optional features, so neither CI nor docs.rs has ever
   exercised them.
3. **docs.rs only documents each crate's default feature set.** `ruprizzle`'s
   `sqlite-rusqlite`, `postgres-tokio-postgres`, and `metrics` modules are therefore absent
   from the published API reference, with no feature badges anywhere.

Freezing an API under semver while its published reference is incomplete for one crate and
absent for another is not a 1.0.

---

## 2. Verified baseline (2026-08-21, `dev-v0-2` @ `d150ead`)

Everything below was re-run for this plan, not carried over from an earlier section.

| Gate | Command | Result |
|---|---|---|
| Format | `cargo fmt --all --check` | PASS (exit 0) |
| Lint | `cargo clippy --workspace --all-targets -- -D warnings` | PASS (exit 0) |
| Docs (default features) | `RUSTDOCFLAGS=-D warnings cargo doc --workspace --no-deps` | PASS (exit 0) |
| **Docs (all features)** | `RUSTDOCFLAGS=-D warnings cargo doc --workspace --no-deps --all-features` | **FAIL — 2 errors** |
| **docs.rs, `ruprizzle-cli`** | `curl docs.rs/crate/ruprizzle-cli/1.0.0-rc.1/status.json` | **FAIL — `doc_status: false`** |
| docs.rs, other 9 crates | same, per crate | PASS — `doc_status: true` |
| Branch / tag sync | `git status -sb`, `git ls-remote --tags` | 1 commit ahead of `origin/dev-v0-2`; `v1.0.0-rc.1` is on the remote |

The two docs failures in full:

```
error: unresolved link to `rusqlite::RusqlitePool`
   --> crates/runtime/src/executor.rs:218:34

error: `bytes` is both a function and a crate
   --> crates/runtime/src/decode.rs:487:26
```

`ProductionReadiness.md` §17 (89 / 100) remains the current score. §17's claim that "27 local
commits" are unpushed is **stale** — the branch is one commit ahead, and the RC tag is on the
remote.

---

## 3. The two decisions

Neither can be settled by running a command. Both are recorded here so the outcome is a
decision rather than a drift.

### D1 — The two-week RC feedback window

`docs/Stability.md` commits, in writing, to "at least **two weeks** of real-world use between
publishing `1.0.0-rc.1` and cutting `1.0.0`", and `PathToStableV1.md`'s definition of done
requires "at least one external project reporting a successful upgrade". `1.0.0-rc.1` was
published **2026-08-21** — the same day this plan is written. Cutting `1.0.0` now shortens
that window to zero.

**Options:**

- **D1-a — Wait.** Cut `1.0.0` on or after 2026-09-04. Honours the written policy. Costs two
  weeks of calendar time and depends on external adoption that 43 lifetime downloads make
  unlikely to materialise on its own.
- **D1-b — Waive, in writing.** Cut `1.0.0` now, and amend `docs/Stability.md` to record the
  waiver, its rationale, and what replaces it — exactly the pattern already used and accepted
  for the W4-02 48-hour soak in `docs/SoakReport.md`. The substitute assurance is the gate
  matrix in §6 below plus `cargo-semver-checks` continuing to compare `1.0.0` against the
  published RC.

**Chosen: D1-b.** The point of an RC window is external eyes; with no external consumers to
provide them, waiting two weeks buys calendar time and no information. What it must not do is
leave the repository asserting a policy it did not follow — so the waiver is written into
`docs/Stability.md`, not left implicit. If the API does prove wrong in the field, semver's
answer is `1.1.0` for additions and `2.0.0` for breaks, and `docs/Stability.md` already
documents the deprecation process for both.

### D2 — Public dependencies and the semver freeze

`crates/runtime/src/lib.rs` re-exports `sqlx`, `serde`, and `serde_json` (`pub use sqlx;` at
line 110), and `crates/runtime/src/rusqlite.rs` re-exports `rusqlite::Row` and
`rusqlite::types`. These are **public dependencies**: a major bump of any of them is a
breaking change to `ruprizzle`'s own API, because a caller's `sqlx::PgPool` and ours must be
the same type.

`sqlx 0.9.0` shipped 2026-05-06. Taking it before `1.0.0` would be the semver-cheap moment.
It is also a large, genuinely breaking migration:

- all `query*()` functions now take `impl SqlSafeStr`; ruprizzle builds SQL dynamically, so
  every dynamic call site needs `AssertSqlSafe(...)` — **133 call sites across ~25 files**;
- `SqliteValue` becomes `!Sync` and `SqliteValueRef` `!Send`, which touches the decode path;
- lifetimes removed from `AnyArguments` and from the `Arguments` trait;
- MySQL text/blob → `AnyTypeInfo` conversion changed, i.e. behavioural, not just structural;
- sqlx's MSRV rises to 1.86, forcing our `rust-version` up from 1.85.

**Chosen: defer sqlx 0.9 past 1.0.0.** This is days of work with real behavioural risk in the
`Any` and SQLite paths, against a directive to ship quickly, and sqlx 0.8.6 is current and
maintained. What ships instead is the *honest statement of the consequence*:
`docs/Stability.md` gains a "Public dependencies" section naming `sqlx`, `serde`,
`serde_json`, and `rusqlite`, and stating that a major bump of any of them requires a major
bump of `ruprizzle`. That converts a hidden trap into a documented, dated commitment.

> **Note for the reader deciding otherwise:** if the sqlx 0.9 migration is done *before* the
> tag, it costs one minor version and no user disruption. Done after, it costs `2.0.0`. That
> asymmetry is the whole argument for D2, and it is being knowingly traded for schedule.
> Tracked as a `2.0.0` candidate in `ProjectPlan/v2/V2FeaturesPlan.md`.

---

## 4. Workstreams

### S1 — Documentation defects *(the blocking work)*

- [x] **S1-01 · Fix the two broken intra-doc links.**
      `crates/runtime/src/executor.rs:218` — ``[`rusqlite::RusqlitePool`]`` resolves against
      the `rusqlite` *crate*, which has no such item; the type is ours, at
      `crate::rusqlite::RusqlitePool`. `crates/runtime/src/decode.rs:487` — ``[`bytes`]`` is
      ambiguous between our `bytes()` function and the `bytes` crate pulled in by
      `postgres-tokio-postgres`; disambiguate to ``[`bytes()`]``.
      **Exit:** `RUSTDOCFLAGS=-D warnings cargo doc --workspace --no-deps --all-features`
      exits 0.

- [x] **S1-02 · Make `ruprizzle-cli` documentable.**
      Root cause: `crates/cli/Cargo.toml` declares a single `[[bin]]` target carrying
      `doc = false`, and the crate has no `[lib]`. rustdoc is therefore asked to document
      nothing, which docs.rs records as a failed build. Drop `doc = false` so the binary
      crate's own rustdoc — including the `//!` module docs already at the top of
      `src/main.rs` and the `clap` command surface — is published.
      **Exit:** `cargo doc -p ruprizzle-cli --no-deps` emits `target/doc/ruprizzle_cli/`.

- [x] **S1-03 · docs.rs metadata on every publishable crate.**
      No crate currently has a `[package.metadata.docs.rs]` table, so docs.rs builds default
      features only. Add to each publishable manifest:
      ```toml
      [package.metadata.docs.rs]
      all-features = true
      rustdoc-args = ["--cfg", "docsrs"]
      ```
      and add `#![cfg_attr(docsrs, feature(doc_cfg))]` to the crates that gate items behind
      features, so the published reference carries feature badges instead of silently
      omitting `sqlite-rusqlite`, `postgres-tokio-postgres`, and `metrics`. This is only safe
      **after** S1-01, since `all-features = true` is exactly the configuration that currently
      fails.
      **Exit:** every publishable manifest carries the table; the local `--all-features` doc
      build is green.

- [x] **S1-04 · Close the CI gap that let this through.**
      `.github/workflows/ci.yml`'s `docs` job runs `cargo doc --workspace --no-deps` with no
      `--all-features`, which is precisely why S1-01's two errors survived into a published
      release. Add `--all-features`. While in the file, add `ruprizzle-check` and
      `ruprizzle-lsp` to the `semver-checks` job's package list — they are published and
      semver-covered, and were omitted for the same reason `cargo xtask release` omitted them
      before V1-05.
      **Exit:** the CI docs job fails on a broken link in feature-gated code.

**S1 gate:** `doc_status: true` for all ten publishable crates at `1.0.0`, verified against
docs.rs after publish.

### S2 — Dependency currency

- [x] **S2-01 · `cargo update`.** Refresh the lockfile to the latest semver-compatible
      versions across the whole tree, then re-run the full gate matrix.
- [x] **S2-02 · Internal-only major bumps.** Bump only dependencies that are *not* part of the
      public API, where the API delta is small enough to verify in one compile:
      `criterion 0.5 → 0.8` (dev-only, benches), `notify 7 → 8` (CLI file watch only),
      `metrics 0.23 → 0.24` (behind the optional `metrics` feature). Anything that fails to
      compile in one pass is dropped from this plan rather than fought.
- [x] **S2-03 · Deliberately not bumped.** Recorded so each is a decision:
      `sqlx 0.8` (D2 — public dep, 133 call sites, MSRV +1);
      `rusqlite 0.32 → 0.40` (public re-export under `sqlite-rusqlite`; the same D2 argument at
      smaller scale, and it is the soak-tested path);
      `syn 2 → 3` and `prettyplease 0.2 → 0.3` (proc-macro / codegen core; a mid-release-cut
      rewrite of the generator is a poor trade);
      `sha2 0.10 → 0.11` (`digest 0.11` API change on the *migration checksum* path — the one
      place where a silent behavioural difference would corrupt user state);
      `tower-lsp 0.20` (already the latest release).

**S2 gate:** `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`,
`cargo test --workspace`, and `cargo deny check advisories` all green on the updated lockfile.

### S3 — The version cut

- [x] **S3-01 · Record the D1 waiver.** Amend `docs/Stability.md`'s "Release candidates"
      section: the two-week window was waived on 2026-08-21, by whom, why, and what stands in
      its place. Do not delete the policy — a future RC still gets a window.
- [x] **S3-02 · Add the public-dependency section (D2).** A new section in `docs/Stability.md`
      naming `sqlx`, `serde`, `serde_json`, and `rusqlite` as public deps of the 1.0 line, with
      the consequence stated plainly.
- [x] **S3-03 · Bump the workspace to `1.0.0`.** `[workspace.package] version` plus the ten
      internal `version = "…"` pins in the root `Cargo.toml`, and `examples/blog/Cargo.toml`
      (a standalone crate, so `cargo` will not fix it for us).
- [x] **S3-04 · `CHANGELOG.md` `[1.0.0]` section.** Everything since `1.0.0-rc.1`, plus the
      compare links at the foot of the file.
- [x] **S3-05 · Documentation sweep `1.0.0-rc.1` → `1.0.0`.** The version string appears in
      `README.md` (status banner, crate table, roadmap), `docs/README.md`,
      `docs/quickstart.md`, `docs/Examples.md`, `docs/Operations.md`, `docs/faq.md` (including
      its embedded JSON-LD answer), `docs/announcement.md`, `docs/SUMMARY.md`,
      `docs/FeaturesMasterComparison.md`, `docs/BenchmarkResults.md`, and
      `docs/MigrationGuideToV1.md`. Each needs reading, not `sed` — several sentences describe
      the RC *as a release candidate* and become false, not merely stale, under a version
      substitution.
- [x] **S3-06 · `cargo xtask release-check --tag v1.0.0`.** The V1-05 guard that tag,
      workspace version, and changelog heading agree.

**S3 gate:** `release-check` passes; no `1.0.0-rc.1` remains in `README.md`, `docs/`, or any
manifest except historical `CHANGELOG.md` entries and compare links.

### S4 — Publish *(maintainer action, not automated here)*

Left explicitly to the maintainer: these are irreversible and outward-facing, and one of them
spends a crates.io token.

- [ ] **S4-01 · Push `dev-v0-2`, open / merge to `main`.**
- [ ] **S4-02 · Tag `v1.0.0` and push.** `release.yml` triggers on both tag shapes since V1-05.
- [ ] **S4-03 · Confirm the release workflow published all ten crates.**
- [ ] **S4-04 · Re-verify docs.rs.** For each of the ten crates,
      `curl -s https://docs.rs/crate/<crate>/1.0.0/status.json` must return
      `"doc_status":true`. This is the real exit gate for S1 — the local build is a proxy.
- [ ] **S4-05 · W6-05 rescore** against the published `1.0.0` in `ProductionReadiness.md` §18,
      targeting ≥ 92 / 100, and close W6-05 and the `PathToStableV1.md` definition of done.

---

## 5. Sequencing

```
S1-01 ──> S1-03 ──┐
S1-02 ────────────┼──> S2-01 ──> S2-02 ──> S3 ──> S4
S1-04 ────────────┘
```

S1-01 strictly precedes S1-03: turning on `all-features = true` for docs.rs while
`--all-features` fails would convert nine passing docs builds into nine failing ones. S2
precedes S3 so the lockfile in the tagged commit is the one the gates ran against. S4 is
maintainer-gated throughout.

---

## 6. Definition of done

`1.0.0` ships when all of these hold. The two that D1 modifies are struck through.

- [x] `cargo fmt --all --check` — exit 0.
- [x] `cargo clippy --workspace --all-targets -- -D warnings` — exit 0.
- [x] `cargo clippy` green for `sqlite-rusqlite` and `postgres-tokio-postgres` (covered by the `--all-features` run).
- [x] `cargo test --workspace` — green against a live PostgreSQL: 93 binaries, 476 passed, 0 failed, 5 ignored (§6a).
- [x] `RUSTDOCFLAGS=-D warnings cargo doc --workspace --no-deps --all-features` — exit 0.
- [x] `cargo deny check advisories licenses bans sources` — green.
- [x] `cargo xtask harden` — exit 0.
- [x] `cargo xtask release-check --tag v1.0.0` — exit 0.
- [ ] All ten publishable crates report `doc_status: true` on docs.rs at `1.0.0`.
- [x] `docs/Stability.md` records the D1 waiver and the D2 public-dependency list.
- [ ] Production readiness rescored against published `1.0.0` at ≥ 92 / 100. *(W6-05)*
- [ ] ~~Two-week RC feedback window~~ — **waived under D1**, waiver recorded in
      `docs/Stability.md`.
- [ ] ~~At least one external project reporting a successful upgrade~~ — **waived under D1**;
      no external consumer exists to report one.

---

## 6a. Execution log — 2026-08-21

S1 through S3 were executed on `dev-v0-2`. What follows is what was actually run and what it
returned, so a reader does not have to take §6's checkboxes on trust.

| Gate | Command | Result |
|---|---|---|
| Format | `cargo fmt --all --check` | exit 0 |
| Lint (all features) | `cargo clippy --workspace --all-targets --all-features -- -D warnings` | exit 0 |
| Docs (all features) | `RUSTDOCFLAGS=-D warnings cargo doc --workspace --no-deps --all-features` | exit 0; `target/doc/ruprizzle_cli/` now produced |
| Tests | `RUPRIZZLE_TEST_PG_URL=… cargo test --workspace --no-fail-fast` | exit 0 — **93 binaries, 476 passed, 0 failed, 5 ignored** |
| Supply chain | `cargo deny check advisories licenses bans sources` | all four ok |
| Hardening | `cargo xtask harden` | exit 0 — every crate at or under its panic / indexing budget |
| Release guard | `cargo xtask release-check --tag v1.0.0` | ok — tag, workspace version, and CHANGELOG agree on `1.0.0` |

MySQL-backed tests are skipped rather than failed, because this machine has no MySQL server and
`RUPRIZZLE_REQUIRE_DB` was not set. PostgreSQL 17.10 was live for the run, so the DB-backed
paths — including the per-schema isolation added in `ProductionReadiness.md` §17 — were
exercised for real.

### Two things worth recording

**The first full run failed one test, and the failure was correct.** The 15
`ruprizzle-codegen` snapshots embed `pub const RUPRIZZLE_VERSION`, so bumping the workspace
version is *supposed* to move them. They were updated by hand rather than through
`cargo insta review` (which is interactive); the diff in each is exactly the one line, and the
generated code is otherwise byte-identical to what `1.0.0-rc.1` emitted. That identity is the
evidence behind this release's "no API changes since the RC" claim.

**S2-02's three major bumps compiled in one pass, with one deprecation to absorb.**
`criterion 0.6` deprecated `criterion::black_box` in favour of `std::hint::black_box`; under
`-D warnings` that is a failure, not a warning, so `crates/runtime/benches/query_construction.rs`
was switched over. `notify 8` and `metrics 0.24` needed no source changes at all.

### Not done, and why

- **`RELEASES.md`** was last updated at `0.1.0-alpha.2` and has not tracked releases since. It
  already points at `CHANGELOG.md` in its first line. Back-filling six releases of prose was
  out of scope for this cut; either retire the file or restart it deliberately.
- **`ProductionReadiness.md` §18** — the W6-05 rescore is S4-05 and must run against the
  *published* `1.0.0`, not this working tree. The current score stands at 89 / 100 (§17).
- **docs.rs verification** is S4-04 and is only possible after publish. Everything S1 did is
  verified locally; `doc_status: true` for all ten crates is the real gate.

---

## 7. Risks

| Risk | Likelihood | Mitigation |
|---|---|---|
| The API is wrong in a way the waived RC window would have caught | Medium | Accepted under D1. `cargo-semver-checks` still compares `1.0.0` to the published RC; corrections ship as `1.1.0` / `2.0.0` under the documented deprecation process. |
| `all-features = true` on docs.rs surfaces a *new* rustdoc failure on the docs.rs nightly that the pinned 1.95 toolchain does not reproduce | Medium | S4-04 checks `status.json` per crate after publish; a docs.rs failure is fixable by a patch release without yanking. |
| Being pinned to `sqlx 0.8` for the life of the 1.0 line becomes a real user complaint | Medium | Named in D2 and written into `docs/Stability.md`, so it is a disclosed constraint rather than a surprise. sqlx 0.9 is a `2.0.0` candidate. |
| `cargo update` pulls a regression the suite does not catch | Low | The full gate matrix is re-run on the updated lockfile before the tag; the lockfile is committed, so a bisect is available. |
| `ruprizzle-cli` docs stay empty because rustdoc for a bin-only crate documents little of value | Low | Its `//!` header and `clap` surface are real content; if docs.rs still reports `doc_status: false` after S1-02, the fallback is a thin `src/lib.rs` re-exporting the command types. |

---

*Created 2026-08-21. Narrows `PathToStableV1.md` to its open tail; supersedes nothing.*
