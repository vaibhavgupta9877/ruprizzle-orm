# Path to Stable v1.0

> **Status:** ACTIVE — currently at W0 (immediate hygiene); W1–W6 are pending.

**From:** `0.1.1-beta.1` (published to crates.io 2026-08-13, 84/100 production readiness)
**To:** `1.0.0` — a version whose API we commit to under semver and whose capability surface
does not lose a feature comparison on absence alone.

**Created:** 2026-08-13
**Owner:** Vaibhav Gupta
**Supersedes:** §9 of [`../ProductionReadiness.md`](../ProductionReadiness.md) and the
"Deliberately out of scope" list of
[`../ProductionReadinessPlan.md`](../ProductionReadinessPlan.md)

> **For agentic workers:** REQUIRED SUB-SKILL — use `superpowers:subagent-driven-development`
> or `superpowers:executing-plans` to implement this plan task by task. Steps use checkbox
> (`- [ ]`) syntax for tracking. Do not batch tasks across workstream boundaries; each has
> its own exit gate.

---

## 1. What 1.0 means here

`1.0.0` is not "no more bugs." It is three specific commitments, and the plan exists to
make all three affordable:

1. **Semver commitment.** The public API of `ruprizzle`, `ruprizzle-migrate`, and the
   generated client stops changing without a major bump. Anything we are not prepared to
   support for years must be feature-gated, marked unstable, or removed *before* 1.0 —
   which is why the capability work comes before the version bump, not after.
2. **Operational commitment.** A team can run this in production and answer "is it healthy,
   and if not, why" without reading our source. That means metrics, not just spans.
3. **Capability commitment.** A team evaluating ruprizzle against Prisma, Drizzle, Diesel,
   or SeaORM loses features by *choosing a trade-off*, not by hitting a wall. Today they
   hit walls: no savepoints, no aggregates, no joins, no MySQL.

### Explicitly not part of 1.0

Named here so each omission is a decision rather than an oversight:

- **MongoDB, ScyllaDB, DuckDB, Cassandra.** The relational schema DSL and the migration
  diff engine are built on relational assumptions. Supporting a document store means a
  second product.
- **Multi-tenancy primitives and row-level security.** Real features, but they belong to a
  layer above the ORM and would be premature to freeze under semver.
- **pgvector / vector search.** Deferred to 1.1. Additive behind a custom column type; no
  reason to block 1.0 on it.
- **A hosted studio / GUI.** `drizzle-studio` and Prisma Studio are strong DX, but a web
  application is out of scope for a library workspace. Reconsider post-1.0.
- **Replacing `sqlx::Any` as the default.** ADR-009 stands. Native paths are the escape
  hatch; runtime dialect selection is the promise.

---

## 2. Current position

The numbers this plan starts from, all verified at commit `c3ef7f0`:

