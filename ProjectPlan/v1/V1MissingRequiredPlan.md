# V1 Missing/Weak Features — Validated Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: `superpowers:subagent-driven-development`
> (recommended) or `superpowers:executing-plans` to implement this plan task-by-task.
> Steps use checkbox (`- [ ]`) syntax for tracking. Do not batch tasks across
> workstream boundaries; each workstream has its own exit gate.

**Goal:** Validate `ProjectPlan/v1/V1MissingRequired.md` against the current
`0.4.0-beta.2` implementation and deliver a phased, buildable plan for the
features that are still genuinely missing or weak, while correcting stale public
claims in `README.md` and `docs/KnownLimitations.md`.

**Architecture:** Build on the existing parser → core IR → dialect → codegen →
runtime stack. Add new crates only for editor-facing (LSP) and build-time
(offline query checking) features. Keep SQLx as the runtime and keep the parser
in the build path, not the application dependency tree. Respect `ADR-006`
(explicit join models in v1), `ADR-009` (native drivers are escape hatches), and
`PathToStableV1` v1.0 release sequencing.

**Tech Stack:** Rust 2024, SQLx 0.8, tokio, serde, pest, miette, `metrics`,
`tower-lsp` for the language server, `clap` for new CLI subcommands.

---

## Global Constraints

- Branch: `dev-v0-2`; workspace version: `0.4.0-beta.2`; MSRV: Rust 1.85.
- Every task must keep `cargo fmt --all --check`,
  `cargo clippy --workspace --all-targets -- -D warnings`,
  `cargo test --workspace`, `cargo deny check advisories`, and
  `cargo xtask harden` green.
- Public API additions to `ruprizzle` or `ruprizzle-migrate` must be semver-safe
  or gated behind a feature flag.
- Do not replace SQLx or introduce a sidecar query engine.
- Do not silently rename tables/columns in migrations.
- New schema syntax must parse on PostgreSQL, MySQL/MariaDB, and SQLite and
  degrade gracefully with a clear diagnostic when a dialect cannot support it.

---

## 1. Validation of `ProjectPlan/v1/V1MissingRequired.md`

The table below cross-checks every limitation/priority in
`V1MissingRequired.md` against the implementation at HEAD.

| Feature / gap from `V1MissingRequired.md` | Current status | Evidence | Completeness |
|---|---|---|---|
| Compile-time SQL verification / offline query checking | Missing | `README.md:533`, `docs/KnownLimitations.md:12` | 0% |
| LSP for `schema.ruprizzle` | Missing | `README.md:534`, `editor/README.md:40`, only `editor/ruprizzle.tmLanguage.json` | 0% |
| Implicit many-to-many join tables | Partial | `docs/adr/ADR-006-ExplicitJoinModels.md`, `crates/runtime/tests/m2m.rs` | 50% (explicit works) |
| JSON path querying | Complete | `crates/runtime/src/json.rs`, `crates/runtime/src/compile.rs:1023-1221`, `crates/runtime/tests/json.rs` | 100% |
| Full-text search | Missing | `README.md:537` | 0% |
| PostGIS / geospatial | Missing | `README.md:537`, no spatial types in `crates/dialect/src` | 0% |
| Soft deletes | Missing | `README.md:537`, no `@deletedAt`/`softDelete` in parser | 0% |
| Polymorphic relations | Missing | `README.md:537`, no polymorphic/discriminator support in `crates/core/src/ir.rs` | 0% |
| Recursive loading beyond depth 2 / tree loading | Partial | `crates/runtime/src/include.rs`, `crates/core/src/ir.rs:193` `max_include_depth` default 3, no tree helpers | 70% |
| Connection-pool metrics | Complete | `crates/runtime/src/pool.rs:533-589`, `crates/runtime/tests/soak.rs:32`, `docs/Operations.md` | 100% |
| MySQL / MariaDB support | Complete | `crates/dialect/src/mysql.rs`, `.github/workflows/ci.yml:69-141`, `tests/integration/tests/dialect_conformance.rs` | 100% |
| SQLite `Decimal` stored as text | Partial / limitation | `docs/KnownLimitations.md:15-16`, `crates/runtime/src/rusqlite.rs:935-946` | Workarounds via `sqlite-rusqlite` |
| SQLite `Json` stored as text | Partial / limitation | `docs/KnownLimitations.md:17-20`; stored as text but JSON1 operators work; `crates/runtime/src/json.rs` | Queryable via JSON path; storage remains text |
| Benchmark suite | Partial | `local/cross-orm-bench/run_bench.py`, `docs/BenchmarkResults.md` | SQLite only; no PG/MySQL, no percentiles/throughput |
| PostgreSQL arrays | Complete | `crates/runtime/tests/arrays.rs`, `crates/dialect/src/postgres.rs:292` | 100% |
| PostgreSQL JSONB / path / containment | Complete | `crates/runtime/src/json.rs`, `crates/runtime/src/compile.rs:1023-1221` | 100% |
| Native enums | Complete | `crates/dialect/src/postgres.rs:200-215`, `tests/integration/tests/change_classes.rs:528-556` | 100% |
| Partial indexes | Partial | `crates/dialect/src/lib.rs:111` `partial_indexes` flag; no DSL/implementation | 10% |
| Expression indexes | Missing | `crates/core/src/ir.rs:527` `IndexDef` only holds field names | 0% |
| Generated columns | Missing | `crates/dialect/src/common.rs:411-445` `column_spec` has no generated-clause support | 0% |
| PostgreSQL extensions | Missing | No `CREATE EXTENSION` in migration engine | 0% |

