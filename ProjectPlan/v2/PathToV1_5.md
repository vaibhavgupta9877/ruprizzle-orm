# Path to v1.5 — publish plan and docs cleanup

Assessed 2026-09-13 against `main` at `f3faa0f`.

## 1. Verdict

**Feasible.** The code is in good shape. A version decision and a short list of concrete
fixes stand between the tree and a successful crates.io publish. The documentation site
needs a cleanup before or alongside the release.

## 2. Current state

| Item | State | Evidence |
|---|---|---|
| Registry | `1.0.0-rc.1` is the only published version | crates.io API for `ruprizzle`, `ruprizzle-core`, `ruprizzle-cli` |
| Tags | `v1.0.0`, `v1.0.1`, `v1.5.0` exist; none uploaded anything | `CHANGELOG.md` `[1.5.0]`, `RELEASES.md` |
| Workspace version | `1.5.0` | `cargo xtask release-check --tag v1.5.0` passes |
| Readiness blockers | All §6 items closed; full gate passed 2026-08-31 | `ProductionReadinessV1_5.md` §8–§10 |
| `cargo fmt --all --check` | Fixed (R1) — previously failed in | `crates/testkit/src/lib.rs` (7 hunks), `crates/runtime/tests/insert_validation.rs` |

The fmt gate is the one that failed the previous release run. It may be line endings on
Windows, but CI will fail the same way until it is fixed.

## 3. Decision: version number

`1.5.0` was never uploaded, so crates.io would accept it. But the CHANGELOG records the
`v1.5.0` tag as the immutable record of a failed release, and states that the next
attempt uses a new version.

- **Recommended: `1.5.1`.** No retagging, consistent with existing policy, still "v1.5".
- Alternative: publish as `1.5.0` — requires deleting and recreating the tag and
  rewriting the `[1.5.0]` CHANGELOG section.

The rest of this plan assumes `1.5.1`.

## 4. Release requirements

- [x] **R1 — Fix formatting.** `cargo fmt --all`, commit, confirm CI's fmt job is green.
      *Done 2026-09-13: real rustfmt diffs (not line endings) in the two files; `cargo fmt --all --check` passes locally. CI fmt job to be confirmed on push.*
- [x] **R2 — Bump the version to `1.5.1`** in `workspace.package.version`, all 12
      internal pins in `Cargo.toml`, and the `editor/` VS Code extension. Add a
      `[1.5.1]` CHANGELOG section (fold `[Unreleased]` into it) and a `RELEASES.md`
      entry. Verify with `cargo xtask release-check --tag v1.5.1`.
      *Done 2026-09-13: workspace + 12 pins, `examples/blog` pin and generated
      version constant, VS Code extension, `Cargo.lock`; `[1.5.1] - pending publication`
      CHANGELOG section and RELEASES entry. `release-check --tag v1.5.1` passes.*
- [x] **R3 — Fix the `public-api` CI job.** `.github/workflows/ci.yml` runs
      `cargo public-api diff 1.5.0 --deny=all`, which fetches `1.5.0` from crates.io.
      That version does not exist, so the job fails. Point the baseline at
      `1.0.0-rc.1` for now, or enable the job only after `1.5.1` is published and
      move the baseline to it.
      *Done 2026-09-13: baseline is `1.0.0-rc.1`, report-only (no `--deny`). Verified
      locally that the diff resolves for the crates; `--deny=removed` is not viable
      because `ruprizzle` lists 283 removed items from the intentional
      `#[doc(hidden)]` modules. Follow-up after R9: baseline `1.5.1` + `--deny=all`.*
- [ ] **R4 — Semver check.** ⚠ *Run 2026-09-13 — FAILS; decision needed.* `cargo-semver-checks` compares against the latest
      published version (`1.0.0-rc.1`). The `#[non_exhaustive]` changes in
      `[Unreleased]` are acceptable under a minor bump from an RC; run it locally
      before tagging.

      *Result (`cargo semver-checks`, 1.0.0-rc.1 → 1.5.1, exit 100):* the tool reports
      "semver requires new major version" for 4 of 8 checked crates. `ruprizzle-macros`
      (proc-macro) is not checked.

      | Crate | Result | Breaking items |
      |---|---|---|
      | `ruprizzle-parser`, `-codegen`, `-migrate`, `-lsp` | pass | none |
      | `ruprizzle-core` | 4 major | `ScalarType` +Point/Polygon/MultiPolygon/LineString; `SchemaError` `#[non_exhaustive]` and discriminant shifts; `FieldAttrs` +`is_created_at`/`is_deleted_at`; `IndexDef` +`index_type` |
      | `ruprizzle-dialect` | 2 major | `RustType` +4 PostGIS variants; `DialectError` `#[non_exhaustive]` |
      | `ruprizzle-check` | 3 major | `QueryManifest.version`, `QueryEntry` +`id`/`params`/`result_columns`/`location`; `QueryCheckError` `#[non_exhaustive]` and `UnknownTable` +`suggestion`/`location` |
      | `ruprizzle` | 5 major | `ArrayFilterOp` +Has/IsEmpty/IsNotEmpty (discriminants shift); `FilterNode` +FullTextMatch/Spatial; 43 fns, `CompiledSql`, `CountingExecutor`, `ROW_NUMBER_ALIAS` now `#[doc(hidden)]` |

      The assumption above does not hold: the tool treats rc.1 → 1.5.1 as a minor bump
      and flags these as major. They cannot be made compatible, because adding the
      fields and variants is the feature work, so CI's `semver` job will fail the same
      way. Pick one before R9:
      (a) accept them as RC → stable breakage: document it in the 1.5.1 notes and
      skip or re-baseline the CI job for this release only (e.g. `baseline-rev: v1.5.0`,
      which still flags the `[Unreleased]` `#[non_exhaustive]`/`doc(hidden)` items);
      (b) publish as `2.0.0`. Full log: run the command above.