| Metric | Value |
|---|---|
| Production readiness | 84 / 100 |
| Tests | 218 passing, 0 failing, 4 ignored, 55 binaries |
| Source / test lines | 18,855 / 5,207 (3.6 : 1) |
| Clippy | Zero warnings at `-D warnings` |
| `xtask harden` | Passes; all crates at or under panic budget |
| `cargo fmt --all --check` | **Failing** — 5 hunks (finding #1) |
| Published versions | 4, none yanked, 43 total downloads |
| Databases | PostgreSQL, SQLite |
| Driver paths | `sqlx::Any` (default), `sqlite-rusqlite`, `postgres-tokio-postgres` |

**Where we already win** (from `docs/BenchmarkResults.md`, 2026-08-12 SQLite, µs/op):
fastest `select_by_pk` in the comparison at 3.0 (Diesel 9.9, Drizzle 29.0, Prisma 162.3);
fastest `bulk_insert_1000` at 1,191 (Diesel 5,336, Prisma 13,154); nested `include` at
4,468 versus Sea-ORM 23,437 and Prisma 33,534. **Performance is not this plan's problem.**
Every workstream below is capability, operability, or assurance.

---

## 3. Workstream overview

Seven workstreams, ordered so that each unblocks the next. W0 is hygiene and takes a day;
W1–W3 are the substance; W4–W6 are what converts "works" into "we will support this."

| # | Workstream | Effort | Gates |
|---|---|---|---|
| **W0** | Immediate hygiene | 1 day | Green CI on every job |
| **W1** | Transaction & type completeness | 1.5 weeks | Savepoints, arrays, streaming |
| **W2** | Query surface — Prisma & Drizzle parity | 3.5 weeks | Aggregates, joins, CTEs, nested writes |
| **W3** | Operability | 1 week | Metrics, slow-query events, runbook |
| **W4** | Assurance | 1.5 weeks | Fuzzing, soak, feature-matrix CI |
| **W5** | Ecosystem & DX | 2.5 weeks | MySQL, introspection, LSP, seeding |
| **W6** | Release engineering & the 1.0 commitment | 1 week | Stability policy, publish automation |

**Total: ~12 weeks for one experienced Rust developer**, or ~7 calendar weeks with W2 and
W5 partially parallelised (they touch different crates: W2 is `runtime` + `codegen`, W5 is
`parser` + `dialect` + `cli`).

---

# W0 — Immediate hygiene

**Goal:** every gate this project owns is green before any feature work starts. One day.
**Rationale:** the assessment's finding #1 is that a gate we own is red on a published
commit. Feature work on top of a red gate is how a red gate becomes permanent.

- [x] **W0-01 · Fix rustfmt.** Run `cargo fmt --all`. Hunks appeared in the formatter
      output across `crates/runtime/src/compile.rs`, `include.rs`, `query.rs`, `related.rs`,
      `xtask/src/main.rs`, the two example files, and several test files; all were pure
      line-wrapping with no semantic change. Verified with `cargo fmt --all --check`.
- [x] **W0-02 · Gitignore deep-test scratch dirs.** `/local/deep-tests/db/.tmp*/` was
      already in `.gitignore`; 43 stale `.tmp*` directories were removed from the working
      tree. *(finding #10)*
- [x] **W0-03 · Add backend features to the CI matrix.** Coverage is already provided by
      the `native-drivers` job in `.github/workflows/ci.yml`, which runs `cargo clippy` and
      `cargo test` for `sqlite-rusqlite`, `postgres-tokio-postgres`, and both features
      combined. *(finding #4)*
- [x] **W0-04 · Governance and release plumbing.** Added `CODE_OF_CONDUCT.md`
      (Contributor Covenant 2.1), `.github/ISSUE_TEMPLATE/bug.yml`,
      `.github/ISSUE_TEMPLATE/feature.yml`, `.github/pull_request_template.md`, and
      `.github/workflows/release.yml` which runs the full gate and then publishes the eight
      crates in dependency order on a `v*` tag. *(finding #7)*
- [x] **W0-05 · Promote ADRs to `docs/adr/`.** Split
      `ImplPlan10AppendixDecisions.md`'s ADR-001 … ADR-009 into
      `docs/adr/ADR-00N-<slug>.md` with an index, leaving a pointer behind. Source comments
      already cite bare ADR numbers as though this tree exists. *(2 h — finding #6)*

**Exit gate:** all nine CI jobs green, plus the two new feature jobs. `cargo fmt --all
--check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`,
and `cargo xtask harden` all clean locally and in CI.

---

# W1 — Transaction & type completeness

**Goal:** close the three gaps most likely to make a user abandon the library mid-project.
**Effort:** ~1.5 weeks.

These are not parity features — they are correctness-adjacent holes in what we already
claim to do. `Tx` exists but cannot nest. `Value::Array` exists but throws. `stream()` exists
but buffers.

### W1-01 · Savepoints and nested transactions — **the single most valuable item in this plan**

*Assessment finding #2. 3 days.*

`Tx` is flat: `begin`, `begin_with_isolation`, `commit`, `rollback`. Any code that wants a
retryable sub-unit inside a transaction — the standard pattern for "try the optimistic
path, fall back on conflict" — has to abandon the transaction entirely. Diesel, Drizzle, and
Prisma all support this; Drizzle implements nested `transaction()` as savepoints directly.

- [x] **Step 1.** Add `Tx::savepoint(&self) -> Result<Savepoint<'_>, Error>` issuing
      `SAVEPOINT <generated_name>`. Names must be generated (monotonic counter per `Tx`), never
      user-supplied — a user-supplied savepoint name is a SQL identifier injection vector, and
      `xtask harden` will correctly reject any `format!` that interpolates one.
- [x] **Step 2.** `Savepoint::release(self)` → `RELEASE SAVEPOINT`; `Savepoint::rollback(self)`
      → `ROLLBACK TO SAVEPOINT`. Drop without either must roll back, matching `Tx`'s existing
      drop semantics, and emit a `tracing` warning.
- [x] **Step 3.** Nesting: a `Savepoint` must itself yield savepoints, to arbitrary depth.
      Model the depth in the type or in a runtime counter — do not allow a released savepoint
      to be released twice.
- [x] **Step 4.** Ergonomic closure form: `tx.transaction(|sp| async { … })` that commits on
      `Ok` and rolls back to the savepoint on `Err`, mirroring Drizzle's nested-transaction API.
- [x] **Step 5.** Implement across all three driver paths — `sqlx::Any`, `rusqlite`,
      `tokio-postgres`. The `rusqlite` path is synchronous and runs on `spawn_blocking`;
      savepoint lifetime must not outlive the blocking task.
- [x] **Step 6.** `crates/runtime/tests/savepoint.rs` via the `both_dbs!` macro: nested commit,
      nested rollback leaving the outer transaction live, drop-without-release, depth ≥ 3, and
      rollback-to-savepoint after a constraint violation (the motivating case).
- [x] **Step 7.** Document in `docs/QueryGuide.md` and remove the savepoint entry from
      `docs/KnownLimitations.md`.

### W1-02 · Array bind values

*Assessment finding #3. 3 days.*

`Value::Array` is rejected at bind time in all four encoders (`value.rs:132,271,318,384`,
`tokio_postgres.rs:325`). It reads as unimplemented rather than dead. Postgres arrays are a
headline Postgres feature and their absence is called out in `KnownLimitations.md`.

- [x] **Step 1.** Decide the scope and record it as an ADR: Postgres arrays are real
      (`int4[]`, `text[]`, `uuid[]`); SQLite has no array type. The honest answer is native
      support on Postgres and a documented, explicit JSON-encoded fallback on SQLite —
      **not** silent divergence between backends.
- [ ] **Step 2.** Encode on `postgres-tokio-postgres` natively via `ToSql` for `Vec<T>`.
- [ ] **Step 3.** Encode on `sqlx::Any` + Postgres. If `Any` cannot express array binds,
      that is itself an ADR-009 data point: document that arrays require the native feature
      and error with a message that says so, rather than the current bare
      "array bind values are not supported yet".
- [ ] **Step 4.** Schema DSL support: `tags String[]` in `schema.ruprizzle`, through the
      parser, dialect DDL (`text[]`), codegen, and the migration differ.
- [ ] **Step 5.** Filter operators: `contains`, `contained_by`, `overlaps` (`@>`, `<@`, `&&`).
- [ ] **Step 6.** Tests across all paths; update `KnownLimitations.md`.

### W1-03 · True streaming cursors

*2 days.*

`SelectQuery::stream` buffers the full result set and yields decoded rows.
`KnownLimitations.md` justifies this honestly with a measurement — `sqlx`'s `.fetch()` is
~64% slower per row on SQLite — and that justification is correct for small result sets and
wrong for large ones. A million-row export should not require a million-row `Vec`.

- [ ] **Step 1.** Keep the buffered path as the default; it is faster and most queries are
      small. Add `SelectQuery::stream_unbuffered()` backed by `sqlx`'s `.fetch()` and, on
      the native paths, by the driver's own cursor.
- [ ] **Step 2.** Postgres: use a server-side cursor (`DECLARE`/`FETCH`) on the
      `tokio-postgres` path so memory is bounded server-side too, not just client-side.
- [ ] **Step 3.** Document the trade-off in `docs/QueryGuide.md` with the measured numbers —
      buffered for < ~10k rows, unbuffered above — so the choice is informed rather than
      guessed.
- [ ] **Step 4.** Test with a result set larger than a deliberately constrained memory budget.

**W1 exit gate:** savepoints, arrays, and unbuffered streaming all work on all three driver
paths, tested via `both_dbs!`; three entries removed from `KnownLimitations.md`; test count
≥ 250.

---

# W2 — Query surface: Prisma & Drizzle parity

**Goal:** stop losing feature comparisons on absence. **Effort:** ~3.5 weeks.

This is the largest workstream and the one that determines whether 1.0 is competitive.
`docs/FeaturesMasterComparison.md` currently records `Aggregates: Partial`, `JSON
operators: Partial`, `Many-to-many: Partial`, `Lazy loading: No` — while Prisma and Drizzle
are `Yes` on all four. The builder today has `filter`, `or_filter`, `order_by`, `limit`,
`offset`, `after`/`before` cursors, `distinct`, `include`, `columns`, `count`, `exists`,
`page`, upsert, bulk insert, and `with_related` nested writes. Everything below is what it
does not have.

### W2-01 · Aggregates and grouping

*From Prisma's `aggregate` / `groupBy`; Drizzle's SQL aggregate helpers. 5 days.*

Today only `count()` and `exists()`. There is no `sum`, `avg`, `min`, `max`, and no
`GROUP BY` at all — `grep` for `group_by` across `crates/runtime/src` returns one comment.

- [ ] **Step 1.** `Aggregate<M>` expression type over typed `Column<M, V>` tokens:
      `sum`, `avg`, `min`, `max`, `count`, `count_distinct`. Numeric aggregates must be
      constrained to numeric column types at compile time — that type safety is the reason
      to prefer this over raw SQL.
- [ ] **Step 2.** `SelectQuery::aggregate(...)` returning a typed struct rather than a map.
- [ ] **Step 3.** `SelectQuery::group_by(cols)` yielding a `GroupedQuery<M>`, with
      `having(Filter)` on the aggregate.
- [ ] **Step 4.** Codegen: generated per-model aggregate result structs so
      `User::query(&db).group_by(User::role).aggregate(...)` returns named fields.
- [ ] **Step 5.** `.to_sql()` must work on grouped and aggregate queries — SQL transparency
      on every builder is a stated product promise and cannot have holes.
- [ ] **Step 6.** Tests via `both_dbs!` plus snapshot tests of emitted SQL.

### W2-02 · Explicit joins

*Drizzle's `leftJoin`/`innerJoin`; Diesel's join DSL. 5 days.*

ADR-004 chose batched relation loading over JOINs, and that decision is correct for
`include` — the benchmarks vindicate it. But it is a decision about *relation loading*, not
about the query language. There is currently no way to express a join at all, which blocks
every reporting query.

- [ ] **Step 1.** Write ADR-010 first, distinguishing "batched loading is the default for
      `include`" from "explicit joins are available for queries the batcher cannot express."
      Do not weaken ADR-004.
- [ ] **Step 2.** `SelectQuery::inner_join` / `left_join` / `right_join` / `full_join` over
      declared relations, with the join condition inferred from the schema's foreign key.
- [ ] **Step 3.** Joins on arbitrary conditions via typed column tokens from both sides,
      for the cases where the FK is not the join key.
- [ ] **Step 4.** Result typing: joined queries return tuples of model types, with
      `Option<T>` for the nullable side of an outer join. This is where the type system
      earns its keep and where the design must be got right the first time — it is a 1.0 API.
- [ ] **Step 5.** Self-joins with table aliasing.
- [ ] **Step 6.** `.to_sql()`, snapshot tests, `both_dbs!` coverage.

### W2-03 · Subqueries, CTEs, and set operations

*Drizzle's `with()`, `union`/`intersect`/`except`, and subquery support. 5 days.*

None of these exist. They are what "SQL-like typed builder" means to a Drizzle user, and
`FeaturesMasterComparison.md` rates our query style as exactly that.

- [ ] **Step 1.** Subqueries in filters: `User::id.in_subquery(Post::query().columns(Post::author_id))`.
- [ ] **Step 2.** Correlated `EXISTS` / `NOT EXISTS` subqueries.
- [ ] **Step 3.** CTEs: `Query::with("name", subquery)` emitting `WITH … AS (…)`, including
      the `RECURSIVE` form — recursive CTEs are the standard answer to tree/hierarchy
      queries and self-referential relations are already a supported feature.
- [ ] **Step 4.** Set operations: `union`, `union_all`, `intersect`, `except`, with
      compile-time enforcement that both sides project the same shape.
- [ ] **Step 5.** Dialect differences: SQLite lacks `FULL OUTER JOIN` before 3.39 and
      differs on `RIGHT JOIN`. `DbDialect` must report capability and the builder must
      return a clear compile-time or construction-time error rather than emitting SQL that
      fails at the server. Document in `docs/DialectNotes.md`.

### W2-04 · JSON operators

*Currently `Partial` for us, `Yes` for Prisma. 3 days.*

- [ ] **Step 1.** Postgres: `->`, `->>`, `#>`, `#>>`, `@>`, `?`, `jsonb_set`, exposed as
      typed methods on `Column<M, Json>`.
- [ ] **Step 2.** SQLite: `json_extract` and friends. `KnownLimitations.md` says SQLite
      `Json` is stored as text and cannot be queried — SQLite's JSON1 extension makes this
      addressable, so either implement it or restate the limitation accurately.
- [ ] **Step 3.** Path-based filtering and ordering on JSON fields.
- [ ] **Step 4.** Update `KnownLimitations.md` and the comparison table.

### W2-05 · Full many-to-many

*Currently `Partial` for us, `Yes` for every competitor. 3 days.*

ADR-006 chose explicit join models. That is a defensible, arguably better design than
Prisma's implicit join tables — but "you must model the join table yourself and traverse it
in two hops" is why the table says `Partial`.

- [ ] **Step 1.** `@relation(through: PostTag)` in the schema DSL, declaring a many-to-many
      that traverses an explicit join model.
- [ ] **Step 2.** Codegen a direct `post.tags` accessor that hides the two-hop traversal
      while the join model stays visible and queryable — keeping ADR-006's honesty and
      Prisma's ergonomics.
- [ ] **Step 3.** `include` support through the relation, preserving the bounded
      one-query-per-level batching guarantee.
- [ ] **Step 4.** Nested writes: attach/detach/set semantics on the join rows.
- [ ] **Step 5.** Upgrade the comparison table entry to `Yes` with a footnote naming the
      explicit-join-model design.

### W2-06 · Nested writes and relation mutations

*Prisma's nested `create`/`connect`/`disconnect`. 3 days.*

`InsertQuery::with_related` already exists, which is the hard half.

- [ ] **Step 1.** Extend to nested update and nested delete.
- [ ] **Step 2.** `connect` / `disconnect` / `set` for existing rows by primary key.
- [ ] **Step 3.** Cascade behaviour must be explicit and must match the schema's declared
      `onDelete`, not silently diverge from what the database will do.
- [ ] **Step 4.** All nested writes run in a single transaction — and with W1-01 landed,
      each nested step can take a savepoint, so a partial failure need not discard the batch.

### W2-07 · Prepared statements and `$dynamic` building

*Drizzle's `.prepare()` and dynamic query mode. 2 days.*

- [ ] **Step 1.** `SelectQuery::prepare()` producing a reusable compiled statement with
      named placeholders, skipping SQL construction per call. The benchmark already shows
      `to_sql` at 0.9 µs on the `Any` path — for a 3.0 µs query that is a third of the
      budget, so this is a measurable win, not just an ergonomic one.
- [ ] **Step 2.** Conditional building — apply a `filter` only when a value is present —
      without the builder's type state fighting it. This is the single most common
      real-world builder complaint across every ORM.
- [ ] **Step 3.** Benchmark prepared versus unprepared and publish in `docs/Performance.md`.

**W2 exit gate:** `FeaturesMasterComparison.md` regenerated with no `Partial` in the query
builder or relations sections that a competitor scores `Yes` on; every new builder supports
`.to_sql()`; snapshot tests for all emitted SQL; test count ≥ 340.

---

# W3 — Operability

**Goal:** close the largest remaining scoring gap (dimension 3, 7.5/10). **Effort:** ~1 week.

- [x] **W3-01 · Metrics export behind a `metrics` feature.** Query count, duration
      histogram, error count by `Error::kind()`, pool size/idle/waiters, migration
      application count and duration. Emit via the `metrics` crate facade so users pick
      Prometheus or OTel rather than us picking for them. **2 days.** *(finding #5)*
- [x] **W3-02 · Slow-query threshold event.** A configurable duration above which a query
      emits `WARN` with the SQL *shape* — bind counts, never values. The existing PII
      discipline in `error_redaction.rs` is the standard to hold. **0.5 day.**
- [x] **W3-03 · `docs/Operations.md`.** What each span means, what to alert on, how to read
      `PoolStats`, how to interpret saturation, what to do when `ping` fails, and a worked
      example dashboard. The assessment has called this out for two passes. **1 day.**
- [x] **W3-04 · Connection lifecycle events.** Trace connect, disconnect, acquire-timeout,
      and reconnect. Today a pool that is thrashing is invisible.  **0.5 day.**
- [x] **W3-05 · Concurrency and throughput benchmarks.** The axis the current Criterion
      suite does not cover: queries/sec against pool size, tail latency under contention,
      behaviour at pool exhaustion. **2 days.**

**Exit gate:** dimension 3 rescored ≥ 9.0; `docs/Operations.md` published; Prometheus/OTel
scrape documented in `docs/Operations.md`; connection lifecycle, slow-query, and query metrics
covered by tests; `cargo fmt --check`, `cargo clippy -p ruprizzle --features sqlite-rusqlite`,
`cargo test -p ruprizzle --features sqlite-rusqlite`, and `cargo bench -p ruprizzle --features
sqlite-rusqlite --bench concurrency` all pass.

---

# W4 — Assurance

**Goal:** convert "correct" into "correct over time and under attack." **Effort:** ~1.5 weeks.

- [ ] **W4-01 · Fuzz the parser and the migration splitter.** `cargo-fuzz` targets for
      `crates/parser` (schema DSL) and `crates/migrate` (SQL splitter). These are the two
      hand-written scanners over untrusted-ish input, and the splitter has already produced
      two silent-corruption defects once. This is the one defect class the current suite is
      structurally unlikely to find. **3 days.** *(finding #8)*
- [x] **W4-02 · Soak test.** 48 hours of sustained mixed load with connection churn and a
      forced failover, tracking memory, file descriptors, and pool health. This is the
      evidence the assessment has cited as missing in all three passes and the reason the
      "critical data" verdict is ⚠️ rather than ✅. **3 days.**
- [x] **W4-03 · Feature-combination CI matrix.** Formalise W0-03 into a real matrix across
      the three driver paths and both databases. **0.5 day.**
- [ ] **W4-04 · Justify or remove the `grammar.rs` panic sites.** 27 of 29. Either a comment
      per site naming the Pest invariant that makes it unreachable, or a real error path.
      Then lower the budget. **1 day.** *(finding #9)*
- [ ] **W4-05 · Mutation testing.** `cargo-mutants` over `crates/migrate` and
      `crates/runtime` to find where 218 tests are passing without asserting anything.
      **1 day.**
- [ ] **W4-06 · Remove the throwaway `pool.acquire()` in `is_postgres`.** Detect the backend
      from connect options. **0.5 day.** *(finding #11)*

**Exit gate:** fuzzers run ≥ 4 CPU-hours per target with no crashes and are wired into a
scheduled CI job; soak report published; mutation score recorded as a baseline.

---

# W5 — Ecosystem & DX

**Goal:** the features that decide adoption rather than evaluation. **Effort:** ~2.5 weeks.
May run in parallel with W2 — different crates.

- [ ] **W5-01 · MySQL / MariaDB dialect.** The single largest database-support gap; every
      competitor supports it. Additive behind `DbDialect`. Needs dialect SQL generation,
      migration DDL, a `both_dbs!` extension to three databases, and CI service containers.
      **5 days.**
- [ ] **W5-02 · Introspection (`ruprizzle db pull`).** Generate `schema.ruprizzle` from an
      existing database. Currently `Planned` in the comparison table while Prisma, Drizzle,
      and SeaORM all ship it. This is the on-ramp for every existing project — arguably the
      highest-adoption-leverage item in the whole plan. **4 days.**
- [ ] **W5-03 · Seeding.** `ruprizzle db seed` with a declarative seed file and a
      transactional, idempotent apply. Every competitor has it. **2 days.**
- [ ] **W5-04 · Migration squashing.** Deferred to 0.2 in `KnownLimitations.md`; collapse a
      migration history into a single baseline. Long-lived projects need it. **2 days.**
- [ ] **W5-05 · Heuristic rename detection.** Currently requires `@renamedFrom`. Detect
      likely renames from the diff and *prompt* — never guess silently, since a wrong guess
      is data loss. **2 days.**
- [ ] **W5-06 · Mutual foreign-key cycles in migrations.** Currently must be broken by
      hand. Emit deferred constraints on Postgres and a documented two-phase apply on
      SQLite. **2 days.**
- [ ] **W5-07 · LSP.** Completion, diagnostics, go-to-definition for `schema.ruprizzle`.
      Deferred to 0.2; the TextMate grammar covers highlighting only. Prisma's LSP is a
      major part of why its schema DSL feels good. **5 days — the one item safe to slip past
      1.0 if the schedule bites.**

**Exit gate:** MySQL in the CI matrix and the comparison table; `db pull` round-trips a
non-trivial existing schema; `KnownLimitations.md` "Deferrals to 0.2" section reduced to LSP
alone or emptied.

---

# W6 — Release engineering & the 1.0 commitment

**Goal:** make the semver promise real. **Effort:** ~1 week.

- [ ] **W6-01 · Public API review.** Enumerate the full public surface of every crate with
      `cargo-public-api`. Everything we are not prepared to support for years gets
      feature-gated, `#[doc(hidden)]`, marked unstable, or removed. **This is the last
      cheap moment to remove anything.** **2 days.**
- [ ] **W6-02 · API stability policy.** `docs/Stability.md`: what is covered by semver, what
      is explicitly not (generated code internals, `xtask`, benchmark harnesses), MSRV
      policy, and the deprecation process. Dimension 8 stays below 8 until this exists.
      **0.5 day.**
- [ ] **W6-03 · `cargo-semver-checks` in CI.** Mechanical enforcement of W6-02, so semver
      is a gate rather than a habit. **0.5 day.**
- [ ] **W6-04 · Release candidates.** `1.0.0-rc.1` with a real feedback window before
      `1.0.0`. The current 43 downloads across four versions is not enough exposure to
      freeze an API on. **Do not skip this.** **Calendar time, not effort.**
- [ ] **W6-05 · Final production readiness assessment.** Re-run against `1.0.0-rc.1`,
      targeting **≥ 92/100**, with dimension 1 (correctness) ≥ 9.0 on the back of fuzzing and
      soak, and dimension 3 (operability) ≥ 9.0 on the back of W3. **1 day.**
- [ ] **W6-06 · Migration guide from beta.** Every breaking change between `0.1.1-beta.1`
      and `1.0.0`, with before/after code. **1 day.**

---

## 4. Sequencing

```
W0 ─┬─> W1 ──> W2 ──────────────> W3 ──> W4 ──> W6
    └─> W5 (parallel with W1/W2, different crates)
```

- **W0 gates everything.** Do not start feature work on a red `fmt` job.
- **W1 precedes W2** because W2-06's nested writes want savepoints from W1-01.
- **W3 follows W2** so the new query surface is instrumented as it lands, not retrofitted.
- **W4 follows W2 and W5** because fuzzing and soak must cover the final code, not an
  intermediate state.
- **W6 is last by definition** — the API cannot be frozen until it is finished.

### Milestones

| Version | Contains | Cumulative | Meaning |
|---|---|---|---|
| `0.1.1` | W0 | week 1 | Every gate green; publishable by automation |
| `0.2.0-beta.1` | W0 + W1 | week 3 | Savepoints, arrays, streaming — no more hard walls |
| `0.3.0-beta.1` | + W2 | week 7 | Query surface competitive with Prisma and Drizzle |
| `0.4.0-beta.1` | + W3 + W5 | week 10 | Operable and adoptable; MySQL and `db pull` |
| `1.0.0-rc.1` | + W4 + W6 | week 12 | Assured and frozen |
| `1.0.0` | rc feedback | week 14+ | Committed |

Breaking changes are expected across the `0.x` line and are the reason for staying below
1.0 through W2. W2-02's join result typing in particular should be treated as provisional
until W6-01 reviews it.

---

## 5. Feature parity scorecard — target state at 1.0

Tracks `docs/FeaturesMasterComparison.md`. **Bold** = changed by this plan.

| Feature | Today | At 1.0 | Workstream |
|---|---|---|---|
| Savepoints / nested transactions | No | **Yes** | W1-01 |
| Array binds & array columns | No | **Yes** (Postgres native) | W1-02 |
| Streaming / cursors | Buffered | **Yes** (both modes) | W1-03 |
| Aggregates | Partial | **Yes** | W2-01 |
| Group by / having | No | **Yes** | W2-01 |
| Explicit joins | No | **Yes** | W2-02 |
| Subqueries / CTEs / set ops | No | **Yes** | W2-03 |
| JSON operators | Partial | **Yes** | W2-04 |
| Many-to-many | Partial | **Yes** | W2-05 |
| Nested writes (update/delete/connect) | Partial | **Yes** | W2-06 |
| Prepared statements | No | **Yes** | W2-07 |
| Metrics export | No | **Yes** | W3-01 |
| MySQL / MariaDB | No | **Yes** | W5-01 |
| Introspection (`db pull`) | Planned | **Yes** | W5-02 |
| Seeding | Partial | **Yes** | W5-03 |
| Migration squashing | No | **Yes** | W5-04 |
| Rename detection | No | **Yes** (prompted) | W5-05 |
| FK cycles in migrations | No | **Yes** | W5-06 |
| LSP | No | **Yes** | W5-07 |
| Compile-time query checking | Planned | Planned | *deferred to 1.1* |
| Lazy loading | No | No | *rejected — conflicts with ADR-004* |
| Multi-tenancy / RLS | No | No | *out of scope, §1* |
| Vector search | No | No | *deferred to 1.1* |
| MSSQL / MongoDB / edge targets | No | No | *out of scope, §1* |

**Lazy loading is a deliberate no.** Every competitor except Diesel and Drizzle offers it,
and it is the single most reliable source of accidental N+1 queries in production ORMs.
ADR-004's bounded one-query-per-level batching is the better answer, and adding lazy loading
would undermine it. This should be stated as a design position in the comparison table's
footnotes rather than left looking like a gap.

---

## 6. Risks

| Risk | Impact | Mitigation |
|---|---|---|
| W2-02 join result typing proves unworkable in Rust's type system without heavy macro machinery | High — joins are table stakes | Prototype the typing *before* committing to the API. Tuple-of-models with `Option` for outer sides is the fallback; a `#[derive]`-based row struct is the escalation. Timebox to 2 days before escalating. |
| W5-01 MySQL forces a three-way `both_dbs!` rewrite and doubles integration CI time | Medium | Extend the macro first as its own task; run MySQL jobs on merge rather than on every PR. |
| Scope creep — the parity list is long and each item suggests two more | High | The §5 scorecard is the contract. Anything not in it is 1.1. Do not add rows mid-plan. |
| Freezing the API at 1.0 before real users have exercised the new surface | High | W6-04's rc window is non-negotiable. 43 downloads is not validation. |
| W2 lands 7 new builder types and the `.to_sql()` promise silently develops holes | Medium | Snapshot-test emitted SQL for every builder; add an `xtask` check that every public builder type exposes `to_sql`. |
| Fuzzing (W4-01) finds a parser defect class requiring a grammar rewrite | Medium | Run fuzzers early — start them during W2 even though the gate is in W4 — so findings arrive with time to absorb them. |

---

## 7. Definition of done

`1.0.0` ships when all of these hold:

- [ ] Every workstream exit gate met.
- [ ] Production readiness ≥ 92/100, with correctness and operability each ≥ 9.0.
- [ ] `cargo fmt`, `clippy -D warnings`, `cargo test --workspace`, `cargo deny`, and
      `cargo xtask harden` green across the full feature and OS matrix.
- [ ] Fuzzers clean at ≥ 4 CPU-hours per target; 48-hour soak with no leak or degradation.
- [ ] `docs/Stability.md` published and `cargo-semver-checks` enforcing it.
- [ ] `docs/KnownLimitations.md` contains only deliberate design positions — no "not
      implemented yet."
- [ ] `docs/FeaturesMasterComparison.md` matches the §5 target column.
- [ ] `1.0.0-rc.1` has had a real feedback window with at least one external project
      reporting a successful upgrade.
- [ ] Automated publish workflow has performed at least one release end to end.

---

*This plan supersedes §9 of `ProductionReadiness.md` and the "Deferred to 0.2" list in
`docs/KnownLimitations.md`. It does not supersede `ProductionReadinessPlan.md`, which is
complete and retained as history. Effort estimates assume one experienced Rust developer
and exclude the release-candidate feedback window.*