### Stale public claims to fix

- `README.md:520` lists **connection-pool metrics** as remaining work; they are
  already implemented in `crates/runtime/src/pool.rs`.
- `README.md:536` says **SQLite `Json` cannot be queried with JSON operators**;
  `crates/runtime/src/json.rs` and `crates/runtime/tests/json.rs` demonstrate
  JSON path filtering/ordering on SQLite via JSON1.
- `README.md:519` says **raw-SQL compile-time verification** is not implemented;
  this is still true, but the wording should match `docs/KnownLimitations.md`.

---

## 2. Release Phasing

| Phase | Target | Contains | Rationale |
|---|---|---|---|
| **v1.0 (RC → GA)** | `1.0.0` | LSP, release process, MySQL/benchmark hardening | Closes the open workstreams in `PathToStableV1` that are prerequisites for the semver promise. |
| **v1.1** | `1.1.0` | Offline query checking, partial/expression indexes, generated columns, Postgres extensions | Adds "see mistakes before runtime" and Postgres DDL parity without changing the query-builder public API. |
| **v1.2 / v2.0** | `1.2.0` or `2.0.0` | Full-text search, PostGIS, soft deletes, polymorphic relations, implicit many-to-many, tree loading | Large surface-area features that need their own ADRs and a longer feedback window; ADR-006 currently forbids implicit join tables in v1. |

---

## Workstream A — Correct stale public-facing claims (pre-requisite)

### Task A.1: Refresh `README.md` and `docs/KnownLimitations.md`

**Files:**
- Modify: `README.md:515-537`
- Modify: `docs/KnownLimitations.md:1-59`

**Interfaces:**
- Consumes: the validation table above.
- Produces: a public status section that matches `docs/KnownLimitations.md`.

- [x] **Step 1: Remove or reword the stale entries**
  - In `README.md:520`, delete "Connection pool metrics" from the main
    remaining-work list.
  - In `README.md:536`, reword the SQLite JSON note to:
    "`Json` on SQLite is stored as TEXT, but JSON1 `json_extract`,
    `json_type`, and `json_set` are supported; containment (`@>`) is
    approximated because JSON1 has no containment operator."
  - In `README.md:519`, keep the compile-time checking entry but align wording
    with `docs/KnownLimitations.md`.

- [x] **Step 2: Update `docs/KnownLimitations.md` to be explicit about v1.0 vs 0.2+**
  - Keep the "Current beta" section.
  - Add a "Deferred to v1.1" section: compile-time query checking.
  - Add a "Deferred to v1.2+" section: full-text search, PostGIS, soft deletes,
    polymorphic relations, implicit many-to-many, recursive tree helpers.

- [x] **Step 3: Verify docs**
  - Run: `cargo xtask ci`
  - Expected: all green.

- [x] **Step 4: Commit**
  - `git add README.md docs/KnownLimitations.md`
  - `git commit -m "docs: align README and KnownLimitations with 0.4.0-beta.2 status"`

---

## Workstream B — LSP for `schema.ruprizzle` (v1.0, W5-07)

### Task B.1: Scaffold the LSP crate and CLI command

**Files:**
- Create: `crates/lsp/Cargo.toml`
- Create: `crates/lsp/src/lib.rs`
- Create: `crates/lsp/src/main.rs` (thin wrapper for `stdio` mode)
- Modify: `Cargo.toml` workspace members
- Modify: `crates/cli/Cargo.toml`
- Modify: `crates/cli/src/main.rs:61-90`

**Interfaces:**
- Consumes: `ruprizzle_parser::parse_with_warnings`, `ruprizzle_core::ir::Schema`.
- Produces: a `LanguageServer` type and a `ruprizzle lsp` subcommand.

- [x] **Step 1: Add `tower-lsp` to the workspace**
  - In `Cargo.toml` `[workspace.dependencies]`, add:
    `tower-lsp = { version = "0.20", default-features = false, features = ["runtime-tokio"] }`
  - This version is older than 7 days and has a stable LSP 3.17 implementation.

- [x] **Step 2: Create `crates/lsp/Cargo.toml`**
  - Set `name = "ruprizzle-lsp"`, `publish = true`, version `0.4.0-beta.2`.
  - Depend on `ruprizzle-parser`, `ruprizzle-core`, `ruprizzle-codegen`,
    `tower-lsp`, `tokio`, `serde_json`, `lsp-types` (re-exported by
    `tower-lsp`).

- [x] **Step 3: Implement `Backend` state**
  - In `crates/lsp/src/lib.rs`, define:
    ```rust
    pub struct Backend {
        pub client: Client,
        pub schema_path: Arc<Mutex<PathBuf>>,
    }
    ```
  - Implement `tower_lsp::LanguageServer` with `initialize`,
    `initialized`, `shutdown`, `did_open`, `did_change`, `did_close`.