- [ ] **R5 — Full local gate.** `cargo xtask ci`, `cargo xtask harden`,
      `cargo deny check`, `cargo clippy -p ruprizzle-cli --features studio --all-targets -- -D warnings`,
      `cargo test -p ruprizzle-cli --features studio`, and `cargo xtask release`
      (dry-run `cargo package` for every crate, in order).
- [ ] **R6 — MySQL/MariaDB.** Never verified locally; depends on CI's `integration`
      job. Require it green on the release commit.
- [ ] **R7 — Registry token.** Confirm `CARGO_REGISTRY_TOKEN` is set in Actions
      secrets and visible to `release.yml`. Run `workflow_dispatch` with
      `publish=false` first; the credential preflight fails fast on an empty token.
- [ ] **R8 — Crate names.** `ruprizzle-turso` and `ruprizzle-d1` have never been
      published. Confirm both names are still free on crates.io.
- [ ] **R9 — Tag and publish.** Push `v1.5.1`, let `release.yml` publish, then run
      `scripts/check-release-state.sh --expect 1.5.1`. No file may call the version
      released until that command passes.
- [ ] **R10 — State known gaps in the release notes:** Studio has no authentication;
      the Turso and D1 adapters have not been run against the live services;
      `askama` is still on 0.12.

Not verified during this assessment: recent CI results (GitHub CLI was not
authenticated), clippy, and the full test suite. R5 and R6 cover them.

## 5. Docs cleanup plan

Two kinds of staleness: pages that say v1.1–v1.5 features do not exist, and pages that
contradict each other about which versions were released.

### 5.A Pages that deny shipped features

| Page | Problem | Fix |
|---|---|---|
| `docs/KnownLimitations.md` (last edited 2026-08-21) | "Deferred to v1.2+" lists full-text search, PostGIS, soft deletes, implicit many-to-many and tree helpers — all shipped in v1.1–v1.4. Section is headed "Current beta". | Rewrite for 1.5: remove shipped items; add the real limits (Studio has no auth, adapters untested live, no Neon adapter, Turso has no embedded replicas, D1 has no Worker binding / `wasm32`). |
| `docs/FeaturesMasterComparison.md` (2026-08-21) | "Measured version 1.0.0". Serverless/edge row says "Partial" and ignores Turso/D1. No rows for Studio, read replicas, query cache, OpenTelemetry, PostGIS. | Re-audit every row against `WhatsNewV1_1ToV1_5.md`; add the missing rows. Vector search and multi-tenancy "No" appear correct (not shipped). |
| `docs/DialectNotes.md` (08-18), `docs/MigratingFrom.md` (08-13), `docs/performance.md` (08-19) | Written before v1.1. | Add per-dialect notes for arrays, FTS, PostGIS, soft deletes, replicas; map Prisma/Drizzle users to the new features. |
| `docs/Operations.md` | Snippet pins `ruprizzle = "1.0.0"`. | Use the release version; add replica routing, query cache, OpenTelemetry. |
| `docs/BenchmarkResults.md`, `docs/SoakReport.md` | Measured on ruprizzle `1.0.0`; the soak was stopped after ~47 minutes. | Label clearly as 1.0 measurements, or re-run on 1.5.1. |

### 5.B Pages that contradict each other about releases

- `docs/Stability.md` says "`1.0.0` shipped on 2026-08-21" and
  `docs/MigrationGuideToV1.md` says "`1.0.0`, published 2026-08-21". The FAQ,
  `announcement.md` and `RELEASES.md` say it was never published. Correct both.
- `README.md`, `docs/README.md`, `docs/quickstart.md`, `docs/faq.md`,
  `docs/Examples.md` and `llms.txt` direct users to install `1.0.0-rc.1`. Accurate
  today; must change the moment `1.5.1` is published.

### 5.C Sequence

- [ ] **D1 — Before publishing:** fix 5.A and the two false claims in 5.B. None depends
      on the version number.
- [ ] **D2 — Immediately after `check-release-state.sh --expect 1.5.1` passes:** update
      every install command and "only version on crates.io" banner to `1.5.1` in one
      commit. Keep `announcement.md` archived and write a 1.5 announcement.
- [ ] **D3 — Prevent recurrence:** add a docs CI step that fails if any `docs/` page
      names an installable version other than the one the registry serves (excluding
      ADRs and archived pages), reusing `scripts/check-release-state.sh`.
- [ ] **D4 — Site rebuild:** `pages.yml` deploys from `main` on merge. Spot-check the
      live `KnownLimitations.html`, which currently shows the stale text.
