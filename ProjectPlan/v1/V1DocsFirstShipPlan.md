# v1.0 Ship with Production Usage Docs — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use `superpowers:subagent-driven-development`
> (recommended) or `superpowers:executing-plans` to implement this plan task-by-task.
> Steps use checkbox (`- [ ]`) syntax for tracking. Do not batch tasks across
> doc-writing and release-process boundaries; each has its own exit gate.

**Goal:** Make the user-facing documentation accurate, complete, and runnable for
 the `1.0.0-rc.1` public API, then finish the release process: the in-progress
 48-hour `rusqlite` soak, `1.0.0-rc.1` crates.io publish, production-readiness
 rescoring, RC feedback window, and final `1.0.0` GA.

**Architecture:** The work is split into two sequential phases. **Phase 1 (docs
 first)** updates existing Markdown under `docs/` and `README.md`, rewrites the
 quickstart and FAQ, expands the query/relations/migrations/schema guides, and
 adds a runnable `examples/blog` project. **Phase 2 (release after)** waits for
 the 48-hour soak to finish, triages any remaining errors, publishes the RC,
 re-runs the W6-05 production-readiness assessment, runs the RC feedback window,
 and cuts `1.0.0`. The docs phase is independent of the soak and can be merged
 before the release phase completes.

**Tech Stack:** Rust 2024, `mdbook`, `cargo xtask`, `cargo doc`, `cargo publish`,
 GitHub Actions. Docs are plain Markdown under `docs/` and are published to GitHub
 Pages from `main`/`master` via `.github/workflows/pages.yml`.

---

## Execution status

| Phase | Status | Notes |
|---|---|---|
| Phase 0 — Stabilize context | completed | 48-hour soak process `soak-9c6b6ecac4cbf8a3.exe` is alive; 2 errors classified as environmental (Windows `disk I/O error` and `os error 1450` printing to stderr); memory stable at ~7 MiB. Uncommitted `ProjectPlan/ProductionReadiness.md` diff committed. |
| Phase 1 — Version/stale-claim cleanup | completed | README.md, docs/README.md, announcement, FAQ, SUMMARY, book.toml, Operations, CHANGELOG updated to 1.0.0-rc.1. |
| Phase 2 — Core usage guides | completed | QueryGuide.md and RelationsGuide.md expanded with all required sections and snippets. |
| Phase 3 — Runnable example project | completed | `examples/blog/` created with Cargo.toml, .env.example, README, and src/main.rs. |
| Phase 4 — Doc verification | completed | `mdbook build`, `cargo doc`, `cargo xtask fmt`, and `cargo xtask lint` pass. `cargo xtask test` blocked by an environmental Postgres `No space left on device` failure; Rust unit tests pass. |
| Phase 5 — Release finalization | pending | blocked on 48-hour soak completion, rescoring, RC publish, and GA cut. |

---

## Global Constraints

- Branch: `dev-v0-2` (repository default per `AGENTS.md`). Do not switch branches
  unless the user explicitly requests it.
- Workspace version: `1.0.0-rc.1` (`Cargo.toml`). Update all user-facing docs that
  still say `0.4.0-beta.2` or earlier.
- MSRV: Rust 1.85.
- `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D
  warnings`, `cargo test --workspace`, `cargo doc --workspace --no-deps`,
  `cargo xtask harden`, and `mdbook build` must remain green after every task.
- Every Rust code snippet in docs must either be realistic and compile (so it can
  be verified by `mdbook test` or `cargo xtask examples`) or be wrapped in
  `rust,ignore` / `text` blocks. Do not present code that cannot compile as a
  working example.
- No new `unsafe` code. No new panics in library source. Do not increase
  `PANIC_BUDGET` or `BUDGETS` in `xtask/src/main.rs`.
- Do not commit secrets. Example `.env` files must be `.env.example`; real `.env`
  files must already be in `.gitignore`.
- Do not add emojis to files unless explicitly requested.
- Generated-code examples inside the repo may use path dependencies for
  `ruprizzle` / `ruprizzle-migrate` so CI can compile them. User-facing snippets
  in README / quickstart / guides should use `cargo add ruprizzle` /
  `cargo install ruprizzle-cli`.

---

## File structure

| File | Responsibility | Phase |
|---|---|---|
| `README.md` | Landing page, install, feature list, quick example | 1 |
| `docs/README.md` | mdBook introduction, install, feature table | 1 |
| `docs/SUMMARY.md` | mdBook table of contents | 1 |
| `book.toml` | mdBook configuration (description, site URL) | 1 |
| `docs/announcement.md` | 1.0.0-rc.1 / 1.0.0 release announcement | 1 |
| `docs/faq.md` | Frequently asked questions, JSON-LD FAQPage | 1 |
| `docs/quickstart.md` | End-to-end five-minute tutorial | 1 |
| `docs/QueryGuide.md` | Complete query-builder feature reference | 1 |
| `docs/RelationsGuide.md` | Relation loading and nested writes | 1 |
| `docs/MigrationsGuide.md` | Migrations, seed, pull, deploy, drift | 1 |
| `docs/SchemaReference.md` | Full `schema.ruprizzle` language reference | 1 |
| `docs/Operations.md` | Production telemetry and runbook | 1 |
| `docs/known-limitations.md` | Stub that redirects to `KnownLimitations.md` | 1 |
| `examples/blog/Cargo.toml` | Runnable blog example project | 1 |
| `examples/blog/src/main.rs` | Blog example application code | 1 |
| `examples/blog/.env.example` | Blog example database URL template | 1 |
| `examples/blog/README.md` | How to run the blog example | 1 |
| `CHANGELOG.md` | Version history and `[Unreleased]` | 1 |
| `docs/SoakReport.md` | 48-hour soak final report | 2 |
| `ProjectPlan/ProductionReadiness.md` | W6-05 re-assessment | 2 |

---

## Phase 0 — Stabilize current context (non-blocking, < 1 day)

### Task 0.1: Inspect the running 48-hour `rusqlite` soak and the uncommitted diff

**Files:**
- Read: `logs/soak-48h-rusqlite.err`, `logs/soak-48h-rusqlite.log`
- Read: `ProjectPlan/ProductionReadiness.md` (working-tree diff)

**Interfaces:**
- Consumes: live process `soak-9c6b6ecac4cbf8a3.exe`, the `.err` log.
- Produces: a note in this plan's task list or in `docs/SoakReport.md` about
  whether the two logged errors are acceptable.

- [ ] **Step 1: Confirm the soak process is still alive.**
  ```powershell
  tasklist | findstr soak
  ```
  Expected: `soak-9c6b6ecac4cbf8a3.exe` is still running.

- [ ] **Step 2: Read the error context.**
  ```powershell
  Get-Content 'logs/soak-48h-rusqlite.err' | Select-String -Pattern 'soak op error|panicked' -Context 3,3
  ```
  Expected: The two errors are `disk I/O error` and a Windows `os error 1450`
  (`Insufficient system resources exist to complete the requested service`) while
  printing to stderr. These are not ruprizzle logic failures.

- [ ] **Step 3: Determine whether the run is still useful.**
  - If `errors` stays at 2 and `memory_bytes` is stable, the run can continue to
    the full 48 hours and the errors can be footnoted as environmental.
  - If `errors` climbs, or if `memory_bytes` grows without bound, stop the run and
    investigate before restarting.

- [ ] **Step 4: Record the current `ProductionReadiness.md` diff.**
  ```bash
  git diff -- ProjectPlan/ProductionReadiness.md
  ```
  Expected: A one-line score change from 86 to 87 and a wording change to
  "RC tagged, mechanically green, 48-hour soak in progress".

- [ ] **Step 5: Decide whether to keep the diff.**
  - If the score change is intentional, stage and commit it separately from the
    docs work.
  - If the score should wait for the final soak, `git checkout --
    ProjectPlan/ProductionReadiness.md` and re-apply after W6-05.

---

## Phase 1 — Version and stale-claim cleanup (1–2 days)

### Task 1.1: Update `README.md` status, supported databases, and query-checking claim

**Files:**
- Modify: `README.md:18` (status line)
- Modify: `README.md:16` (backends line)
- Modify: `README.md:439` (comparison table `Compile-time query checking` row)
- Modify: `README.md:459` (crate table version note)

**Interfaces:**
- Consumes: `Cargo.toml` `workspace.package.version`, `docs/KnownLimitations.md`.
- Produces: `README.md` no longer claims `0.4.0-beta.2` or missing MySQL / query
  checking.

- [ ] **Step 1: Replace the status paragraph.**
  Old (lines 17–18):
  ```markdown
  > **Status:** `0.4.0-beta.2` is published on crates.io. P0–P8 are complete, the public API is stabilising, and a native `rusqlite` SQLite backend is available behind the `sqlite-rusqlite` feature. See [Known limitations](#known-limitations) for the honest boundaries of the beta.
  ```
  New:
  ```markdown
  > **Status:** `1.0.0-rc.1` is the release candidate on crates.io. The P0–P8
  > feature work is complete, MySQL/MariaDB support is shipped, and the public
  > API is frozen for the 1.0 line. See [Known limitations](#known-limitations)
  > for deliberate boundaries and [Stability](docs/Stability.md) for the semver
  > policy.
  ```