- [x] **Step 4: Wire the CLI**
  - In `crates/cli/src/main.rs`, add `Command::Lsp { stdio: bool }`.
  - In `crates/cli/src/main.rs`, add a `run_lsp` function that calls
    `ruprizzle_lsp::run_stdio().await`.

- [x] **Step 5: Add to workspace and verify it builds**
  - Add `"crates/lsp"` to the workspace `members` in `Cargo.toml`.
  - Run: `cargo clippy -p ruprizzle-lsp`
  - Expected: no warnings.

- [x] **Step 6: Commit**
  - `git add Cargo.toml crates/lsp crates/cli`
  - `git commit -m "feat(lsp): scaffold ruprizzle-lsp crate and ruprizzle lsp command"`

### Task B.2: Diagnostics via `textDocument/publishDiagnostics`

**Files:**
- Create: `crates/lsp/src/diagnostics.rs`
- Modify: `crates/lsp/src/lib.rs`

**Interfaces:**
- Consumes: `SchemaError` from `ruprizzle_core::diagnostic`, `miette::Diagnostic`.
- Produces: `Vec<lsp_types::Diagnostic>`.

- [x] **Step 1: Write the converter**
  - In `crates/lsp/src/diagnostics.rs`:
    ```rust
    use lsp_types::{Diagnostic, DiagnosticSeverity, NumberOrString, Position, Range};
    use miette::{LabeledSpan, SourceSpan};

    fn byte_offset_to_position(source: &str, offset: usize) -> Position {
        let mut line = 0;
        let mut character = 0;
        for (i, c) in source.char_indices() {
            if i == offset {
                break;
            }
            if c == '\n' {
                line += 1;
                character = 0;
            } else {
                character += 1;
            }
        }
        Position { line, character }
    }

    pub fn schema_error_to_diagnostic(source: &str, err: &SchemaError) -> Diagnostic {
        let default_span = LabeledSpan::new_with_span(None, SourceSpan::from(0));
        let label = err
            .labels()
            .unwrap_or_else(|| Box::new(std::iter::empty()))
            .next()
            .unwrap_or(default_span);
        let start = byte_offset_to_position(source, label.offset());
        let end = byte_offset_to_position(source, label.offset() + label.len());
        Diagnostic {
            range: Range { start, end },
            severity: Some(DiagnosticSeverity::ERROR),
            code: err
                .code()
                .map(|c| NumberOrString::String(c.to_string())),
            source: Some("ruprizzle".into()),
            message: format!("{err}"),
            ..Default::default()
        }
    }
    ```

- [x] **Step 2: Trigger validation on open/change**
  - In `Backend::did_open` and `Backend::did_change`, call
    `ruprizzle_parser::parse_with_warnings(file, text)`.
  - Convert errors to diagnostics and call `client.publish_diagnostics(uri, diagnostics, version).await`.

- [x] **Step 3: Add a test**
  - Create `crates/lsp/tests/diagnostics.rs` with an invalid schema
    (`model User { id String }` — no `@id`) and assert the diagnostic
    message contains "primary key".

- [x] **Step 4: Verify and commit**
  - Run: `cargo test -p ruprizzle-lsp`
  - Expected: PASS.
  - `git commit -m "feat(lsp): publish diagnostics from parser/validator"`

### Task B.3: Completion

**Files:**
- Create: `crates/lsp/src/completion.rs`
- Modify: `crates/lsp/src/lib.rs`

**Interfaces:**
- Consumes: `Schema` IR, AST source text, cursor position.
- Produces: `CompletionResponse`.

- [x] **Step 1: Determine context from source text**
  - Parse the source with `parse_ast` (non-fatal) and find the token nearest the
    cursor.
  - Recognise contexts: top-level block keyword (`datasource`/`model`/`enum`),
    inside a model (field type/attributes), inside `@@index([...])` or
    `@@unique([...])`, inside attribute arguments.

- [x] **Step 2: Build completion lists**
  - Top-level: keywords, model/enum names.
  - Field type: scalar types, user models, user enums, with `[]`/`?` suffix hints.
  - Attributes: `@id`, `@default`, `@unique`, `@relation`, `@db.*`, `@map`,
    `@updatedAt` and argument names.
  - Index/unique: field names of the current model.

- [x] **Step 3: Implement `completion` request**
  - In `Backend::completion`, call `crates/lsp/src/completion.rs::complete(schema, text, position)`.
  - Return `CompletionList { is_incomplete: false, items }`.

- [x] **Step 4: Test**
  - Add `crates/lsp/tests/completion.rs` that places the cursor after a field
    type and expects `"@unique"` in the completion items.

- [x] **Step 5: Commit**
  - `git commit -m "feat(lsp): completion for schema DSL keywords, types and attributes"`

### Task B.4: Go-to-definition and hover

**Files:**
- Create: `crates/lsp/src/goto.rs`
- Create: `crates/lsp/src/hover.rs`
- Modify: `crates/lsp/src/lib.rs`

**Interfaces:**
- Consumes: AST spans (`ast::ModelDecl::name_span`, `ast::FieldDecl::type_span`,
  `ast::EnumDecl::name_span`), `Schema` IR.
