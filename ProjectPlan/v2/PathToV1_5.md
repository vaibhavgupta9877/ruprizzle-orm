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
- [x] **R4 — Semver check.** *Resolved 2026-09-13 with option (a).* `cargo-semver-checks` compares against the latest
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

      *Resolution — option (a), accepted as RC → stable changes, with a smooth-upgrade path:*
      - `docs/UpgradingFromRc1.md`: an "am I affected" table, the compiler error and a
        before/after fix for every item above, an error lookup table, and rollback steps.
        Linked from README, docs/README, SUMMARY, CHANGELOG `[1.5.1]`, RELEASES; the
        exception is written into `docs/Stability.md`.
      - Verified compatible rather than assumed: clients generated by rc.1 for five
        schemas build unchanged against 1.5.1; hidden-module signatures are unchanged.
      - Found and fixed a break the semver tools miss: rc.1 migration snapshots failed
        with ``missing field `is_created_at` ``. `#[serde(default)]` was added, with a
        regression test (`95fa5b4`).
      - CI `semver` job uses `release-type: major` for this release (passes locally, exit
        0). **Follow-up after R9:** remove `release-type`, and move `public-api` to
        `1.5.1 --deny=all` (see R3).
- [x] **R5 — Full local gate.** `cargo xtask ci`, `cargo xtask harden`,
      `cargo deny check`, `cargo clippy -p ruprizzle-cli --features studio --all-targets -- -D warnings`,
      `cargo test -p ruprizzle-cli --features studio`, and `cargo xtask release`
      (dry-run `cargo package` for every crate, in order).
      *Done 2026-09-13: all six commands exit 0 on Windows, rustc 1.95.0, with
      PostgreSQL 17 required (`RUPRIZZLE_TEST_PG_URL` + `RUPRIZZLE_REQUIRE_DB=1`, as in
      CI's `postgres` leg; MySQL left to R6).*

      | Command | Result |
      |---|---|
      | `xtask ci` (fmt, clippy, test, doc) | pass — 111 test binaries ok, 3 ignored tests |
      | `xtask harden` | pass — lint, test, docs, deny, package checks; panic and arithmetic/indexing audits within budget; injection audit clean |
      | `deny check` | advisories, bans, licenses, sources ok |
      | clippy `ruprizzle-cli --features studio` | pass, no warnings |
      | test `ruprizzle-cli --features studio` | pass |
      | `xtask release` (dry-run) | all 12 crates packaged in publish order |

      *Found and fixed:* R2 left the 15 codegen `*_generated.snap` snapshots asserting
      `RUPRIZZLE_VERSION = "1.5.0"`, failing `snapshots::all_examples_all_dialects`
      (`505b4b1`). *Note:* with `RUPRIZZLE_REQUIRE_DB=1` but no `RUPRIZZLE_TEST_PG_URL`,
      `runtime/tests/arrays.rs` panics by design, so a local gate must set both.
- [x] **R6 — MySQL/MariaDB.** Never verified locally; depends on CI's `integration`
      job. Require it green on the release commit.
      *Done locally 2026-09-13; CI `integration` still to be confirmed green on the
      release commit after push.* CI was red on `main` (`f3faa0f`), and running the
      suite locally showed why. Setup: portable MySQL 8.4.9 (CI's version) and MariaDB
      11.4.8, plus PostgreSQL 17, with `RUPRIZZLE_REQUIRE_DB=1`, Windows, rustc 1.95.0.

      | Run (`cargo test --workspace --no-fail-fast`) | Result |
      |---|---|
      | Before fixes, MySQL 8.4 + PG | 10 test binaries failing |
      | After fixes, MySQL 8.4 + PG | 111 binaries, 0 failed, 72 MySQL cases, none skipped |
      | After fixes, MariaDB 11.4 + PG | 111 binaries, 0 failed, 72 MySQL cases, none skipped |
      | `cargo clippy --workspace --all-targets -D warnings`, `cargo fmt --check` | pass |

      *Library bugs fixed (each has a CHANGELOG `[1.5.1]` entry):*
      - `@default(uuid4())` generated `DEFAULT UUID()`, a syntax error; now
        `DEFAULT (UUID())` (`10acd37`).
      - Savepoints and nested transactions failed with error 1295: the commands went
        through the prepared-statement protocol (`448929c`).
      - `InsertManyQuery::exec` returned no rows (no `RETURNING`) (`df4ebb8`).
      - JSON path `.eq("str")` never matched: the string was bound as `'"str"'` (`a447e47`).
      - Rolling back a foreign-key-cycle migration failed with error 3730:
        `SET FOREIGN_KEY_CHECKS` ran on a different pooled connection (`e47e660`).

      *Test portability (`6fde8a0`):* a join order assertion without `ORDER BY`; `TEXT`
      used as a key; `$n` placeholders; an inline `REFERENCES` that MySQL ignores;
      `DECIMAL` and `TEXT` decoding through `sqlx::Any`.
      *Not covered:* CI tests MySQL only, with no MariaDB leg. Adding one is a follow-up.
- [x] **R7 — Registry token.** *Done 2026-09-13: after the owner ran
      `gh secret set CARGO_REGISTRY_TOKEN`, the `publish=false` dry run (run
      `34745364164`, commit `fe472b7`) passed every step and reported
      "CARGO_REGISTRY_TOKEN is present (35 characters)".* Confirm `CARGO_REGISTRY_TOKEN` is set in Actions
      secrets and visible to `release.yml`. Run `workflow_dispatch` with
      `publish=false` first; the credential preflight fails fast on an empty token.
      *Partly done 2026-09-13 — workflow fixed; confirming the secret needs repo access.*
      - *Found:* the step above could not work. The credential preflight ran only when
        publishing (`if: steps.mode.outputs.publish == 'true'`), so a `publish=false`
        dispatch never looked at the token.
      - *Fixed (`e6b9e74`):* the preflight runs in both modes. A publishing run still
        fails fast on an empty token; a dry run emits a warning and writes
        "visible / not visible (N characters)" to the job summary, never the value.
      - *Remaining (owner, needs GitHub access; `gh` is not authenticated here):*
        after pushing, run **Actions → Release → Run workflow** with `publish=false`
        on the release commit and confirm the summary shows the token as visible.
        Tick R7 then.
      - *Dry run done 2026-09-13 — **blocked: the secret does not exist.***
        `gh workflow run release.yml -f publish=false` (run `34742977453`) warned
        "CARGO_REGISTRY_TOKEN is empty. A publishing run (tag push) would fail here",
        and `GET /repos/…/actions/secrets` reports `total_count: 0`. **Owner action:**
        create a crates.io API token (scopes `publish-new` + `publish-update`, both new
        crate names from R8 included) and add it with
        `gh secret set CARGO_REGISTRY_TOKEN`, then re-run the dry run and tick R7.
      - *Also found by the dry run:* its test step failed on
        `snapshots::all_examples_all_dialects`. `Schema::fingerprint` hashed source
        byte offsets, so a CRLF (Windows) checkout generated a different `SCHEMA_HASH`
        than Linux. Fixed in `b2e34a9`.
      - *Pushing:* the `MainRule1` ruleset applies to every branch except `main` and
        names matching `dev*`, and requires signed commits and forbids creating
        branches. None of these commits is signed, so `docs/path-to-v1_5` is pushed
        as `dev-path-to-v1_5`. Merge it into `main` with a PR, or sign the commits.
- [x] **R8 — Crate names.** `ruprizzle-turso` and `ruprizzle-d1` have never been
      published. Confirm both names are still free on crates.io.
      *Done 2026-09-13: both free.* `GET crates.io/api/v1/crates/<name>` returns 404 for
      `ruprizzle-turso`, `ruprizzle_turso`, `ruprizzle-d1` and `ruprizzle_d1` (crates.io
      treats `-` and `_` as the same name), and the sparse index
      (`index.crates.io/ru/pr/<name>`) returns 404 for both, against 200 for
      `ruprizzle-core` as a control. Names cannot be reserved without publishing, so
      re-run the check right before R9. *Re-checked 2026-09-13 after the token was
      added: both still 404 on the API and the index.*
- [x] **R9 — Tag and publish.** Push `v1.5.1`, let `release.yml` publish, then run
      `scripts/check-release-state.sh --expect 1.5.1`. No file may call the version
      released until that command passes.
      *Done 2026-09-13: published.* PR #12 merged `dev-path-to-v1_5` into `main`
      (all 33 CI checks green; merge commit `d2c0ec3`), `v1.5.1` tagged on `d2c0ec3`.
      Release run `34746899780` passed its gate and published all 12 crates in 15m30s,
      including its own registry verification step. Re-run locally:
      `check-release-state.sh --expect 1.5.1` → "OK: every crate serves 1.5.1".
      *Now unblocked:* D2 (install commands and banners → `1.5.1`), the R3/R4 CI
      follow-ups (`public-api` baseline `1.5.1 --deny=all`, drop `release-type: major`),
      and turning CHANGELOG `[1.5.1] - pending publication` into the release date.
- [x] **R10 — State known gaps in the release notes:** Studio has no authentication;
      the Turso and D1 adapters have not been run against the live services;
      `askama` is still on 0.12.
      *Done 2026-09-13:* a "Known gaps" section in CHANGELOG `[1.5.1]` and a
      "Known gaps" paragraph in the RELEASES `1.5.1` entry. `askama = "0.12"` confirmed
      in `crates/cli/Cargo.toml`.

Not verified during this assessment: recent CI results (GitHub CLI was not
authenticated), clippy, and the full test suite. R5 and R6 cover them (both done locally).

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