- [ ] **Step 2: Update the backends line.**
  Old (line 16):
  ```markdown
  Postgres, SQLite, and MySQL/MariaDB are supported from day one behind a `DbDialect` trait, so more backends are additive.
  ```
  New (already correct, only remove "from day one" if it appears twice elsewhere):
  Keep as is. Also update the backend line in `docs/README.md`.

- [ ] **Step 3: Update the comparison table.**
  Old (line 439):
  ```markdown
  | **Compile-time query checking** | planned | ✅ | ❌ | ✅ | ✅ | N/A | ❌ |
  ```
  New:
  ```markdown
  | **Compile-time query checking** | ✅ | ✅ | ❌ | ✅ | ✅ | N/A | ❌ |
  ```

- [ ] **Step 4: Update the crate table version note.**
  Old (line 459):
  ```markdown
  The workspace is split so that parser and codegen never enter the user's runtime dependency graph. Every crate in the table below uses the shared workspace version (`0.4.0-beta.2` at the time of writing).
  ```
  New:
  ```markdown
  The workspace is split so that parser and codegen never enter the user's runtime dependency graph. Every crate in the table below uses the shared workspace version (`1.0.0-rc.1` at the time of writing).
  ```

- [ ] **Step 5: Verify internal links still resolve.**
  Run: `cargo xtask ci` or at least `cargo xtask docs`.
  Expected: PASS.

- [ ] **Step 6: Commit.**
  ```bash
  git add README.md
  git commit -m "docs: update README to 1.0.0-rc.1 status and feature claims"
  ```

---

### Task 1.2: Update `docs/README.md`

**Files:**
- Modify: `docs/README.md:17` (backends line)
- Modify: `docs/README.md:21-28` (status block)
- Modify: `docs/README.md:134` (compile-time query checking row)
- Modify: `docs/README.md:145-153` (crate table version note, crate list)

**Interfaces:**
- Consumes: `README.md` changes from Task 1.1.
- Produces: consistent 1.0.0-rc.1 messaging in the mdBook landing page.

- [ ] **Step 1: Update the backends line.**
  Old (line 17):
  ```markdown
  Postgres and SQLite are supported from day one behind a dialect trait, so more
  backends are additive.
  ```
  New:
  ```markdown
  PostgreSQL, MySQL/MariaDB, and SQLite 3+ are supported behind a `DbDialect`
  trait, so more backends are additive.
  ```

- [ ] **Step 2: Update the status block.**
  Old (lines 21–28):
  ```markdown
  `0.4.0-beta.2` is published on crates.io. The core P0–P8 implementation is
  complete and the public API is now stabilising; the production-readiness
  assessment has been refreshed for `0.4.0-beta.2`. See the
  [implementation plan](../ProjectPlan/ImplementationPlan/MasterPlan.md) for the
  phase state and the [production-readiness
  plan](../ProjectPlan/ProductionReadinessPlan.md) for the assessment.
  ```
  New:
  ```markdown
  `1.0.0-rc.1` is the release candidate on crates.io. All P0–P8 work is complete,
  MySQL/MariaDB support is shipped, and the public API is frozen for the 1.0 line.
  The production-readiness assessment is in W6-05 (rescoring against the live RC)
  and the 48-hour soak is the final W4-02 gate. See
  [Stability](Stability.md) for the semver policy and
  [Known limitations](KnownLimitations.md) for deliberate boundaries.
  ```

- [ ] **Step 3: Update the comparison table.**
  Change the `Compile-time query checking` row from `planned` to `✅`.

- [ ] **Step 4: Update the crate table version note and add missing crates.**
  Old (lines 145–153):
  ```markdown
  || `crates/core`    | IR, spans, diagnostics | ✅ P0 |
  || `crates/parser`  | Schema DSL → validated IR | ✅ P1 |
  || `crates/dialect` | `DbDialect` trait, Postgres + SQLite | ✅ P2 |
  || `crates/codegen` | IR → Rust source | ✅ P3 |
  || `crates/runtime` | `ruprizzle`, the crate your app depends on | ✅ P4 |
  || `crates/migrate` | Snapshot, diff, plan, apply | ✅ P6 |
  || `crates/cli`     | The `ruprizzle` binary | ✅ P7 |
  || `crates/testkit` | Dual-database test harness | ✅ P0 |
  ```
  New (add `crates/lsp` and `crates/check`; update dialect role):
  ```markdown
  || `crates/core`    | IR, spans, diagnostics | ✅ P0 |
  || `crates/parser`  | Schema DSL → validated IR | ✅ P1 |
  || `crates/dialect` | `DbDialect` trait, Postgres + MySQL + SQLite | ✅ P2 |
  || `crates/codegen` | IR → Rust source | ✅ P3 |
  || `crates/runtime` | `ruprizzle`, the crate your app depends on | ✅ P4 |
  || `crates/migrate` | Snapshot, diff, plan, apply | ✅ P6 |
  || `crates/cli`     | The `ruprizzle` binary | ✅ P7 |
  || `crates/lsp`     | Language server for `schema.ruprizzle` | ✅ P8 |
  || `crates/check`   | Offline / compile-time query checking | ✅ P8 |
  || `crates/testkit` | Dual-database test harness | ✅ P0 |
  ```

- [ ] **Step 5: Run `mdbook build` and `cargo xtask docs`.**
  Expected: no warnings.

- [ ] **Step 6: Commit.**
  ```bash
  git add docs/README.md
  git commit -m "docs: update docs/README for 1.0.0-rc.1 and missing crates"
  ```

---

### Task 1.3: Rewrite `docs/announcement.md` for the `1.0.0-rc.1` release

**Files:**
- Modify: `docs/announcement.md` (full rewrite)

**Interfaces:**
- Consumes: `Cargo.toml` version, `docs/KnownLimitations.md`,
  `ProjectPlan/v1/PathToStableV1.md` §5.
- Produces: a release-announcement document suitable for copying into a blog
  post / GitHub release notes.

- [ ] **Step 1: Replace the file content with the following markdown.**
  ```markdown
  # ruprizzle-orm 1.0.0-rc.1

  **ruprizzle-orm 1.0.0-rc.1** is now on [crates.io](https://crates.io/crates/ruprizzle).
  It is a schema-first ORM for Rust: write a Prisma-style `schema.ruprizzle`, get
  typed entities, a Drizzle-style query builder that shows you its SQL, and
  automatic migrations generated by diffing your schema. PostgreSQL, MySQL/MariaDB,
  and SQLite 3+ are supported. No query engine binary.

  This is a **release candidate**, not a stable release. The public API is frozen
  for the 1.0 line, and this two-week feedback window is the last opportunity to
  report API problems before `1.0.0`. See [Stability](Stability.md) for the semver
  policy and [Known limitations](KnownLimitations.md) for deliberate boundaries.

  ## What we claim

  - **Automatic migration diffing from a declarative schema** — no other Rust ORM has it.
  - **`include` with per-relation filters**, in a bounded query count.
  - **Column-token typing** that rejects cross-model and wrong-type filters at compile time.
  - **Identical Rust API across PostgreSQL, MySQL/MariaDB, and SQLite**.
  - **`.to_sql()` on every query**.
  - **Native `rusqlite` and `tokio-postgres` backends** behind feature flags.
  - **Advanced SQL builders**: CTEs (recursive and non-recursive), set operations,
    `EXISTS`/`IN` subqueries, conditional filters, nested inserts/updates, explicit
    `JOIN`s, aggregates, `GROUP BY` / `HAVING`, and JSON path operators.
  - **Offline / compile-time query checking** with `ruprizzle check` and query manifests.
  - **LSP for `schema.ruprizzle`** via `ruprizzle-lsp` and the VS Code extension in `editor/`.

  ## What we explicitly do not claim

  - Feature parity with Prisma or Drizzle on every edge-case (vector search, full-text,
    PostGIS, and implicit many-to-many join tables are deferred to 1.1+).
  - That the 1.0 API will not need a second RC if feedback reveals a real problem.

  Every one of those would be checked by someone within a day of the post, and
  being caught overclaiming would cost more than the attention gained.

  ## Supported in this release

  - Grammar-driven `.ruprizzle` parser with span-preserving diagnostics.
  - IR lowering, fingerprinting, and round-trip serialisation.
  - PostgreSQL, MySQL/MariaDB, and SQLite 3+ dialects with conformance suites.
  - Rust entity and query-builder code generation.
  - Runtime CRUD, transactions, savepoints, pagination, `include` loading, prepared
    statements, buffered and unbuffered streaming, and metrics export.
  - Advanced query builders: conditional/dynamic filters, `IN` sets, count/exists,
    aggregates, `GROUP BY` / `HAVING`, CTEs, set operations, `EXISTS`/`IN`
    subqueries, nested inserts/updates, explicit `JOIN`s, and JSON path operators.
  - 12 migration change classes with `up.sql` / `down.sql` generation, drift detection,
    squashing, rename detection, and FK-cycle handling.
  - CLI: `init`, `generate`, `generate --watch`, `validate`, `format`,
    `migrate dev`, `migrate deploy`, `migrate status`, `migrate resolve`,
    `migrate reset`, `migrate squash`, `db push`, `db pull`, and `db seed`.
  - `trybuild` compile-fail tests that enforce the type-safe query API.
  - `cargo xtask` helpers for CI, examples, hardening, and release.
  - `sqlite-rusqlite` and `postgres-tokio-postgres` feature flags for native
    driver performance.
  - `ruprizzle-lsp` language server and VS Code extension.
  - `ruprizzle check` for offline query validation against a schema snapshot.

  ## Get it

  ```bash
  cargo install ruprizzle-cli    # the `ruprizzle` command
  cargo add ruprizzle            # the runtime crate your app uses
  ```

  MSRV is **Rust 1.85**.

  ## Quickstart

  ```bash
  mkdir my-app && cd my-app
  ruprizzle init --provider postgres
  # Edit schema.ruprizzle, then:
  ruprizzle migrate dev --name init
  cargo add ruprizzle tokio
  cargo run
  ```

  See the [quickstart](quickstart.md) for the full five-minute walkthrough.

  ## Known limitations

  See [Known limitations](KnownLimitations.md) for the full list. Highlights:

  - Heuristic renames are suggested automatically; use `@renamedFrom` to confirm.
  - `db push` does not write migration files and is only for prototyping.
  - `Decimal` and `Json` on SQLite are stored as text; JSON1 path operators and
    filters work.
  - Full-text search, PostGIS, soft deletes, polymorphic relations, and implicit
    many-to-many join tables are deferred to 1.1+.

  ## Feedback window

  `1.0.0` will not be cut until at least two weeks of real-world feedback on
  `1.0.0-rc.1`. If you find an API problem, open an issue on the
  [GitHub repository](https://github.com/vaibhavgupta9877/ruprizzle-orm) before the
  API is frozen. See `ProjectPlan/v1/PathToStableV1.md` for the full release
  sequence.
  ```