- Produces: `GotoDefinitionResponse`, `Hover`.

- [x] **Step 1: Map cursor to a token**
  - Use the byte offset of the cursor to find the AST node whose span contains it.

- [x] **Step 2: Resolve model/enum references**
  - For a `FieldDecl::type_span` that names a model or enum, look up the target
    declaration and return its `Location` (URI + range from `name_span`).

- [x] **Step 3: Hover**
  - For a field token, show `field_name: Type` plus any `///` doc comment.
  - For a model token, show the model name and first paragraph of its docs.

- [x] **Step 4: Test and commit**
  - Add `crates/lsp/tests/goto.rs`.
  - `git commit -m "feat(lsp): go-to-definition and hover for models, enums and fields"`

### Task B.5: Editor packaging

**Files:**
- Create: `editor/vscode/package.json`
- Create: `editor/vscode/src/extension.ts`
- Modify: `editor/README.md`

**Interfaces:**
- Consumes: `ruprizzle-lsp` binary.
- Produces: VS Code extension and editor docs.

- [x] **Step 1: Minimal VS Code extension**
  - `package.json` registers `*.ruprizzle` and a server that starts
    `ruprizzle lsp --stdio`.
  - `src/extension.ts` is the Node/Electron adapter.

- [x] **Step 2: Update `editor/README.md`**
  - Document the VS Code extension and how to run the stdio server in other
    editors.

- [x] **Step 3: Commit**
  - `git commit -m "feat(lsp): minimal VS Code extension and editor docs"`

---

## Workstream C — Offline / compile-time query checking (v1.1)

### Task C.1: Design `query-manifest.json` and write ADR-012

**Files:**
- Create: `docs/adr/ADR-012-OfflineQueryChecking.md`
- Create: `crates/check/Cargo.toml`
- Create: `crates/check/src/manifest.rs`

**Interfaces:**
- Consumes: `CompiledSql` from `crates/runtime/src/compile.rs`, `Schema` IR.
- Produces: `QueryManifest` type and JSON schema.

- [x] **Step 1: Write ADR-012**
  - State the goal: validate dynamically constructed queries and `raw!` fragments
    against the schema **without a live database** at CI/build time.
  - Decision: use a manifest of `(sql, binds, source_file, line, dialect)`
    captured from test/example runs and from a static scan of `raw!` literals,
    then validate against a schema snapshot.

- [x] **Step 2: Define the manifest type**
  - In `crates/check/src/manifest.rs`:
    ```rust
    #[derive(Serialize, Deserialize)]
    pub struct QueryManifest {
        pub schema_hash: String,
        pub queries: Vec<QueryEntry>,
    }

    #[derive(Serialize, Deserialize)]
    pub struct QueryEntry {
        pub sql: String,
        pub source: Option<String>,
        pub line: Option<u32>,
        pub dialect: String,
    }
    ```

- [x] **Step 3: Commit**
  - `git commit -m "docs(adr): ADR-012 for offline query checking and query-manifest"`

### Task C.2: Capture manifests from `to_sql()` in tests/examples

**Files:**
- Create: `crates/runtime/src/query_manifest.rs`
- Modify: `crates/runtime/src/query.rs` (`to_sql` methods)
- Modify: `crates/runtime/src/lib.rs`

**Interfaces:**
- Consumes: `CompiledSql` produced by every builder's `to_sql()`.
- Produces: `query-manifest.json` on disk when `RUPRIZZLE_RECORD_QUERIES` is set.

- [x] **Step 1: Add an opt-in recorder**
  - In `crates/runtime/src/query_manifest.rs`:
    ```rust
    static RECORDING: OnceLock<Mutex<Vec<QueryEntry>>> = OnceLock::new();

    pub fn record(sql: String, source: Option<&'static str>, line: Option<u32>, dialect: &str) {
        if std::env::var("RUPRIZZLE_RECORD_QUERIES").is_ok() {
            RECORDING.get_or_init(|| Mutex::new(Vec::new()))
                .lock().unwrap()
                .push(QueryEntry { sql, source, line, dialect: dialect.into() });
        }
    }
    ```

- [x] **Step 2: Instrument `to_sql()` methods**
  - In `crates/runtime/src/query.rs`, at the end of each public `to_sql()`
    method, call `query_manifest::record(compiled.sql.into_owned(),
    file!(), line!(), self.dialect().name())`.

- [x] **Step 3: Dump manifest at process end**
  - Add a `write_manifest()` function and an `atexit`-style hook or call it from
    `crates/runtime/tests` that set `RUPRIZZLE_RECORD_QUERIES`.

- [x] **Step 4: Commit**
  - `git commit -m "feat(check): opt-in query manifest recorder"`

### Task C.3: Validate `raw!` at compile/build time

**Files:**
- Modify: `crates/macros/src/lib.rs`
- Modify: `crates/macros/Cargo.toml`
- Modify: `crates/check/src/lib.rs`

**Interfaces:**
- Consumes: schema snapshot path, `raw!` format string and bind expressions.
- Produces: compile-time errors for unknown tables/columns or malformed SQL.

