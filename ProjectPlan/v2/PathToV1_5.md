# Path to v1.5 — publish plan and docs cleanup

Assessed 2026-09-13 against `main` at `f3faa0f`.
**Post-release review 2026-09-30:** `1.5.1` is published. Ten known issues and logic
gaps (K1–K10) and the `1.5.2` patch plan are in [§6](#6-post-release-review--known-issues-and-logic-gaps).

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

- [x] **D1 — Before publishing:** fix 5.A and the two false claims in 5.B. None depends
      on the version number.
      *Done 2026-09-13 (post-publish catch-up):* `KnownLimitations.md` rewritten for
      `1.5.1` (shipped features removed from "deferred"; real gaps added);
      `FeaturesMasterComparison.md` re-audited — Turso/D1/Studio/replicas/cache/OTel/
      PostGIS/FTS/soft-delete rows added, measured-version caveat added;
      `DialectNotes.md` gained a per-dialect v1.1–v1.5 table; `MigratingFrom.md`
      gained a Prisma/Drizzle map; `Operations.md` pin fixed; `BenchmarkResults.md`
      and `SoakReport.md` labelled as pre-release `1.0.x` measurements.
      `Stability.md` and `MigrationGuideToV1.md` now say `1.0.0`/`1.0.1` were tagged
      but never published and name `1.5.1` as the first stable.
- [x] **D2 — Immediately after `check-release-state.sh --expect 1.5.1` passes:** update
      every install command and "only version on crates.io" banner to `1.5.1` in one
      commit. Keep `announcement.md` archived and write a 1.5 announcement.
      *Done 2026-09-13:* all install snippets now use bare `cargo add`/`cargo install`
      or the `1.5` line; every "current version" banner names `1.5.1` published
      2026-09-13. `announcement.md` stays archived with a corrected banner pointing
      at `WhatsNewV1_1ToV1_5.md`, which serves as the 1.5 announcement.
- [x] **D3 — Prevent recurrence:** add a docs CI step that fails if any `docs/` page
      names an installable version other than the one the registry serves (excluding
      ADRs and archived pages), reusing `scripts/check-release-state.sh`.
      *Done 2026-09-13:* `scripts/check-docs-version.sh` queries crates.io for the
      served version and fails if any non-excluded doc pins a different one; wired
      into the `docs` job in `ci.yml`. The R3/R4 follow-ups landed with it —
      `public-api` now diffs against `1.5.1 --deny=all`, and the `semver-checks`
      `release-type: major` exception is removed.
- [x] **D4 — Site rebuild:** `pages.yml` deploys from `main` on merge. Spot-check the
      live `KnownLimitations.html`, which currently shows the stale text.
      *Done 2026-09-30:* the `Docs` workflow deployed `1ec1db3` (run `34751940876`).
      Live `KnownLimitations.html`, `faq.html` and `index.html` all name `1.5.1`. None
      of them contains "Deferred to v1.2", "Current beta" or an rc.1 install banner.

## 6. Post-release review — known issues and logic gaps

Assessed 2026-09-30 against `main` at `1ec1db3`, which is the published `1.5.1` plus
docs-only commits. The rating and scorecard are in
[`ProductionReadinessV1_5.md` §11](ProductionReadinessV1_5.md#11-re-assessment-of-the-published-151-2026-09-30)
(72 / 100, CONDITIONAL). This section is the working list. Every item has been
checked against the code; nothing here is inferred from a name alone.

The pattern behind most of these: **§8–§10 checked that each feature existed. This
review checked that it does what its rustdoc and `docs/` say.** Most gaps are on a
side path next to a correct main path, or are a builder setting that is stored and
never read. Compiler, clippy and the existing tests cannot see either kind.

### 6.A Known issues

#### K1 — Studio accepts cross-origin requests (High, security)

`crates/cli/src/studio/routes.rs:21` builds the router with no middleware. No handler
checks `Origin`, `Referer` or `Host`, and there is no CSRF token. Studio binds to
`127.0.0.1`, but that stops remote *hosts*, not a remote *page* in the developer's
own browser:

- **CSRF.** `POST /studio/sandbox/execute`, `POST …/rows`, `PATCH …/cell` and
  `DELETE …/rows/{id}` take `application/x-www-form-urlencoded` (or no body). A
  cross-origin `<form>` POST is a CORS "simple request", so the browser sends it
  with no preflight.
- **DNS rebinding.** A hostname that re-resolves to `127.0.0.1` reaches Studio with
  `Host: attacker.example`, which is accepted. The page is then same-origin with
  Studio and can *read* responses, including every table through
  `GET /studio/models/{m}/table`.

Browser local-network protections differ by vendor and version, so they cannot be
the defence.

**Fix:** reject any request whose `Host` is not `localhost`, `127.0.0.1` or `[::1]`
with the bound port, unless `--yes-i-know` is set for a non-loopback bind. Reject
any non-GET request whose `Origin` is absent or does not match. Add a per-process
random token to every mutating form, and require it. Test all three with `tower`
`oneshot` requests that carry a foreign `Origin` and `Host`.

**Status: fixed** on `fix/v1-5-2-k1-k5` (`crates/cli/src/studio/guard.rs`). The token
is delivered through `hx-headers` on `<body>` rather than per form, so every htmx
request carries it. Consequence: scripted `curl` mutations are refused too.

#### K2 — Studio's read-only gate can be bypassed (High, data safety)

`sandbox::is_read_only` (`handlers/sandbox.rs:62`) classifies a statement by its
first keyword. `SELECT`, `WITH` and `EXPLAIN` all pass as reads, which misses the
following:

| Input to `/studio/sandbox/execute` (no `--allow-writes`) | Effect |
|---|---|
| `EXPLAIN ANALYZE DELETE FROM users` | Postgres **executes** the `DELETE`. `EXPLAIN ANALYZE` runs the statement it explains. |
| `WITH d AS (DELETE FROM users RETURNING *) SELECT count(*) FROM d` | Postgres data-modifying CTE; the rows are deleted |
| `SELECT … INTO new_table`, `SELECT setval(…)`, `SELECT pg_terminate_backend(…)` | DDL, sequence mutation, or killing other sessions |

`/studio/explain` (`handlers/explain.rs:98`) builds `format!("EXPLAIN {sql}")` and
has **no** `allow_writes` check at all. Submitting `ANALYZE DELETE FROM users`
produces `EXPLAIN ANALYZE DELETE FROM users`.

**Fix:** stop classifying SQL text. Run every read-mode statement inside a
database-enforced read-only transaction, then roll it back:
- Postgres: `BEGIN; SET TRANSACTION READ ONLY; …; ROLLBACK`
- MySQL: `START TRANSACTION READ ONLY`
- SQLite: `PRAGMA query_only = ON` on the connection

In `/studio/explain`, refuse input that starts with `ANALYZE` or `(`, and run it
through the same read-only transaction. Add regression tests for each row of the
table above on SQLite and Postgres.

**Status: fixed** on `fix/v1-5-2-k1-k5` (`studio/db.rs::fetch_dynamic_read_only`).
One deviation from the plan: SQLite does not use `PRAGMA query_only`, because
SQLite runs every `;`-separated statement in one call and a later statement could
switch the pragma off. It opens a separate `SQLITE_OPEN_READONLY` connection
instead. The same multi-statement behaviour was a second, SQLite-only K2 vector
(`SELECT 1; DELETE …`), now covered by a test. MySQL is implemented but was not
run locally.

#### K3 — `soft_delete()` fails on Postgres (High, correctness)

`UpdateQuery::soft_delete` (`crates/runtime/src/query.rs:2092`) does this:

```rust
let now = chrono::Utc::now().to_rfc3339();
Ok(self.set(Column::<M, String>::new(M::TABLE, col), now))
```

This binds a `Value::Str`, and on Postgres `Value::Str` carries type `TEXT`
(`value.rs`, `impl sqlx::Type<sqlx::Postgres>`). The parser requires `@deletedAt` to
be `DateTime?` (`parser/src/lower.rs:656`), so the column is a timestamp.
Reproduced in PostgreSQL 17.10 with the statement shape sqlx sends:

```
PREPARE p(text) AS UPDATE sd SET deleted_at = $1 WHERE id = 1;
ERROR:  column "deleted_at" is of type timestamp with time zone but expression is of type text
```

Every Postgres user of the documented soft-delete write gets this error. The only
test (`crates/runtime/tests/v1_1_features.rs`) declares `deleted_at TEXT`, which
hides the problem. MySQL has not been verified: `DATETIME` may reject the `+00:00`
suffix under strict mode. The stamp also comes from the client clock, not the
database's `now()`, although the rustdoc says `now()`.

**Fix:** bind `Value::DateTime(Utc::now())`, or emit the dialect's `CURRENT_TIMESTAMP`
as a SQL expression. Change the test fixture to the column type that
`ruprizzle migrate` generates, and run it on all three dialects.

**Status: fixed** on `fix/v1-5-2-k1-k5`. Binds `Value::DateTime`. Client clock kept,
and the rustdoc corrected, rather than emitting `CURRENT_TIMESTAMP`, because SQLite's
`CURRENT_TIMESTAMP` text format differs from the RFC 3339 that every other
`DateTime` write stores. The test fixture is now `TIMESTAMPTZ` on Postgres. Still
open: the `v1_1_features` fixture has no MySQL leg.

#### K4 — Soft-deleted rows leak through side paths (Medium, correctness)

`SelectQuery::effective_filter` (`query.rs:514`) adds `deleted_at IS NULL`, and
`fetch_all`, `count`, `exists` and `aggregate` all use it. These paths build SQL
without it:

| Path | Where | Result |
|---|---|---|
| Include with a per-parent limit (window-function dialects) | `include.rs:85`, `select_partitioned::<C>` | `include(posts.take(5))` returns deleted posts, while `include(posts)` does not |
| Many-to-many loads | `m2m.rs:185`, `compile::select::<C>` | Deleted related rows appear |
| Relation filters `some` / `every` / `none` | `rel.rs:231`, `FilterNode::InSubquery` | "Has a post" matches a user whose only post is deleted |
| Hierarchy `descendants` / `ancestors` | `hierarchy.rs` `to_sql` | Deleted nodes, and subtrees reached through them, are returned |
| Join right-hand side | `join_select_with_columns` | Only the left model's filter is applied |

`docs/WhatsNewV1_1ToV1_5.md` says "every generated query filters
`WHERE deleted_at IS NULL`". That is not true for the rows above.

**Fix:** apply the child model's soft-delete predicate in each of these paths. Add
one test per row. Until then, correct the docs sentence.

**Status: fixed** on `fix/v1-5-2-k1-k5`. Two corrections to the table above, found
while fixing it. First, the plain m2m *include* (`IncludeMany`) already went through
`SelectQuery` and was correct. The leak at `m2m.rs:185` is the reload after an m2m
*write*. Second, the relation filters are emitted by codegen (`emit.rs`, `_some` /
`_none` / `_every`). `rel.rs:231` is the delete-cascade filter, which is a write and
correctly unfiltered. Users must regenerate to get the relation-filter part.
`HierarchyQuery` has no `with_deleted()` opt-out yet, because adding one changes
public API. That is deferred to `1.6`.

#### K5 — Public builder methods that do nothing (Medium, API honesty)

In `crates/runtime/src/query.rs:210–238`, all five of these bodies are `self` with
the argument discarded:

| Method | Rustdoc promise |
|---|---|
| `SelectQuery::cache(ttl)` | "Sets query result caching with a specific TTL" |
| `SelectQuery::cache_key(key)` | "Sets a custom cache key" |
| `SelectQuery::cache_tag(tag)` | "Associates a cache invalidation tag" |
| `SelectQuery::use_primary()` | "Forces this query to execute on the primary database pool" |
| `SelectQuery::use_replica()` | "Directs this query to execute on a read replica pool" |

The same gap shows up elsewhere:
- `QueryCache` / `InMemoryCache` work as a standalone key-value store, but nothing
  in query execution reads or writes them.
- `metrics::CACHE_HITS_TOTAL` and `CACHE_MISSES_TOTAL` are defined, and the only
  test asserts their string values. No code emits them.
- `compile::PlanCache` is used only by its own tests.

A user who writes `.use_primary()` for read-your-writes consistency gets a replica
read with no error.

**Fix (1.5.2, semver-safe):** mark all five `#[deprecated(note = "no effect in 1.5;
use RoutedPool::primary() / InMemoryCache directly")]`, rewrite their rustdoc to say
they do nothing, and state in the docs that the cache is manual. **Fix (2.0):**
either implement them, or remove them together with `PlanCache` and the two unused
metric names.

**Status: 1.5.2 part fixed** on `fix/v1-5-2-k1-k5`. The `public-api --deny=all` diff
against `1.5.1` is empty, so deprecating the methods passes the gate. The 2.0 half is
still open.

#### K6 — `RoutedPool` settings that are never read (Medium, correctness)

In `crates/runtime/src/pool.rs`:

- **`fallback_to_primary(false)` has no effect.** The field is stored (`:659`) and
  returned by `falls_back_to_primary()`. But `select_replica` (`:694`) returns
  `&self.primary` whenever no replica is healthy, whatever the flag says. A user
  who disabled fallback to protect the primary still gets primary reads.
- **`LeastConnections` always picks the first replica.** `ReplicaPool::active_conns`
  is never incremented or decremented by any executor path, so every replica
  reports 0 and `min_by_key` returns the first. The test sets the counters by hand.
- **`Random` is not random.** It computes `idx * 2654435761 mod n` from the
  round-robin counter. For two replicas this is plain alternation.
- **Health is never checked automatically.** `check_health()` only runs when the
  application calls it. The docs say "unhealthy replicas are skipped", which holds
  only if the application runs its own health loop.
- **Routing is by method, not by statement.** In the `Executor` impl
  (`executor.rs:796`), every `fetch_all_raw` and `stream_raw` goes to a replica. ORM
  writes are safe, because `Insert/Update/DeleteQuery` take a concrete `&Pool`. A
  raw prepared `fetch` of `INSERT … RETURNING`, `UPDATE … RETURNING` or
  `SELECT … FOR UPDATE` through a `RoutedPool`, however, goes to a replica and fails
  there or reads stale data.
- **The tests cannot catch any of this.** `tests/integration/tests/replica_routing.rs`
  checks the choice with `provider()`, and all three pools are SQLite.

**Fix:** honour the flag by returning an error when fallback is off and no replica
is healthy. Keep an RAII guard on `active_conns` around every routed call. Either
seed a real RNG for `Random` or rename it. Document `check_health` as the caller's
job, or add an optional `spawn_health_checks(interval)`. Route raw statements by
their leading keyword, sending anything other than `SELECT`/`WITH … SELECT` to the
primary. Rewrite the tests to use three distinct file databases, each holding a
marker row.

**Status: fixed** on `fix/v1-5-2-k6-k10`, with no public API change (the `public-api
--deny=all` diff is empty). `fallback_to_primary(false)` makes the executor return an
error; `select_replica()` cannot fail, so it returns the first (unhealthy) replica
instead of the primary. A router with **no** replicas still reads from the primary.
An `InFlight` guard holds `active_conns` for each routed read, and for a stream until
it is dropped. `Random` hashes a counter with a fresh `RandomState`. Raw statements go
to a replica only if `is_replica_safe` passes (read keyword first, no
write/lock/sequence word anywhere). `check_health` is documented as manual rather
than adding a `spawn_health_checks` method, which would be new public API; that is
deferred to 1.6.

#### K7 — Tree "cycle protection" is a depth cap (Medium, correctness)

`hierarchy.rs:253`: with `cycle_protection` on (the default) and no `max_depth`, the
recursion condition is `h.__depth < 100`. That caps runaway recursion but does not
detect a cycle. With `A → B → A`, the `UNION ALL` emits A and B about 50 times each,
and `HierarchyNode::build` receives the duplicates. With `cycle_protection(false)`
and no `max_depth`, the condition is `1 = 1`: Postgres and SQLite recurse until the
statement is cancelled, and MySQL fails at `cte_max_recursion_depth` (1000).

**Fix:** keep a visited-path column and stop on revisit. On Postgres 14+ use
`CYCLE id SET is_cycle USING path`. On SQLite and MySQL, carry a delimited path
string and test it with `instr`. Rename the option if it stays a depth cap. Add a
cyclic-data test that asserts no duplicate ids.

**Status: fixed** on `fix/v1-5-2-k6-k10`. All three dialects carry a text path
(`,k1,k2,`) and test it with `strpos` / `LOCATE` / `instr`. Postgres does not use an
array or the `CYCLE` clause: a model without `COLUMNS` selects `*` from the CTE, and
the sqlx `Any` driver cannot decode an array column. The 100-level default cap is kept
so trees deeper than that behave as in 1.5.1. Known limit: a text key containing a
comma can be mistaken for a visited node. `cycle_protection(false)` without
`max_depth` still recurses without end on cyclic data; that is now documented.

#### K8 — RUSTSEC-2026-0285 fails the dependency gate (Medium, supply chain / CI)

rustls 0.23.43 is in `Cargo.lock`. It is reached only via `reqwest` →
`hyper-rustls` / `tokio-rustls` from `ruprizzle-turso` and `ruprizzle-d1`. rustls
accepts TLS 1.3 handshake messages across encryption-level boundaries. The
transcript is still authenticated, so the practical impact is low. But
`cargo deny check` fails, so `cargo xtask harden` fails, and the CI `deny` step will
fail on the next push to `main`. The last green `ci.yml` run (2026-09-13) predates
the advisory.

**Fix:** `cargo update -p rustls` (to ≥ 0.23.45), then re-run `cargo deny check`.
Downstream users who resolve fresh already get the fixed version; the lockfile only
affects this repository.

**Status: fixed** on `fix/v1-5-2-k6-k10`. rustls is 0.23.45 and `cargo deny check`
passes. `cargo xtask harden` then exposed one more direct index in the K1 code
(`studio/guard.rs`, CLI indexing budget 5 of 4); it now uses `get`, and `harden`
completes.

#### K9 — The mutation job is red and unwatched (Medium, test quality)

`mutants.yml` has failed on every scheduled run since the release: 2026-09-14,
09-21 and 09-28 (run `36380122023`). On 09-28:

| Job | Caught | Missed | Timeouts | Kill rate |
|---|---|---|---|---|
| `migrate` | 169 | 395 | 13 | 30% |
| `runtime` shards 1–3 | 229 | 698 | — | 25% |

The runtime figure is overstated, because the job has no database services and the
Postgres/MySQL tests skip themselves. The `migrate` misses are real, because they
are in pure functions:

- `Change::is_destructive → true` survives (`migrate/src/change.rs:73`). No test
  checks that a **safe** change is reported as safe.
- `diff_enums → ()` survives (`migrate/src/diff.rs:18`). No test checks that enum
  changes are diffed at all.
- `||` → `&&` in `diff_columns` (`diff.rs:89`, `:153`) survives.

**Fix:** add table-driven tests for `is_destructive` across every `Change` variant,
and enum add/remove/rename diff tests. Then set a kill-rate floor for `migrate` in
the workflow, so the job fails on a regression rather than always. Add Postgres
and MySQL services to the runtime shards, or scope them to DB-free modules.

**Status: fixed** on `fix/v1-5-2-k6-k10`. `crates/migrate/tests/change_classification.rs`
(13 tests). Local `cargo mutants -p ruprizzle-migrate` (27.1.0): **212 caught, 354
missed, 11 timeouts, 53 unviable = 37.5%**, up from 30%. On `change.rs` + `diff.rs`
alone: 19 → 62 of 72 viable caught; every survivor named above is now caught. The
10 left there are near-equivalent: `scalar_changed` arms that fall through to
`prev != next`, `db_name == … && p == ix` where `p == ix` implies the first, and the
last three `||` in the FK comparison, where the constraint name is derived from the
owner columns. `mutants.yml`: `migrate` fails only below `KILL_RATE_FLOOR=35`
(`.github/scripts/mutants_floor.py`, exit codes 2/3 from cargo-mutants are
tolerated). Runtime shards get Postgres 17 and MySQL 8.4 services with
`RUPRIZZLE_REQUIRE_DB=1` and report their rate with no floor until a baseline with
databases exists. The workflow change is not yet run on GitHub.

#### K11 — `@renamedFrom` onto an existing column name (Low, found while fixing K9)

`diff_columns` with `prev {a Int, b String}` and `next {b Int @renamedFrom("a")}`
emits only `RenameColumn a -> b`. The old `b` is neither dropped nor altered, so
the planned `RENAME COLUMN a TO b` collides with the existing column and the
migration fails at apply time rather than at plan time. **Fix (1.6):** drop the old
same-named column first (destructive, so it goes behind `--accept-data-loss`), or
refuse the plan with a clear diagnostic. Pinned in part by
`a_renamed_field_is_not_altered_against_a_namesake`.

**Fixed (1.6):** `diff_columns` now emits a `DropColumn` for the shadowed old
column (destructive, so it needs `--accept-data-loss`), and the planner runs such
drops in a phase before `columns_to_rename`. Pinned by
`a_renamed_field_is_not_altered_against_a_namesake`, which now asserts the drop and
that it precedes the rename in `up.sql`.

#### K10 — D1 binds a non-finite float as `NULL` (Low)

`crates/d1/src/api.rs:131`: `Number::from_f64(f).map_or(Json::Null, …)`. `NaN` and
`±inf` are written as `NULL` without an error, while bytes and arrays in the same
function are refused with `D1Error::Unsupported`. Make non-finite floats
`Unsupported` too. Also document that D1 returns integers as JSON numbers, so
values beyond 2^53 lose precision on the server before the adapter sees them.

**Status: fixed** on `fix/v1-5-2-k6-k10`. `to_json` returns `D1Error::Unsupported`
for `NaN` / `±inf`. The same gap was in `ruprizzle-turso` (`to_hrana` put the float in
`HranaValue::Float`, which serde_json serialises as `"value": null`), so it is fixed
there too. The ±2^53 limit is documented in the `ruprizzle-d1` crate docs and in
`KnownLimitations.md`. Neither adapter has been run against the live service.

#### Carried forward (already in the `1.5.1` release notes, still open)

- Studio has no authentication. K1 is a separate, cheaper fix that does not depend
  on building authentication.
- The Turso and D1 adapters have not been run against the live services.
- `askama` is on 0.12.
- CI has no MariaDB leg (R6 follow-up).

### 6.B `1.5.2` patch plan

Semver-safe items only. K5's removal and K6's `Result`-returning fallback wait for
`2.0` if they change a signature.

- [x] **P1 — K8.** *Done — see ProductionReadinessV1_5 §11.6.* `cargo update -p rustls`, `cargo deny check`, `cargo xtask harden`.
      Do this first, so CI is green before any other change lands.
- [x] **P2 — K1.** *Done — see ProductionReadinessV1_5 §11.6.* `Host` / `Origin` validation and a per-process form token in Studio,
      with `oneshot` tests.
- [x] **P3 — K2.** *Done — see ProductionReadinessV1_5 §11.6.* Read-only transactions for sandbox reads and EXPLAIN. Refuse
      `ANALYZE` in `/studio/explain`. Add regression tests on SQLite and Postgres.
- [x] **P4 — K3.** *Done (PG + SQLite; MySQL not run) — see ProductionReadinessV1_5 §11.6.* `soft_delete()` binds a real timestamp. Test on a `DateTime`
      column on all three dialects.
- [x] **P5 — K4.** *Done — see ProductionReadinessV1_5 §11.6.* Soft-delete predicate in partitioned includes, m2m, relation
      filters, hierarchy and join right-hand side. Correct the `WhatsNew` sentence.
- [x] **P6 — K5.** *Done — see ProductionReadinessV1_5 §11.6.* Deprecate the five no-op methods and rewrite their rustdoc.
      Document the cache as manual.
- [x] **P7 — K6.** *Done — see ProductionReadinessV1_5 §11.6.* Keep the `active_conns` guard, route raw statements by keyword,
      make the `Random` / health-check docs honest, rewrite the routing tests.
      Honour `fallback_to_primary(false)` without a signature change if possible
      (for example, route to the first unhealthy replica and let it error).
      Otherwise defer to 2.0.
- [x] **P8 — K7.** *Done — see ProductionReadinessV1_5 §11.6.* Real cycle detection, and a cyclic-data test.
- [x] **P9 — K9.** *Done — see ProductionReadinessV1_5 §11.6.* `is_destructive` and `diff_enums` tests, and a kill-rate floor for
      `migrate` in `mutants.yml`.
- [x] **P10 — K10.** *Done — see ProductionReadinessV1_5 §11.6.* Refuse non-finite `f64` in D1.
- [ ] **P11 — Release.** CHANGELOG `[1.5.2]` with a **Fixed** entry per item, then
      `cargo xtask release-check --tag v1.5.2`, then the R5 local gate, then tag
      through `dev-main → main`. **Branching note:** `dev-main` is currently behind
      `main` by the entire 1.5.1 release. Fast-forward it to `main` before
      branching the patch work from it.