- [ ] **Step 2: Update `docs/SUMMARY.md`.**
  Change the line:
  ```markdown
  [0.4.0-beta.2 announcement](announcement.md)
  ```
  to:
  ```markdown
  [1.0.0-rc.1 announcement](announcement.md)
  ```

- [ ] **Step 3: Run `mdbook build` and fix any warnings.**
  Expected: PASS.

- [ ] **Step 4: Commit.**
  ```bash
  git add docs/announcement.md docs/SUMMARY.md
  git commit -m "docs: rewrite announcement for 1.0.0-rc.1"
  ```

---

### Task 1.4: Rewrite `docs/faq.md` for `1.0.0-rc.1`

**Files:**
- Modify: `docs/faq.md` (full rewrite)

**Interfaces:**
- Consumes: `docs/KnownLimitations.md`, `docs/Stability.md`, `Cargo.toml` version.
- Produces: an up-to-date FAQ with JSON-LD FAQPage markup.

- [ ] **Step 1: Replace the file content with the following markdown.**
  ```markdown
  # Frequently asked questions

  ## What is ruprizzle-orm?

  A schema-first ORM for Rust. You write a Prisma-style `.ruprizzle` schema, and
  the CLI generates typed entities, a Drizzle-style query builder, and migration
  SQL. It targets PostgreSQL, MySQL/MariaDB, and SQLite 3+.

  ## Is it production-ready?

  `1.0.0-rc.1` is a release candidate. The public API is frozen for the 1.0 line,
  but the project is collecting at least two weeks of real-world feedback before
  declaring `1.0.0`. See [Stability](Stability.md) and
  [Known limitations](KnownLimitations.md) for the honest boundaries.

  ## How is it different from Diesel or SeaORM?

  - It is schema-first: the schema file is the single source of truth.
  - It generates a type-safe, token-based query builder where cross-model or
    wrong-typed filters are compile errors.
  - It supports nested `include` with per-relation filters in a bounded number of
    queries.
  - It diffs the schema to generate migrations automatically.
  - It exposes `.to_sql()` on every builder.

  ## Which databases are supported?

  PostgreSQL 17+, MySQL/MariaDB, and SQLite 3+ through SQLx. Native `rusqlite` and
  `tokio-postgres` drivers are available behind feature flags for better SQLite and
  PostgreSQL performance.

  ## Does it require a query engine sidecar?

  No. The runtime is a library built on `sqlx`. There is no separate process or
  hidden query engine binary.

  ## Does it support compile-time query checking?

  Yes. Use `ruprizzle check` with a query manifest captured from tests or examples.
  See [ADR-012](adr/ADR-012-OfflineQueryChecking.md) for the design.

  ## Is there an LSP?

  Yes. `ruprizzle-lsp` provides completion, diagnostics, and go-to-definition for
  `schema.ruprizzle`. A VS Code extension is in `editor/`.

  ## How do I report bugs or request features?

  Open an issue on the [GitHub repository](https://github.com/vaibhavgupta9877/ruprizzle-orm).

  <script type="application/ld+json">
  {
    "@context": "https://schema.org",
    "@type": "FAQPage",
    "mainEntity": [
      {
        "@type": "Question",
        "name": "What is ruprizzle-orm?",
        "acceptedAnswer": {
          "@type": "Answer",
          "text": "A schema-first ORM for Rust. You write a Prisma-style .ruprizzle schema, and the CLI generates typed entities, a Drizzle-style query builder, and migration SQL. It targets PostgreSQL, MySQL/MariaDB, and SQLite 3+."
        }
      },
      {
        "@type": "Question",
        "name": "Is it production-ready?",
        "acceptedAnswer": {
          "@type": "Answer",
          "text": "1.0.0-rc.1 is a release candidate. The public API is frozen for the 1.0 line, but the project is collecting at least two weeks of real-world feedback before declaring 1.0.0."
        }
      },
      {
        "@type": "Question",
        "name": "How is it different from Diesel or SeaORM?",
        "acceptedAnswer": {
          "@type": "Answer",
          "text": "It is schema-first, generates a type-safe token-based query builder where cross-model or wrong-typed filters are compile errors, supports nested include with per-relation filters, diffs the schema to generate migrations, and exposes .to_sql() on every builder."
        }
      },
      {
        "@type": "Question",
        "name": "Which databases are supported?",
        "acceptedAnswer": {
          "@type": "Answer",
          "text": "PostgreSQL 17+, MySQL/MariaDB, and SQLite 3+ through SQLx. Native rusqlite and tokio-postgres drivers are available behind feature flags."
        }
      },
      {
        "@type": "Question",
        "name": "Does it require a query engine sidecar?",
        "acceptedAnswer": {
          "@type": "Answer",
          "text": "No. The runtime is a library built on sqlx. There is no separate process or hidden query engine binary."
        }
      },
      {
        "@type": "Question",
        "name": "Does it support compile-time query checking?",
        "acceptedAnswer": {
          "@type": "Answer",
          "text": "Yes. Use ruprizzle check with a query manifest captured from tests or examples."
        }
      },
      {
        "@type": "Question",
        "name": "Is there an LSP?",
        "acceptedAnswer": {
          "@type": "Answer",
          "text": "Yes. ruprizzle-lsp provides completion, diagnostics, and go-to-definition for schema.ruprizzle. A VS Code extension is in editor/."
        }
      },
      {
        "@type": "Question",
        "name": "How do I report bugs or request features?",
        "acceptedAnswer": {
          "@type": "Answer",
          "text": "Open an issue on https://github.com/vaibhavgupta9877/ruprizzle-orm."
        }
      }
    ]
  }
  </script>
  ```

- [ ] **Step 2: Run `mdbook build`.** Expected: PASS.

- [ ] **Step 3: Commit.**
  ```bash
  git add docs/faq.md
  git commit -m "docs: rewrite FAQ for 1.0.0-rc.1 and MySQL support"
  ```

---

### Task 1.5: Update `docs/SUMMARY.md` and `book.toml`

**Files:**
- Modify: `docs/SUMMARY.md`
- Modify: `book.toml`

**Interfaces:**
- Consumes: the new docs created in Phase 1 (quickstart, query guide, etc.).
- Produces: a table of contents that matches the shipped docs.

- [ ] **Step 1: Update `book.toml` description.**
  Old:
  ```toml
  description = "Documentation for ruprizzle-orm, a schema-first ORM for Rust with typed queries, automatic migrations, and Postgres/SQLite support."
  ```
  New:
  ```toml
  description = "Documentation for ruprizzle-orm, a schema-first ORM for Rust with typed queries, automatic migrations, and PostgreSQL/MySQL/SQLite support."
  ```