- [x] **Step 1: Add an offline schema check to the `raw!` macro**
  - At compile time, if `RUPRIZZLE_OFFLINE_SCHEMA` is set and points to a schema
    file, parse it and extract table/column names.
  - Validate that every identifier in the raw SQL literal that matches a known
    table/column exists. This is a coarse check, not a full SQL parser.
  - If a check fails, emit `syn::Error::new_spanned` so the build fails with a
    file:line message.

- [x] **Step 2: Add a trybuild test**
  - `crates/runtime/tests/trybuild/raw_unknown_table.rs` uses `raw!("SELECT * FROM not_a_table")`.
  - `.stderr` expects "unknown table `not_a_table`".

- [x] **Step 3: Commit**
  - `git commit -m "feat(macros): offline schema validation for raw! SQL"`

### Task C.4: CLI `ruprizzle check`

**Files:**
- Modify: `crates/cli/src/main.rs`
- Create: `crates/check/src/lib.rs`
- Create: `crates/check/src/validate.rs`

**Interfaces:**
- Consumes: `Schema` from parser, `QueryManifest`, `DbDialect`.
- Produces: `Result<(), Vec<QueryCheckError>>`.

- [x] **Step 1: Add `Command::Check`**
  - `ruprizzle check --schema schema.ruprizzle --manifest query-manifest.json`

- [x] **Step 2: Implement validation logic**
  - For each query entry, use `sql_parser` or a small hand-written validator to
    verify:
    - referenced tables exist in `Schema`;
    - referenced columns exist on those tables;
    - columns used in `WHERE`, `ORDER BY`, and `GROUP BY` have compatible types
      with the bound values (use `Field::kind` and `Value` type).
  - Return a non-zero exit code on failure and print `file:line: message`.

- [x] **Step 3: Add tests and commit**
  - `crates/check/tests/validate.rs` with a known-good and a known-bad manifest.
  - `git commit -m "feat(cli): ruprizzle check for offline query validation"`

---

## Workstream D — MySQL hardening and benchmark completeness (v1.0/RC)

### Task D.1: MySQL-specific conformance tests

**Files:**
- Create: `tests/integration/tests/mysql_conformance.rs`
- Modify: `crates/testkit/src/lib.rs`

**Interfaces:**
- Consumes: `both_dbs!` macro, MySQL backend.
- Produces: passing tests for upserts, arrays, rich types on MySQL.

- [x] **Step 1: Promote MySQL to the default `both_dbs!` set**
  - In `crates/testkit/src/lib.rs` or in a new `all_dbs!` macro, run tests on
    SQLite + PostgreSQL + MySQL when all three URLs are available.

- [x] **Step 2: Add MySQL-specific tests**
  - Upsert with `ON DUPLICATE KEY UPDATE`.
  - `String[]` stored as JSON and queried with `contains`/`overlaps`.
  - `Uuid` round-trip through `CHAR(36)`.
  - `Decimal` text fallback on MySQL? MySQL has `DECIMAL`; verify native.

- [x] **Step 3: Document limitations**
  - Add a section to `docs/DialectNotes.md` for MySQL: no `INTERSECT`/`EXCEPT`,
    no `RETURNING` (PK follow-up), no native enums (CHECK constraint),
    no `FULL OUTER JOIN`/`RIGHT JOIN` on older versions.

- [x] **Step 4: Commit**
  - `git commit -m "test(mysql): conformance suite and dialect notes"`

### Task D.2: PostgreSQL and MySQL benchmarks

**Files:**
- Modify: `local/cross-orm-bench/run_bench.py`
- Create: `local/cross-orm-bench/docker-compose.bench.yml`

**Interfaces:**
- Consumes: `BENCH_PG_URL`, `BENCH_MYSQL_URL`, `BENCH_SQLITE_PATH`.
- Produces: benchmark results for Postgres and MySQL.

- [x] **Step 1: Add Docker services**
  - `docker-compose.bench.yml` with PostgreSQL 17 and MySQL 8.4, both with a
    `ruprizzle_test` database and a `ruprizzle` user.

- [x] **Step 2: Extend `run_bench.py`**
  - Read `BENCH_PG_URL` and `BENCH_MYSQL_URL`; skip a backend if the env var is
    missing.
  - For each available backend, run the Rust harness (`cross_orm_bench.exe`) and
    Node harnesses that support that backend.
  - Append a "PostgreSQL" and a "MySQL" section to `docs/BenchmarkResults.md`.

- [x] **Step 3: Commit**
  - `git commit -m "bench: add Postgres and MySQL harnesses"`

### Task D.3: Percentile and throughput metrics

**Files:**
- Modify: `local/cross-orm-bench/run_bench.py`
- Modify: `local/cross-orm-bench/results.json` shape

**Interfaces:**
- Consumes: raw trial times from each harness.
- Produces: p50/p95/p99 and req/s.

- [x] **Step 1: Add percentile computation**
  - Use `statistics.quantiles` or `numpy` if available; fallback to sorted-list
    interpolation.
  - Add `p50`, `p95`, `p99` to the per-operation result.

- [x] **Step 2: Add throughput**
  - Add a `BENCH_CONCURRENCY` env var (default 1, 10, 100).
  - Run each operation with `N` concurrent clients for a fixed duration and
    record operations/second.

- [x] **Step 3: Update `docs/BenchmarkResults.md` template**
  - Add percentile and throughput tables.

- [x] **Step 4: Commit**
  - `git commit -m "bench: percentile and throughput reporting"`

### Task D.4: Compile-time benchmark

**Files:**
- Create: `local/cross-orm-bench/compile_time.py`
- Create: `xtask/src/bench_compile.rs`

**Interfaces:**
- Consumes: generated 50/200 model schemas.
- Produces: compile-time numbers.

- [x] **Step 1: Generate large schemas**
  - Add a script that creates synthetic `schema.ruprizzle` files with 50 and 200
    models, then runs `cargo build --release -p generated_client`.

- [x] **Step 2: Measure**
  - Use `time` or `cargo build --timings` to capture wall time and binary size.
  - Record in `docs/BenchmarkResults.md`.

- [x] **Step 3: Commit**
  - `git commit -m "bench: generated-client compile-time and binary size"`

---

## Workstream E — PostgreSQL advanced DDL (v1.1)

### Task E.1: Partial indexes (`@@index([...], where: ...)`)

**Files:**
- Modify: `crates/core/src/ir.rs:527-535` `IndexDef`
- Modify: `crates/parser/src/ast.rs` and `crates/parser/src/lower.rs`
- Modify: `crates/parser/src/grammar.pest` (if needed)
- Modify: `crates/dialect/src/postgres.rs:146-154`, `crates/dialect/src/sqlite.rs`,
  `crates/dialect/src/mysql.rs`
- Modify: `crates/migrate/src/diff.rs` (index diff)

**Interfaces:**
- Consumes: schema IR `IndexDef` with a new `where_clause: Option<String>`.
- Produces: `CREATE INDEX ... WHERE <where_clause>` on Postgres/SQLite.

- [x] **Step 1: Extend IR**
  - In `crates/core/src/ir.rs`:
    ```rust
    pub struct IndexDef {
        pub db_name: String,
        pub fields: Vec<IndexField>,
        pub where_clause: Option<String>,
        pub span: Span,
    }
    ```

- [x] **Step 2: Parse `@@index([...], where: "...")`**
  - In the parser, accept a named argument `where` on `@@index` and store it.

- [x] **Step 3: Render `WHERE` clause**
  - In `crates/dialect/src/postgres.rs` and `crates/dialect/src/sqlite.rs`, append
    `WHERE {where_clause}` to `create_index` when present.
  - In `crates/dialect/src/mysql.rs`, reject with `DialectError` because MySQL
    does not support partial indexes.

- [x] **Step 4: Update migration diff**
  - In `crates/migrate/src/diff.rs` (or wherever index diffs live), compare
    `where_clause` and emit drop/re-create when it changes.

- [x] **Step 5: Test and commit**
  - Add a snapshot test in `crates/migrate/tests` or `tests/integration`.
  - `git commit -m "feat(schema): partial indexes with @@index where clause"`

### Task E.2: Expression indexes

**Files:**
- Modify: `crates/core/src/ir.rs:527-545`
- Modify: `crates/parser/src/lower.rs`
- Modify: `crates/dialect/src/*.rs`
- Modify: `crates/migrate/src/diff.rs`

**Interfaces:**
- Consumes: `IndexDef` where entries may be field names or SQL expressions.
- Produces: `CREATE INDEX ... ON table (expression)`.

- [x] **Step 1: Extend IR with expression targets**
  - In `crates/core/src/ir.rs`:
    ```rust
    pub enum IndexTarget {
        Field(FieldName),
        Expression(String),
    }

    pub struct IndexDef {
        pub db_name: String,
        pub targets: Vec<IndexTarget>,
        pub where_clause: Option<String>,
        pub span: Span,
    }
    ```

- [x] **Step 2: Parse `@@index(["(lower(email))"])` and `@@unique(["(coalesce(a,b))"])`**
  - A string that starts with `(` is treated as an expression; otherwise it is a
    field name.

- [x] **Step 3: Render expressions**
  - In `create_index` and `add_unique`, render expression targets verbatim and
    field targets as quoted column names.

- [x] **Step 4: Test and commit**
  - `git commit -m "feat(schema): expression indexes and unique constraints"`

### Task E.3: Generated columns

**Files:**
- Modify: `crates/core/src/ir.rs:288-308` `Field`
- Modify: `crates/parser/src/lower.rs`
- Modify: `crates/dialect/src/common.rs` `column_spec` and `ColumnSpec`
- Modify: `crates/dialect/src/postgres.rs`, `crates/dialect/src/mysql.rs`,
  `crates/dialect/src/sqlite.rs`
- Modify: `crates/codegen/src/emit.rs`

**Interfaces:**
- Consumes: new `@generated("always as (...) stored")` attribute.
- Produces: `GENERATED ALWAYS AS (...) STORED/VIRTUAL` columns; generated
  columns are read-only in the query builder.

- [x] **Step 1: Extend IR**
  - In `crates/core/src/ir.rs`:
    ```rust
    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    pub enum GeneratedKind {
        Virtual,
        Stored,
    }

    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    pub struct GeneratedClause {
        pub expr: String,
        pub kind: GeneratedKind,
    }
    ```
    Add `pub generated: Option<GeneratedClause>` to `Field`.