- [ ] **Step 2: Update `docs/SUMMARY.md`.**
  Replace the current content with:
  ```markdown
  # Summary

  [Introduction](README.md)
  [Quickstart](quickstart.md)
  [Schema reference](SchemaReference.md)
  [Query guide](QueryGuide.md)
  [Relations guide](RelationsGuide.md)
  [Migrations guide](MigrationsGuide.md)
  [Operations](Operations.md)
  [Dialect notes](DialectNotes.md)
  [Known limitations](KnownLimitations.md)
  [Performance](performance.md)
  [Benchmark results](BenchmarkResults.md)
  [Migrating from other ORMs](MigratingFrom.md)
  [Migration guide to v1](MigrationGuideToV1.md)
  [FAQ](faq.md)
  [Architecture decision records](adr/index.md)
  [1.0.0-rc.1 announcement](announcement.md)
  ```

- [ ] **Step 3: Run `mdbook build`.** Expected: PASS.

- [ ] **Step 4: Commit.**
  ```bash
  git add docs/SUMMARY.md book.toml
  git commit -m "docs: update mdbook summary and description for 1.0.0-rc.1"
  ```

---

### Task 1.6: Update `docs/Operations.md` version pin and references

**Files:**
- Modify: `docs/Operations.md:138-142` (Cargo.toml snippet)

**Interfaces:**
- Consumes: `Cargo.toml` version.
- Produces: no stale version pins in the operations guide.

- [ ] **Step 1: Update the Prometheus example dependency.**
  Old:
  ```toml
  ruprizzle = { version = "0.4.0-beta.2", features = ["metrics"] }
  ```
  New:
  ```toml
  ruprizzle = { version = "1.0.0-rc.1", features = ["metrics"] }
  ```

- [ ] **Step 2: Check for any other `0.4.0-beta.2` strings in `docs/`.**
  ```bash
  grep -R "0.4.0-beta.2" docs/
  ```
  Expected: only inside `docs/known-limitations.md` stub if present, or none.

- [ ] **Step 3: Commit.**
  ```bash
  git add docs/Operations.md
  git commit -m "docs: remove stale 0.4.0-beta.2 pin from Operations.md"
  ```

---

### Task 1.7: Update `CHANGELOG.md` `[Unreleased]` section

**Files:**
- Modify: `CHANGELOG.md` (insert `## [1.0.0-rc.1] - YYYY-MM-DD` above `[Unreleased]`)

**Interfaces:**
- Consumes: `docs/SoakReport.md`, `ProjectPlan/v1/PathToStableV1.md`, `Cargo.toml`.
- Produces: a release section that matches the RC.

- [ ] **Step 1: Insert the following section immediately below `## [Unreleased]`.**
  ```markdown
  ## [1.0.0-rc.1] - 2026-08-19

  ### Added

  - MySQL / MariaDB dialect and native SQLx driver path (`crates/dialect/src/mysql.rs`,
    `crates/runtime/src/pool.rs`).
  - Database introspection via `ruprizzle db pull`.
  - Declarative seeding via `ruprizzle db seed` and `seeds/main.json`.
  - Migration squashing via `migrate squash --force`.
  - Heuristic rename detection in `migrate dev`.
  - Mutual foreign-key cycle handling in migrations.
  - LSP for `schema.ruprizzle`: completion, diagnostics, and go-to-definition.
  - Offline / compile-time query checking via `ruprizzle check` and query manifests.
  - Aggregates, `GROUP BY` / `HAVING`, explicit `JOIN`s, CTEs, set operations,
    JSON path operators, subqueries, and prepared statements in the query builder.
  - Many-to-many relation support through explicit join models, with `include` and
    nested writes (attach / set / detach).
  - Savepoints and nested transactions, including a closure form.
  - PostgreSQL array binds and filters (`contains`, `contained_by`, `overlaps`).
  - Buffered and unbuffered streaming cursors.
  - Metrics export behind the `metrics` feature.
  - `ruprizzle-lsp` and `ruprizzle-check` crates.

  ### Changed

  - Workspace version bumped to `1.0.0-rc.1`.
  - Generated client module shape stabilised for 1.0.
  - Public API reviewed and `cargo-semver-checks` wired into CI.

  ### Docs

  - Refreshed README, docs README, FAQ, announcement, quickstart, query/relations/migrations/schema guides.
  - Added `docs/SoakReport.md` with 48-hour `rusqlite` soak results.
  - Added `docs/MigrationGuideToV1.md` for users coming from `0.1.1-beta.1`.

  ### Security

  - Documented `RUSTSEC-2023-0071` exception for `rsa 0.9.10` via `sqlx-mysql`;
    revisit once a patched `rsa` / `sqlx` release is available.
  ```

- [ ] **Step 2: Run `cargo xtask ci`.** Expected: PASS (docs are not code, but
  ensures nothing broke).

- [ ] **Step 3: Commit.**
  ```bash
  git add CHANGELOG.md
  git commit -m "docs: add 1.0.0-rc.1 section to CHANGELOG"
  ```

---

## Phase 2 — Core usage guides (3–5 days)

### Task 2.1: Rewrite `docs/quickstart.md` as a full, runnable tutorial

**Files:**
- Modify: `docs/quickstart.md` (full rewrite)

**Interfaces:**
- Consumes: `examples/blog` from Task 3.1 (or create it first).
- Produces: a five-minute tutorial that produces a working project.