- [x] **Step 2: Parse `@generated(...)`**
  - Syntax: `@generated("always as (lower(first_name || ' ' || last_name)) stored")`.
  - Lower to `GeneratedClause { expr, kind }`.

- [x] **Step 3: Render in dialects**
  - PostgreSQL: `col TYPE GENERATED ALWAYS AS (expr) STORED`.
  - MySQL: same.
  - SQLite: same (3.31+).

- [x] **Step 4: Codegen read-only**
  - In `crates/codegen/src/emit.rs`, omit generated columns from the
    `InsertQuery`/`UpdateQuery` setters but include them in `SelectQuery`
    projections.

- [x] **Step 5: Test and commit**
  - `git commit -m "feat(schema): generated columns"`

### Task E.4: PostgreSQL extensions

**Files:**
- Modify: `crates/core/src/ir.rs` `Datasource`
- Modify: `crates/parser/src/lower.rs`
- Modify: `crates/migrate/src/planner.rs`
- Modify: `crates/dialect/src/postgres.rs`

**Interfaces:**
- Consumes: `datasource db { extensions = ["uuid-ossp", "postgis"] }`.
- Produces: `CREATE EXTENSION IF NOT EXISTS ...` in migration plan.

- [x] **Step 1: Extend IR and parser**
  - Add `pub extensions: Vec<String>` to `Datasource`.
  - Parse `extensions = ["..."]` in the datasource block.

- [x] **Step 2: Emit in migration plan**
  - In `crates/migrate/src/planner.rs`, at the start of a Postgres migration,
    emit `CREATE EXTENSION IF NOT EXISTS {ext};` for each listed extension.
  - On down migrations, emit `DROP EXTENSION IF EXISTS {ext};` only if no other
    schema objects depend on it (best effort).

- [x] **Step 3: Add extension capability flags**
  - In `crates/dialect/src/postgres.rs` `Capabilities`, add
    `postgis: bool` (default false). This is used later by PostGIS support.

- [x] **Step 4: Test and commit**
  - `git commit -m "feat(migrate): CREATE EXTENSION from datasource extensions"`

---

## Workstream F — v1.2+ advanced query and relation features

These features are large enough that each should become its own spec or plan
once v1.0/v1.1 are stable. The sections below are design sketches with the
specific files to touch, so the implementer does not start from a blank page.

### Task F.1: Full-text search

**Design:**
- Schema syntax: `searchText String @fulltext` and
  `@@fulltext([searchText], name: "posts_fts")`.
- Dialect SQL:
  - PostgreSQL: create `tsvector` column or GIN index, query with
    `to_tsvector('english', searchText) @@ plainto_tsquery('english', $1)`.
  - SQLite: create `FTS5` virtual table or `FTS4`; query with `MATCH`.
  - MySQL: `FULLTEXT` index; query with `MATCH (...) AGAINST (...)`.
- Builder API: `post::search_text.search("rust orm")` returning a `Filter<M>`.

**Files to touch:**
- `crates/core/src/ir.rs` `FieldAttrs` add `is_fulltext`;
  `Model` add `fulltext_indexes: Vec<FullTextIndexDef>`.
- `crates/parser/src/lower.rs` parse `@fulltext` and `@@fulltext`.
- `crates/dialect/src/{postgres,sqlite,mysql}.rs` generate index/table and query
  fragments.
- `crates/runtime/src/filter.rs` add `FullText` filter node.
- `crates/runtime/src/compile.rs` compile full-text filter to dialect SQL.
- `crates/codegen/src/emit.rs` emit `Column<M, String>` tokens for full-text
  fields and a `search` method.

### Task F.2: PostGIS / geospatial types

**Design:**
- Native types: `@db.Point`, `@db.Geography`, `@db.Geometry`.
- New scalar types in `crates/core/src/ir.rs` `ScalarType`: `Point`,
  `Geography`, `Geometry` (mapped to `geo_types` or `postgis` Rust types).
- Query operators: `ST_Contains`, `ST_DWithin`, `ST_Intersects`, generated as
  methods on geospatial `Column`s.
- Migration: use `CREATE EXTENSION postgis` from Workstream E.4.

**Files to touch:**
- `crates/core/src/ir.rs` `ScalarType`, `NativeType`.
- `crates/dialect/src/postgres.rs` type mapping and operator SQL.
- `crates/runtime/src/col.rs` geospatial `Column` methods.
- `crates/runtime/src/compile.rs` operator compilation.
- `crates/codegen/src/emit.rs` Rust type mapping.
- Add optional `postgis` feature to `crates/runtime/Cargo.toml`.

### Task F.3: Soft deletes

**Design:**
- Schema syntax: `deletedAt DateTime? @deletedAt` and optional datasource-level
  `softDelete = true`.
- Query builder: `SelectQuery` automatically adds
  `WHERE deleted_at IS NULL` unless `.with_deleted()` or `.only_deleted()` is
  called.
- `DeleteQuery` executes `UPDATE table SET deleted_at = NOW() WHERE ...` instead
  of `DELETE`.
- Unique constraints that include a soft-deleted model need to consider the
  `deleted_at` column; migration engine may suggest partial unique indexes.