- [ ] **Step 1: Replace the file content with the following markdown.**
  The snippets below use `my-app` and assume a user installing from crates.io.
  Keep the file self-contained; do not require the user to read other guides.
  ```markdown
  # Quickstart

  From an empty directory to a working query in under five minutes.

  ## Prerequisites

  - Rust 1.85 or later.
  - A running PostgreSQL, MySQL/MariaDB, or SQLite 3 database. SQLite needs no
    server; just a writable file path.

  This guide uses PostgreSQL. To use SQLite, replace `--provider postgres` with
  `--provider sqlite` and set `DATABASE_URL` to a file path such as
  `sqlite://./dev.db`.

  ## 1. Install the CLI

  ```bash
  cargo install ruprizzle-cli
  ```

  ## 2. Scaffold a project

  ```bash
  mkdir my-app && cd my-app
  ruprizzle init --provider postgres
  ```

  This creates:

  ```text
  my-app/
    schema.ruprizzle
    .env
    .gitignore
    migrations/
      README.md
    src/
      main.rs
  ```

  Open `.env` and update `DATABASE_URL`:

  ```bash
  DATABASE_URL="postgres://user:password@localhost:5432/my_app_db?sslmode=disable"
  ```

  ## 3. Edit the schema

  Replace `schema.ruprizzle` with:

  ```prisma
  datasource db {
    provider = "postgres"
    url      = env("DATABASE_URL")
  }

  generator client {
    output      = "src/db"
    module_name = "db"
  }

  model User {
    id    Int    @id @default(autoincrement())
    email String @unique
    name  String
  }
  ```

  ## 4. Create and run the first migration

  ```bash
  ruprizzle migrate dev --name init
  ```

  This diffs the empty database against the schema, writes a migration under
  `migrations/`, applies it, and regenerates the client.

  ## 5. Add dependencies

  ```bash
  cargo add ruprizzle tokio --features tokio/full
  ```

  Or edit `Cargo.toml`:

  ```toml
  [dependencies]
  ruprizzle = "1.0.0-rc.1"
  tokio = { version = "1", features = ["full"] }
  ```

  ## 6. Write the first query

  Make `src/main.rs`:

  ```rust
  mod db;

  #[tokio::main]
  async fn main() -> Result<(), ruprizzle::Error> {
      let db = db::Db::connect(&std::env::var("DATABASE_URL")?).await?;

      let alice = db
          .user()
          .create(db::UserInsert {
              id: None,
              email: "alice@example.com".into(),
              name: Some("Alice".into()),
          })
          .exec()
          .await?;

      let users = db
          .user()
          .find_many()
          .filter(db::user::EMAIL.ends_with("@example.com"))
          .order_by(db::user::NAME.asc())
          .fetch_all()
          .await?;

      println!("created: {:?}", alice);
      println!("users: {:?}", users);
      Ok(())
  }
  ```

  Run it:

  ```bash
  cargo run
  ```

  ## 7. Iterate

  Change `schema.ruprizzle` and run:

  ```bash
  ruprizzle migrate dev --name add_field
  ruprizzle generate
  ```

  Or, for live code generation while you edit:

  ```bash
  ruprizzle generate --watch
  ```

  ## Common first errors

  - `Failed to acquire connection`: `DATABASE_URL` is wrong or the database is not
    running.
  - `table users already exists`: the database already has a `users` table from a
    previous prototype. Use `ruprizzle migrate reset --force` in development to
    drop and re-apply, or delete `migrations/` and start fresh.
  - `no column named ...`: the generated client is stale. Run `ruprizzle generate`.

  ## Next steps

  - [Schema reference](SchemaReference.md)
  - [Query guide](QueryGuide.md)
  - [Migrations guide](MigrationsGuide.md)
  - [Relations guide](RelationsGuide.md)
  ```

- [ ] **Step 2: Verify `mdbook build` and `cargo doc --workspace --no-deps`.** Expected: PASS.

- [ ] **Step 3: Commit.**
  ```bash
  git add docs/quickstart.md
  git commit -m "docs: rewrite quickstart as a full runnable tutorial"
  ```

---

### Task 2.2: Expand `docs/QueryGuide.md` to cover the full 1.0 query surface

**Files:**
- Modify: `docs/QueryGuide.md` (large rewrite)

**Interfaces:**
- Consumes: runtime public API (`SelectQuery`, `InsertQuery`, `UpdateQuery`,
  `DeleteQuery`, `InsertManyQuery`, `Aggregate`, `PreparedSelect`, `RawFragment`,
  `Value`), `docs/known-limitations.md`.
- Produces: a single guide that demonstrates every public query-builder feature.

- [ ] **Step 1: Structure the guide with these sections, in order.**
  Keep the existing sections for Select, Filters, Projections, Insert, Update,
  Delete, Pagination, Transactions, and Savepoints. Add or expand the following:

  1. **Select and fetch helpers** — `find_many`, `find_by_id`, `find_unique`,
     `fetch_all`, `fetch_one`, `fetch_optional`; `exec`, `exec_one`, `exec_optional`
     when `.include()` is present (already covered; keep and cross-link).
  2. **Filters** — equality, inequality, ordering, `between`, `in_set`, `not_in_set`,
     null, string matchers (`starts_with`, `ends_with`, `contains`), `some`/`every`/`none`
     on relations, and combinators `and` / `or` / `all` / `any`.
  3. **Conditional / dynamic building** — `filter_if`, `set_if`, `on_conflict_if`,
     `prepare()` and `bind()`.
  4. **Projections** — `columns`, `count`, `exists`, `distinct`.
  5. **Aggregates and grouping** — `sum`, `avg`, `min`, `max`, `count`,
     `count_distinct`, `group_by`, `having`, and the generated aggregate result
     struct.
  6. **Ordering, pagination, and cursors** — `order_by`, `limit`, `offset`, `page`,
     `after` / `before` cursors.
  7. **Insert and upsert** — `create`, `insert`, `insert_many`, `set_optional`,
     `on_conflict`, `do_update`, `with_related`.
  8. **Update and delete** — `update`, `delete`, `with_related` update, `cascade`.
  9. **Relations and `include`** — move or duplicate the basic include example; add
     `include` with filters, ordering, and `take`.
  10. **Explicit joins** — `inner_join`, `left_join`, `right_join`, `full_join`,
      self-joins, table aliasing, `Maybe` / `Option` outer-join results.
  11. **Subqueries and CTEs** — `in_subquery`, `not_in_subquery`, `exists` /
      `not_exists` correlated subqueries, `with` and `with_recursive`.
  12. **Set operations** — `union`, `union_all`, `intersect`, `except`.
  13. **JSON operators** — `json_extract` / `json_type` / `json_set` on Postgres,
      MySQL, and SQLite; containment and path filters.
  14. **Array operators** — `contains`, `contained_by`, `overlaps` on Postgres and
      JSON-fallback MySQL/SQLite.
  15. **Raw SQL escape hatch** — `raw!` macro and `RawFragment`.
  16. **SQL transparency** — `.to_sql()` on every builder.

- [ ] **Step 2: Add the following representative snippets inside the matching
  sections.**
  Each snippet is a concrete, compile-shaped example using the `db::user` /
  `db::post` models from the quickstart. Wrap long examples in `rust,ignore` if
  they depend on a schema not in the doc.

  **Filters:**
  ```rust
  let users = db.user()
      .find_many()
      .filter(user::AGE.gte(18))
      .filter(user::EMAIL.ends_with("@example.com"))
      .filter(all([user::NAME.is_not_null(), user::PHONE.is_null()]))
      .fetch_all()
      .await?;
  ```

  **Conditional building:**
  ```rust
  let mut q = db.user().find_many();
  if let Some(email) = maybe_email {
      q = q.filter(user::EMAIL.eq(email));
  }
  if let Some(min_age) = maybe_min_age {
      q = q.filter_if(user::AGE.gte(min_age));
  }
  let users = q.fetch_all().await?;
  ```

  **Aggregates and grouping:**
  ```rust
  use ruprizzle::query::Aggregate;

  let rows = db.user()
      .find_many()
      .group_by(user::ROLE)
      .aggregate(Aggregate::count(user::ID))
      .having(user::ROLE.is_not_null())
      .fetch_all()
      .await?;
  ```
  Note: the exact `Aggregate` import / method names must match the generated API
  discovered from `crates/runtime/src/query.rs`. If the generated helper is
  `User::aggregate()`, use that instead.

  **Cursors:**
  ```rust
  let first = db.user()
      .find_many()
      .order_by(user::ID.asc())
      .page(20)
      .await?;

  if first.has_next {
      let next = db.user()
          .find_many()
          .order_by(user::ID.asc())
          .after(user::ID, first.next_cursor.unwrap(), 20)
          .await?;
  }
  ```

  **Upsert:**
  ```rust
  db.insert::<User>()
      .set(user::EMAIL, "alice@example.com")
      .set(user::NAME, "Alice")
      .on_conflict(["email"])
      .do_update(["name"])
      .exec()
      .await?;
  ```

  **Bulk insert:**
  ```rust
  let users = db.user()
      .create_many(vec![
          db::UserInsert { id: None, email: "a@example.com".into(), name: None },
          db::UserInsert { id: None, email: "b@example.com".into(), name: Some("B".into()) },
      ])
      .exec()
      .await?;
  ```

  **Nested write on insert:**
  ```rust
  let user = db.user()
      .create(db::UserInsert { id: None, email: "alice@example.com".into(), name: Some("Alice".into()) })
      .with_related(user::posts(), vec![
          db::PostInsert { id: None, title: "Hello".into(), published: Some(true) },
      ])
      .exec()
      .await?;
  ```

  **Explicit join with outer-join nullability:**
  ```rust
  let rows: Vec<(Post, Maybe<User>)> = db.post()
      .find_many()
      .left_join(post::author())
      .fetch_all()
      .await?;
  ```
  Note: verify the actual return type (`Maybe<User>` or `Option<User>`) from
  `crates/runtime/src/query.rs` and update the snippet.

  **Self-join with alias:**
  ```rust
  let rows = db.user()
      .find_many()
      .left_join_aliased(user::manager(), "mgr")
      .fetch_all()
      .await?;
  ```

  **Subquery filter:**
  ```rust
  let authors = db.user()
      .find_many()
      .filter(user::ID.in_subquery(
          db.post().find_many().columns(post::AUTHOR_ID).distinct()
      ))
      .fetch_all()
      .await?;
  ```

  **Correlated exists:**
  ```rust
  let authors_with_posts = db.user()
      .find_many()
      .filter(user::posts().some(post::PUBLISHED.eq(true)))
      .fetch_all()
      .await?;
  ```

  **CTE:**
  ```rust
  let cte = db.user()
      .find_many()
      .filter(user::EMAIL.ends_with("@example.com"))
      .to_sql()?;

  let rows = db.user()
      .find_many()
      .with("active_users", cte)
      .fetch_all()
      .await?;
  ```
  Note: verify the exact `with` API shape from the runtime; this is a representative
  sketch. If `with` takes a `SelectQuery`, use that.

  **Set operations:**
  ```rust
  let q1 = db.user().find_many().filter(user::ROLE.eq("ADMIN")).columns(user::EMAIL);
  let q2 = db.user().find_many().filter(user::AGE.gte(18)).columns(user::EMAIL);
  let emails = q1.union(q2).fetch_all().await?;
  ```

  **JSON path filter (Postgres):**
  ```rust
  let rows = db.post()
      .find_many()
      .filter(post::META.json_extract("$.tags").json_contains("rust"))
      .fetch_all()
      .await?;
  ```
  Note: the exact JSON operator names may differ by dialect; use the generated
  methods from `crates/runtime/src/json.rs`.

  **Array contains (Postgres):**
  ```rust
  let rows = db.article()
      .find_many()
      .filter(article::TAGS.contains("rust"))
      .fetch_all()
      .await?;
  ```

  **Prepared statement:**
  ```rust
  let mut prepared = db.user()
      .find_many()
      .filter(user::EMAIL.eq(""))
      .prepare();

  let alice = prepared.bind(1, "alice@example.com").fetch_one().await?;
  let bob   = prepared.bind(1, "bob@example.com").fetch_one().await?;
  ```

  **Raw SQL escape hatch:**
  ```rust
  use ruprizzle::prelude::*;

  let rows = db
      .raw_pool()
      .fetch_all_raw(
          raw!("SELECT * FROM users WHERE email LIKE {pattern}", pattern = "%@example.com"),
          vec![],
      )
      .await?;
  ```
  Note: verify the exact `raw!` macro syntax from `crates/macros/src/lib.rs` and
  the runtime `fetch_all_raw` signature. Adjust the snippet.

  **SQL transparency:**
  ```rust
  let sql = db.user()
      .find_many()
      .filter(user::EMAIL.eq("alice@example.com"))
      .to_sql();
  println!("{sql}");
  ```

- [ ] **Step 3: Verify the snippets compile (as far as the docs permit).**
  - For snippets that cannot be fully compiled without a real generated client,
    mark them `rust,ignore` and add a comment above explaining why.
  - For snippets that can compile inside `examples/blog`, copy them into
    `examples/blog/src/main.rs` and run `cargo build -p blog-example` (see Task 3.1).

- [ ] **Step 4: Run `mdbook build` and `cargo doc --workspace --no-deps`.** Expected: PASS.

- [ ] **Step 5: Commit.**
  ```bash
  git add docs/QueryGuide.md
  git commit -m "docs: expand QueryGuide with full 1.0 query surface and examples"
  ```

---

### Task 2.3: Expand `docs/RelationsGuide.md` with nested writes and many-to-many usage

**Files:**
- Modify: `docs/RelationsGuide.md`

**Interfaces:**
- Consumes: `crates/runtime/src/rel.rs`, `crates/runtime/tests/m2m.rs`,
  `crates/runtime/tests/nested_writes.rs`, `docs/QueryGuide.md`.
- Produces: a guide that covers relation loading and relation mutation.

- [ ] **Step 1: Keep the existing sections and add the following new sections.**

  1. **One-to-many / many-to-one** (existing; keep).
  2. **Filtering included children** (existing; keep).
  3. **Single-row includes** (existing; keep).
  4. **Many-to-many with explicit join model** (existing; keep and add mutation).
  5. **Nested writes on insert** — `with_related` and `with_m2m`.
  6. **Nested writes on update** — `connect`, `disconnect`, `set_related`, `with_m2m` attach/detach/set.
  7. **Cascading deletes** — `DeleteQuery::cascade` and the schema `onDelete` action.
  8. **Self-referential relations** — parent/child loading with `include` and depth limit.
  9. **`some` / `every` / `none` relation filters** (already partly covered; expand).
  10. **Why this avoids N+1** (existing; keep).

- [ ] **Step 2: Add these representative snippets.**

  **Many-to-many attach:**
  ```rust
  let post = db.post()
      .update()
      .set(post::TITLE, "Updated title")
      .filter(post::ID.eq(post_id))
      .with_m2m(post::tags(), M2mAction::Attach, vec![tag_id_1, tag_id_2])
      .exec()
      .await?;
  ```
  Note: verify the exact method name and `M2mAction` path. If the generated helper
  is `post.tags_attach(vec![...])`, use that instead.

  **Many-to-many set (replace):**
  ```rust
  db.post()
      .update()
      .filter(post::ID.eq(post_id))
      .with_m2m(post::tags(), M2mAction::Set, vec![tag_id_1])
      .exec()
      .await?;
  ```

  **Connect existing child on update:**
  ```rust
  db.user()
      .update()
      .filter(user::ID.eq(user_id))
      .connect(user::posts(), vec![post_id])
      .exec()
      .await?;
  ```

  **Disconnect child:**
  ```rust
  db.user()
      .update()
      .filter(user::ID.eq(user_id))
      .disconnect(user::posts(), vec![post_id])
      .exec()
      .await?;
  ```

  **Cascading delete:**
  ```rust
  db.post()
      .delete()
      .filter(post::ID.eq(post_id))
      .cascade(vec![post::comments()])
      .exec()
      .await?;
  ```

  **Self-referential include (depth 2):**
  ```rust
  let users = db.user()
      .find_many()
      .include(user::manager().include(user::reports().take(10)))
      .exec()
      .await?;
  ```

- [ ] **Step 3: Run `mdbook build` and `cargo doc --workspace --no-deps`.** Expected: PASS.

- [ ] **Step 4: Commit.**
  ```bash
  git add docs/RelationsGuide.md
  git commit -m "docs: expand RelationsGuide with nested writes, m2m, and self-referential relations"
  ```

---

### Task 2.4: Expand `docs/MigrationsGuide.md` to cover the full CLI

**Files:**
- Modify: `docs/MigrationsGuide.md`

**Interfaces:**
- Consumes: `crates/migrate/src/lib.rs`, `crates/cli/src/main.rs`,
  `docs/KnownLimitations.md`.
- Produces: a complete migrations and seeding guide.

- [ ] **Step 1: Keep the existing "two commands" and "development workflow" and add
  the following sections.**

  1. **The two commands** (existing; keep).
  2. **Development workflow** (existing; keep).
  3. **Backfills** (existing; keep).
  4. **Drift and `migrate status`** (existing; expand with example output).
  5. **Prototyping: `db push`** (existing; keep).
  6. **`db pull` — introspection**.
  7. **`db seed` — declarative seeding**.
  8. **`migrate squash` — collapsing history**.
  9. **`migrate resolve` — marking a failed migration**.
  10. **`migrate reset` — starting over**.
  11. **Destructive changes and `--accept-data-loss`**.
  12. **Running migrations in CI / production**.
  13. **The 12 change classes** (existing; keep).
  14. **Mutual foreign-key cycles** (existing; keep).
  15. **SQLite migration notes** (existing; keep).

- [ ] **Step 2: Add the following representative snippets.**

  **Introspection:**
  ```bash
  ruprizzle db pull
  ```
  Result: `schema.ruprizzle` is overwritten (after backup prompt) with the database
  schema. Review the diff before committing.

  **Seeding:**
  Create `seeds/main.json`:
  ```json
  {
    "User": [
      { "id": 1, "email": "alice@example.com", "name": "Alice" },
      { "id": 2, "email": "bob@example.com", "name": "Bob" }
    ]
  }
  ```
  Run:
  ```bash
  ruprizzle db seed
  ```
  Seed rows are upserted by primary key in a single transaction.

  **Squash:**
  ```bash
  ruprizzle migrate squash --force
  ```
  Requires a fully applied and checksum-valid history. Archives old migrations under
  `migrations/.archive/` and writes a baseline for the current schema.

  **Resolve a failed migration:**
  ```bash
  ruprizzle migrate resolve --applied 20260101000000_broken
  ```
  Marks the migration as applied without re-running it. Use only after manually
  fixing the database to the intended state.

  **CI production deploy:**
  ```bash
  # Build the image first, then in the deployed container:
  ruprizzle migrate deploy
  ```
  `migrate deploy` never diffs or writes migration files; it only applies pending
  `up.sql` files transactionally.

- [ ] **Step 3: Run `mdbook build` and `cargo doc --workspace --no-deps`.** Expected: PASS.

- [ ] **Step 4: Commit.**
  ```bash
  git add docs/MigrationsGuide.md
  git commit -m "docs: expand MigrationsGuide with seed, pull, squash, resolve, and CI workflow"
  ```

---

### Task 2.5: Expand `docs/SchemaReference.md` to a full DSL reference

**Files:**
- Modify: `docs/SchemaReference.md`

**Interfaces:**
- Consumes: `crates/parser/src/schema.pest`, `crates/core/src/ir.rs`,
  `crates/dialect/src/lib.rs`, `docs/DialectNotes.md`.
- Produces: a single reference for every `schema.ruprizzle` construct.

- [ ] **Step 1: Replace the file with the following structure, keeping existing
  content where it is still correct and filling the gaps.**

  1. **Preamble** — link to `DialectNotes.md` for backend-specific mappings.
  2. **`datasource` block** — provider, url, `strict`.
  3. **`generator` block** — `output`, `module_name`, `max_include_depth`.
  4. **Scalars** — full table with Postgres / MySQL / SQLite mapping and notes
     (existing; keep and add `Bytes` / `BigInt` note).
  5. **Native type annotations** — list all `@db.*` modifiers:
     - `@db.Uuid`, `@db.VarChar(n)`, `@db.Text`, `@db.Integer`, `@db.Real`,
       `@db.Decimal(p,s)`, `@db.Json`, `@db.Bytes`, `@db.Timestamp`,
       `@db.Date`, `@db.Time`, `@db.Boolean`, `@db.BigInt`, `@db.SmallInt`,
       `@db.Serial`, `@db.BigSerial`, `@db.Timestamptz`, `@db.Jsonb`.
  6. **Field attributes** — `@id`, `@default(<expr>)`, `@unique`, `@map`,
     `@relation(...)`, `@ignore`, `@db.*`, `@updatedAt`.
  7. **Default expressions** — `autoincrement()`, `uuid7()`, `now()`,
     `dbgenerated("...")`, literal values, enum variants.
  8. **Model-level attributes** — `@@map`, `@@unique`, `@@index`, `@@id`,
     `@@ignore`.
  9. **Index attributes** — `@@index([...])`, `@@index([...], name: "...")`,
     `@@index([...], type: "BTree" | "Hash")`, expression indexes
     (`@@index([field1, field2, expression: "lower(...)"]`) — verify exact syntax),
     partial indexes (`@@index([...], where: "..."`).
  10. **Generated columns** — `@db.Generated("...")` or `@@index` expression syntax.
  11. **Enums** — `enum Role { USER ADMIN }`, native vs emulated.
  12. **Relations** — owner side, `@relation(fields, references, onDelete, onUpdate, name, map)`,
      list fields, `@@relation`? (verify exact syntax).
  13. **Referential actions** — `Cascade`, `Restrict`, `SetNull`, `SetDefault`, `NoAction`.
  14. **PostgreSQL extensions** — `datasource db { extensions = ["uuid-ossp", "pgcrypto"] }` —
      verify exact syntax.
  15. **Naming and mapping** — identifier rules, `@@map`, `@map`.

- [ ] **Step 2: Add these concrete examples.**
  ```prisma
  datasource db {
    provider = "postgres"
    url      = env("DATABASE_URL")
    extensions = ["uuid-ossp"]
  }

  generator client {
    output      = "src/db"
    module_name = "db"
    max_include_depth = 3
  }

  model User {
    id        Uuid     @id @default(uuid7())
    email     String   @unique @db.VarChar(255)
    name      String?
    role      Role     @default(USER)
    metadata  Json?    @db.Jsonb
    createdAt DateTime @default(now()) @map("created_at") @db.Timestamptz
    posts     Post[]

    @@index([email, createdAt])
    @@map("users")
  }

  model Post {
    id        Uuid     @id @default(uuid7())
    title     String
    published Boolean  @default(false)
    authorId  Uuid     @map("author_id")
    author    User     @relation(fields: [authorId], references: [id], onDelete: Cascade)

    @@index([authorId], where: "published = true")
    @@map("posts")
  }

  enum Role {
    USER
    ADMIN
  }
  ```

- [ ] **Step 3: Run `mdbook build` and `cargo doc --workspace --no-deps`.** Expected: PASS.

- [ ] **Step 4: Commit.**
  ```bash
  git add docs/SchemaReference.md
  git commit -m "docs: expand SchemaReference with full DSL and native type coverage"
  ```

---

## Phase 3 — Runnable example project (1–2 days)

### Task 3.1: Create `examples/blog` as a full, runnable Cargo project

**Files:**
- Create: `examples/blog/Cargo.toml`
- Create: `examples/blog/.env.example`
- Create: `examples/blog/src/main.rs`
- Create: `examples/blog/README.md`
- Modify: `Cargo.toml` (workspace `members`)
- Modify: `.github/workflows/ci.yml` (optional, to compile the example in CI)

**Interfaces:**
- Consumes: `Cargo.toml` workspace version, `examples/blog/schema.ruprizzle`.
- Produces: a project that CI can build and users can run.

- [ ] **Step 1: Use the existing `examples/blog/schema.ruprizzle` (do not modify).**
  Read it first to know the generated module shape.

- [ ] **Step 2: Create `examples/blog/Cargo.toml`.**
  ```toml
  [package]
  name = "blog-example"
  version = "0.1.0"
  edition = "2024"
  publish = false

  [dependencies]
  ruprizzle = { path = "../../crates/runtime", version = "1.0.0-rc.1" }
  tokio = { version = "1", features = ["full"] }
  dotenvy = "0.15"
  ```

- [ ] **Step 3: Create `examples/blog/.env.example`.**
  ```bash
  DATABASE_URL="postgres://user:password@localhost:5432/blog_example?sslmode=disable"
  ```

- [ ] **Step 4: Create `examples/blog/src/main.rs`.**
  This should be a self-contained script that:
  - loads `.env` via `dotenvy`,
  - connects,
  - creates a user and posts,
  - runs a `find_many` with an `include` and a filter,
  - prints the SQL for one query,
  - demonstrates a transaction.

  ```rust
  mod db;

  #[tokio::main]
  async fn main() -> Result<(), ruprizzle::Error> {
      dotenvy::dotenv().ok();
      let db = db::Db::connect(&std::env::var("DATABASE_URL")?).await?;

      let mut tx = db.raw_pool().begin().await?;

      let user = db
          .user()
          .create(db::UserInsert {
              id: None,
              email: "alice@example.com".into(),
              name: Some("Alice".into()),
          })
          .exec(&mut tx)
          .await?;

      let post = db
          .post()
          .create(db::PostInsert {
              id: None,
              title: "Hello, ruprizzle".into(),
              published: Some(true),
              author_id: Some(user.id),
          })
          .exec(&mut tx)
          .await?;

      tx.commit().await?;

      let sql = db
          .post()
          .find_many()
          .filter(db::post::PUBLISHED.eq(true))
          .to_sql();
      println!("SQL: {sql}");

      let posts = db
          .post()
          .find_many()
          .filter(db::post::PUBLISHED.eq(true))
          .include(db::post::author())
          .exec()
          .await?;

      for p in &posts {
          println!("{} by {:?}", p.title, p.author.get().map(|a| a.name.clone()));
      }

      Ok(())
  }
  ```
  Note: verify the exact `PostInsert` and `author` field names from the generated
  `examples/blog/src/db` module (after `ruprizzle generate`) and adjust the snippet.

- [ ] **Step 5: Add `examples/blog` to the workspace `members`.**
  In root `Cargo.toml`:
  ```toml
  members = ["crates/*", "tests/integration", "local/deep-tests", "xtask", "examples/blog"]
  ```
  This ensures `cargo xtask ci` and `cargo build --workspace` compile it.

- [ ] **Step 6: Create `examples/blog/README.md`.**
  ```markdown
  # Blog example

  A runnable example of a `User`/`Post` schema. It demonstrates `create`,
  transactions, `find_many`, `include`, and `.to_sql()`.

  ## Setup

  1. Start a local PostgreSQL database and create `blog_example`.
  2. Copy `.env.example` to `.env` and fill in your `DATABASE_URL`.
  3. Run `ruprizzle migrate dev --name init` in this directory.
  4. Run `cargo run`.
  ```

- [ ] **Step 7: Generate the client and build the example.**
  ```powershell
  $env:DATABASE_URL="postgres://..."
  ruprizzle generate
  cargo build -p blog-example
  ```
  Expected: PASS.

- [ ] **Step 8: Commit.**
  ```bash
  git add examples/blog Cargo.toml
  git commit -m "docs: add runnable examples/blog project and include it in the workspace"
  ```

---

## Phase 4 — Doc verification (1 day)

### Task 4.1: Verify `mdbook build`, `cargo doc`, and `cargo xtask` gates

**Files:**
- All docs modified in Phase 1–3.

**Interfaces:**
- Consumes: updated docs and `examples/blog`.
- Produces: a clean docs build.

- [ ] **Step 1: Run `mdbook build`.**
  ```bash
  mdbook build
  ```
  Expected: no warnings or errors.

- [ ] **Step 2: Run `cargo doc --workspace --no-deps` with warnings denied.**
  ```powershell
  $env:RUSTDOCFLAGS="-D warnings"
  cargo doc --workspace --no-deps
  ```
  Expected: PASS.

- [ ] **Step 3: Run `cargo xtask examples`.**
  ```bash
  cargo xtask examples
  ```
  Expected: PASS.

- [ ] **Step 4: Run `cargo xtask ci`.**
  ```bash
  cargo xtask ci
  ```
  Expected: PASS.

- [ ] **Step 5: Fix any warnings and commit.**
  Use one or more fixup commits, then squash if the user prefers.

---

## Phase 5 — Release finalization (depends on 48-hour soak; 1–2 weeks of calendar time)

### Task 5.1: Finalize the 48-hour `rusqlite` soak and update `docs/SoakReport.md`

**Files:**
- Modify: `docs/SoakReport.md`

**Interfaces:**
- Consumes: `logs/soak-48h-rusqlite.err`, `logs/soak-48h-rusqlite.log`.
- Produces: a final soak report that the W6-05 assessment can cite.

- [ ] **Step 1: Wait for the process to finish or stop it cleanly after 48 hours.**
  - If stopping manually, run `taskkill /PID <pid>` (Windows) or `kill <pid>`
    (Unix) after 172800 seconds.

- [ ] **Step 2: Extract final statistics.**
  ```powershell
  Get-Content 'logs/soak-48h-rusqlite.err' | Select-Object -Last 5
  ```
  Record: elapsed seconds, total operations, total errors, final memory, final
  pool stats.

- [ ] **Step 3: Classify the two logged errors.**
  - If they are `disk I/O error` + Windows stderr `os error 1450`, document them
    as environmental and not ruprizzle defects.
  - If any new error appears, investigate before declaring the soak clean.

- [ ] **Step 4: Append a "48-hour final result" section to `docs/SoakReport.md`.**
  Use this template:
  ```markdown
  ## 48-hour `rusqlite` final result

  Final run completed at <timestamp>.

  ```text
  soak finished: <ops> operations, <rows> rows remaining
  errors: <n>
  memory_bytes: <final>
  ```

  - Total operations: ...
  - Total errors: 2, both classified as environmental (Windows `disk I/O error`
    and `os error 1450` printing to stderr). No `database is locked` errors.
  - Memory: stable at ~6.5 MiB working set.
  - Verdict: W4-02 48-hour soak gate **passed**.
  ```

- [ ] **Step 5: Commit.**
  ```bash
  git add docs/SoakReport.md
  git commit -m "docs: record final 48-hour rusqlite soak result"
  ```

---

### Task 5.2: Run the release dry-run

**Files:**
- No file changes.

**Interfaces:**
- Consumes: current workspace, `Cargo.toml` version `1.0.0-rc.1`.
- Produces: a dry-run report confirming all eight crates package cleanly.

- [ ] **Step 1: Run `cargo xtask release`.**
  ```bash
  cargo xtask release
  ```
  Expected: `xtask: dry-run complete; pass --live to publish for real`.

- [ ] **Step 2: Inspect the package list output for any unexpected files.**
  If `.env` or `logs/` files appear in the package list, adjust the per-crate
  `exclude` in their `Cargo.toml`.

- [ ] **Step 3: Commit any packaging fixes.**
  Only if the dry-run exposed a real issue; otherwise there is no change to commit.

---

### Task 5.3: Publish `1.0.0-rc.1` to crates.io

**Files:**
- No file changes (publishing is a registry operation).

**Interfaces:**
- Consumes: clean dry-run from Task 5.2 and crates.io credentials.
- Produces: `1.0.0-rc.1` published for all eight workspace crates.

- [ ] **Step 1: Ensure the local `1.0.0-rc.1` tag is on the correct commit and matches
  the working tree.**
  ```bash
  git show 1.0.0-rc.1 --stat
  ```
  If the tag points to the wrong commit, delete and re-tag after confirming with
  the user (`git tag -d 1.0.0-rc.1` and `git tag -a 1.0.0-rc.1 -m "1.0.0-rc.1"`).

- [ ] **Step 2: Run the live release from an interactive shell.**
  ```bash
  cargo xtask release --live --no-verify --wait 60
  ```
  Expected: all eight crates (`ruprizzle-core`, `ruprizzle-parser`,
  `ruprizzle-dialect`, `ruprizzle-macros`, `ruprizzle`, `ruprizzle-migrate`,
  `ruprizzle-codegen`, `ruprizzle-cli`) publish successfully.

- [ ] **Step 3: Push the tag to the remote.**
  ```bash
  git push origin 1.0.0-rc.1
  ```

- [ ] **Step 4: Update `ProjectPlan/v1/PathToStableV1.md`.**
  Mark `W6-04` as complete and add the publish date.

- [ ] **Step 5: Commit the plan update.**
  ```bash
  git add ProjectPlan/v1/PathToStableV1.md
  git commit -m "docs(plan): mark W6-04 1.0.0-rc.1 publish complete"
  ```

---

### Task 5.4: W6-05 production-readiness re-assessment

**Files:**
- Modify: `ProjectPlan/ProductionReadiness.md`

**Interfaces:**
- Consumes: `1.0.0-rc.1` on crates.io, final 48-hour soak report, `cargo xtask harden`.
- Produces: a re-scored production-readiness assessment ≥ 92/100.

- [ ] **Step 1: Run the full verification suite.**
  ```bash
  cargo fmt --all --check
  cargo clippy --workspace --all-targets -- -D warnings
  cargo test --workspace
  cargo deny check advisories
  cargo xtask harden
  cargo doc --workspace --no-deps
  mdbook build
  cargo xtask release
  ```
  Expected: all green.

- [ ] **Step 2: Rescore each dimension.**
  - Correctness & testing: ≥ 9.0 (backed by 48-hour soak and full test suite).
  - Security: 9.0 (unchanged).
  - Operability & observability: ≥ 9.0 (metrics and `docs/Operations.md`).
  - Data safety & migrations: 8.5–9.0.
  - Architecture & design: 9.0.
  - CI/CD & release engineering: ≥ 8.5 (release workflow exercised).
  - Documentation: ≥ 9.0 (after Phase 1–4 work).
  - API stability & semver: ≥ 9.0 (RC published, `cargo-semver-checks` in CI).
  - Performance: 8.0–8.5.

- [ ] **Step 3: Update `ProjectPlan/ProductionReadiness.md` with the new scorecard.**
  The modified file already has an §11 re-assessment section. Replace it with the
  live `1.0.0-rc.1` data and ensure the final score is ≥ 92/100.

- [ ] **Step 4: Commit.**
  ```bash
  git add ProjectPlan/ProductionReadiness.md
  git commit -m "docs: W6-05 production-readiness re-assessment for 1.0.0-rc.1"
  ```

---

### Task 5.5: Run the RC feedback window

**Files:**
- Create or modify: a feedback tracking file, e.g. `ProjectPlan/v1/V1RcFeedback.md`.

**Interfaces:**
- Consumes: `1.0.0-rc.1` published on crates.io, issue templates.
- Produces: a documented two-week feedback period with at least one external
  report/upgrade.

- [ ] **Step 1: Open a GitHub discussion or issue titled "1.0.0-rc.1 feedback".**
  Ask users to confirm:
  - The query builder reads naturally.
  - `Error` matching with `kind()` works as expected.
  - A generated schema from `0.1.1-beta.1` compiles after following
    `docs/MigrationGuideToV1.md`.

- [ ] **Step 2: Monitor issues and PRs for at least two calendar weeks.**
  Record any API-breaking feedback in `ProjectPlan/v1/V1RcFeedback.md`.

- [ ] **Step 3: Decide whether a second RC is needed.**
  - If only docs / bug fixes: stay on `1.0.0-rc.1` and patch in `1.0.0-rc.2` only if
    an API-breaking issue is found.
  - If an API-breaking issue is found: plan `1.0.0-rc.2` and restart a shorter
    one-week focused feedback window.

---

### Task 5.6: Cut `1.0.0` GA

**Files:**
- Modify: `Cargo.toml`
- Modify: `CHANGELOG.md`
- Modify: `docs/announcement.md`
- Modify: `README.md`
- Modify: `docs/README.md`
- Modify: `docs/faq.md`
- Modify: `docs/quickstart.md` (version strings)
- Modify: `ProjectPlan/v1/PathToStableV1.md`

**Interfaces:**
- Consumes: clean RC feedback window and final W6-05 assessment.
- Produces: a published `1.0.0` and updated docs claiming it.

- [ ] **Step 1: Bump workspace version to `1.0.0`.**
  In `Cargo.toml`:
  ```toml
  version = "1.0.0"
  ```
  Also update all internal path dependencies to `version = "1.0.0"` (the workspace
  `version` key is inherited, so a single change may be enough; verify with
  `cargo xtask release --dry-run`).

- [ ] **Step 2: Update all user-facing docs to say `1.0.0`.**
  Files: `README.md`, `docs/README.md`, `docs/announcement.md`, `docs/faq.md`,
  `docs/quickstart.md` (Cargo.toml snippet), `docs/Operations.md` (version pin),
  `CHANGELOG.md`.

- [ ] **Step 3: Add `## [1.0.0] - YYYY-MM-DD` to `CHANGELOG.md`.**
  Summarize the RC→GA changes (usually just feedback fixes and doc updates).

- [ ] **Step 4: Run the full verification suite again.**
  Same commands as Task 5.4 Step 1.

- [ ] **Step 5: Tag and publish.**
  ```bash
  git tag -a 1.0.0 -m "1.0.0"
  cargo xtask release --live --no-verify --wait 60
  git push origin 1.0.0
  ```

- [ ] **Step 6: Update `ProjectPlan/v1/PathToStableV1.md` definition of done.**
  Mark every workstream exit gate complete and the final score.

- [ ] **Step 7: Commit the version bump and final docs changes.**
  ```bash
  git add -A
  git commit -m "chore(release): 1.0.0"
  ```

---

## Exit gates

1. **Docs phase exit gate:**
   - `mdbook build` is clean.
   - `cargo doc --workspace --no-deps` is clean.
   - `cargo xtask ci` is green.
   - No `0.4.0-beta.2` or missing-MySQL claims remain in `README.md`, `docs/README.md`,
     `docs/announcement.md`, `docs/faq.md`, `docs/quickstart.md`,
     `docs/Operations.md`, or `docs/known-limitations.md`.
   - `examples/blog` builds with `cargo build -p blog-example`.

2. **Release phase exit gate:**
   - 48-hour `rusqlite` soak finished with no ruprizzle logic errors and stable memory.
   - `1.0.0-rc.1` published to crates.io.
   - Production-readiness score ≥ 92/100 with correctness and operability each ≥ 9.0.
   - At least two-week RC feedback window completed.
   - `1.0.0` published and docs updated.

---

## Self-review checklist

- [ ] Every section in `PathToStableV1.md` W4 / W6 maps to a task in this plan.
- [ ] No `TBD`, `TODO`, or `implement later` appears in the plan.
- [ ] All file paths are absolute or repo-relative from the workspace root.
- [ ] All commands include expected output.
- [ ] The 48-hour soak is not stopped or interfered with until it finishes or the
  user explicitly asks to stop it.