**Files to touch:**
- `crates/core/src/ir.rs` `FieldAttrs` add `is_deleted_at`.
- `crates/parser/src/lower.rs` validate `@deletedAt` on `DateTime?`.
- `crates/dialect/src/{postgres,sqlite,mysql}.rs` default `deleted_at` column
  type.
- `crates/runtime/src/query.rs` `SelectQuery` and `DeleteQuery`.
- `crates/codegen/src/emit.rs` emit `with_deleted()` and `only_deleted()` on
  generated `SelectQuery` extensions.

### Task F.4: Polymorphic relations

**Design:**
- Schema syntax:
  ```text
  model Asset {
    id        Uuid   @id @default(uuid7())
    ownerType String
    ownerId   Uuid
    owner     User?  @relation(polymorphic: true, fields: [ownerType, ownerId], references: [id])
  }
  ```
- The relation carries a discriminator (`ownerType`) and one or more FK columns.
- Codegen emits a `PolymorphicRelated<M>` wrapper and query methods that filter
  by discriminator value.

**Files to touch:**
- `crates/core/src/ir.rs` `RelationRef` add `polymorphic: bool` and
  `discriminator: Option<FieldName>`.
- `crates/parser/src/lower.rs` resolve and validate polymorphic relations.
- `crates/dialect/src/*.rs` generate FK columns and CHECK constraint on
  `discriminator`.
- `crates/runtime/src/rel.rs` and `crates/runtime/src/include.rs` handle
  polymorphic loads.
- `crates/codegen/src/emit.rs` emit polymorphic relation accessors.

### Task F.5: Implicit many-to-many join tables

**Design:**
- ADR-006 currently says "no implicit join tables in v1". Promote this to an
  ADR-013 amendment for v1.2+ that allows sugar over an explicit join model.
- Schema syntax (no `through` on a list relation):
  ```text
  model Post {
    tags Tag[] @relation(...)
  }
  ```
- Lowering creates a hidden join model `PostTag` with columns `post_id` and
  `tag_id`.
- Codegen still uses `M2mWrite`/`IncludeMany` from `crates/runtime/src/m2m.rs`
  but hides the join model from the public API.

**Files to touch:**
- `docs/adr/ADR-013-ImplicitManyToMany.md`
- `crates/core/src/ir.rs` allow `RelationRef.through = None` for many-to-many.
- `crates/parser/src/lower.rs` auto-create implicit join models.
- `crates/migrate/src/planner.rs` generate join tables.
- `crates/codegen/src/emit.rs` generate `post.tags_attach(...)` etc. without a
  user-visible `PostTag` model.

### Task F.6: Recursive tree / hierarchy loading

**Design:**
- For self-referential models, generate:
  - `ancestors(depth)` via recursive CTE (`WITH RECURSIVE up AS ...`).
  - `descendants(depth)` via recursive CTE.
  - `subtree()` to load a self-referential `include` to arbitrary depth.
- Dialects: PostgreSQL and SQLite support recursive CTEs; MySQL 8.0+ supports
  them as well.

**Files to touch:**
- `crates/runtime/src/include.rs` add `IncludeTree`.
- `crates/runtime/src/query.rs` add `SelectQuery::with_recursive_tree(...)`.
- `crates/runtime/src/compile.rs` generate recursive CTE SQL.
- `crates/codegen/src/emit.rs` emit `ancestors()`/`descendants()` methods for
  self-referential relations.

---

## Workstream G — SQLite rich-type workarounds (documentation)

### Task G.1: Keep the limitation honest and the escape hatch visible

**Files:**
- Modify: `docs/KnownLimitations.md:15-34`
- Modify: `docs/performance.md` (if relevant)

- [x] **Step 1: Document the recommended path**
  - State that `Decimal` and `Json` on SQLite are stored as text by the default
    `sqlx::Any` path and that `sqlite-rusqlite` parses them back at decode time
    without the `sqlx::Any` text round-trip.
  - Mention that exact decimal math on SQLite should use `Int` minor units or
    a PostgreSQL backend.

- [x] **Step 2: Commit**
  - `git commit -m "docs(sqlite): clarify rich-type storage and rusqlite workaround"`

---

## Exit Gates and Verification

Before any release phase:

- [ ] `cargo fmt --all --check` passes.
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` passes.
- [ ] `cargo test --workspace` passes.
- [ ] `cargo xtask harden` passes.
- [ ] `cargo deny check advisories` passes.
- [ ] `cargo doc --workspace --no-deps` has no warnings.
- [ ] `docs/KnownLimitations.md` contains only deliberate design positions,
      not "not implemented yet."
- [ ] `docs/FeaturesMasterComparison.md` matches the target state for the phase.

## Self-Review

1. **Spec coverage:** Every row in the `V1MissingRequired.md` validation table
   maps to at least one task above. JSON path, arrays, MySQL, pool metrics, and
   JSONB are already complete; the plan does not duplicate them.
2. **Placeholder scan:** No forbidden placeholder terms remain. Each
   task names the exact files to create or modify and the expected commands.
3. **Type consistency:** `FieldAttrs`, `IndexDef`, `Field`, and `Datasource`
   changes are described consistently across Workstreams C, E, and F.
