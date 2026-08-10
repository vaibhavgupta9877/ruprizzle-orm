# ImplPlan 10 — Decisions, Risks & Deferrals

Reference document. Not a phase. Update the ADR log as decisions change.

---

## Architecture decision records

### ADR-001 · Build on sqlx rather than a custom driver
**Decision:** sqlx provides connection pooling, the wire protocols, TLS, and type
encoding. We generate code that calls it.
**Why:** writing a Postgres wire-protocol implementation is a multi-month project
with a large security surface and no differentiation. Every hour spent there is an
hour not spent on the schema DSL, diffing, and relation loading — which is where
the actual product is.
**Cost:** we inherit sqlx's release cadence and any breaking changes. Mitigated by
an exact version pin in the runtime crate.

### ADR-002 · Codegen, not proc-macros-over-structs
**Decision:** a CLI generates source files that the user commits (or gitignores).
**Why:** three reasons. Generated code is *readable* — users can open it, understand
it, and debug it, which proc-macro output never is. IDE completion works perfectly
with no macro expansion. And compile times are better, because the user's build
does not run the parser and codegen on every `cargo check`.
**Cost:** an explicit `generate` step. Mitigated by `--watch` (P7-03).
**Rejected alternative:** `#[derive(Model)]` on hand-written structs, SeaORM-style.
That inverts the source of truth — the schema stops being declarative and
migrations can no longer be diffed from it, which forfeits the headline feature.

### ADR-003 · `Related<T>` instead of `Option<T>` for relations
**Decision:** a three-state-by-construction type distinguishing "not loaded" from
"loaded and empty."
**Why:** `Option<Vec<Post>>` makes `None` ambiguous, and the ambiguity produces a
silent wrong answer rather than an error. Loud failure with an actionable message
beats a quiet bug.
**Cost:** one unfamiliar type in the public API, and one sanctioned panic.

### ADR-004 · Batched relation loading, not JOINs
**Decision:** one query per relation level.
**Why:** see the comparison table in ImplPlan06 P5-03. JOINs cause row explosion,
make per-relation `take` and `filter` hard, and require de-duplication.
**Cost:** more round trips than a single JOIN for shallow queries. Acceptable — the
count is bounded by schema depth, not data size.

### ADR-005 · Column tokens, not a type-level query DSL
**Decision:** `Column<M, T>` consts with inherent methods, rather than Diesel's
type-level relational algebra.
**Why:** Diesel's approach is more powerful and catches strictly more at compile
time, but its error messages are the single most cited reason people bounce off it.
Column tokens catch the errors that actually happen in practice — wrong value type,
wrong model, wrong operator for the type — with errors a human can read.
**Cost:** some invalid queries (a malformed `GROUP BY`, say) fail at the database
rather than at compile time.

### ADR-006 · Explicit join models for many-to-many
**Decision:** no implicit join tables in v1.
**Why:** hidden tables violate the predictable-SQL principle, and Prisma's implicit
`_AToB` tables become a migration dead end the moment you need a column on the
join. Sugar can be added later without breaking anyone.

### ADR-007 · Snapshot = serialized IR
**Decision:** the migration snapshot is `serde_json` of `ir::Schema`.
**Why:** one type means the differ compares exactly what the parser produces, with
no second schema representation to drift out of sync. It is also human-readable in
diffs and easy to inspect during debugging.
**Cost:** IR changes require a snapshot format version and a migration path. A
`version` field is present from day one for this reason.

### ADR-008 · Postgres and SQLite together from day one
**Decision:** both dialects in v1, not Postgres first and SQLite later.
**Why:** an abstraction with one implementation is not proven to be an abstraction.
Adding SQLite in month two would mean discovering every Postgres assumption baked
into codegen and migrations at the worst possible moment. SQLite also makes the
test suite fast and dependency-free for contributors.
**Cost:** roughly three extra days in P2 and P6, mostly the SQLite table-rebuild
path. Paid deliberately.

---

## P0 deviation log

Deviations from [ImplPlan01Foundation.md](ImplPlan01Foundation.md) made while
implementing P0. Recorded per that file's own instruction; none change the
architecture.

| # | Deviation | Why |
|---|---|---|
| **D-001** | Added `crates/testkit` and `tests/integration` as workspace members, neither in the original layout | The harness needed a home. Putting it in a `publish = false` crate keeps `TestDb` and `both_dbs!` out of the published graph while letting every crate's tests use them. |
| **D-002** | Replaced the `--features it` gate with a runtime skip plus `RUPRIZZLE_REQUIRE_DB=1` | A feature flag makes `cargo test` fail confusingly for a contributor without Docker, and `--features` on a virtual workspace requires every member to declare it. The skip keeps local runs green; the env var — set in CI — makes an absent database a hard failure. Strictly better than the flag: it cannot silently pass in CI. |
| **D-003** | `generated-code-lint` CI job is a guard assertion, not a real lint | There is no generator until P3. The job asserts `ruprizzle generate` is still unimplemented and fails with instructions the moment that stops being true, so it cannot silently pass once codegen lands. |
| **D-004** | `SchemaError` fields exempted from `missing_docs` | Every field is already described by the variant's `#[error]` message and `#[label]` text, which is where a reader meets it. Rustdoc restating those would add noise and drift. `missing_docs` stays enforced everywhere else. |
| **D-005** | Implemented all 22 `SchemaError` variants (rules V01–V18) in P0 rather than P1 | The variants are mechanical; writing them alongside the diagnostic infrastructure was near-free and means P1 writes validation logic against errors that already exist. |
| **D-006** | `miette`'s `fancy` feature is enabled only in the CLI and dev-dependencies | `ruprizzle-core` is depended on transitively by the runtime; a library should not force a terminal renderer and its dependencies on every consumer. |
| **D-007** | Added `rust-toolchain.toml` (pinned 1.95) and `cargo xtask ci` | Pinning makes `rustfmt`/`clippy` output identical for every contributor and in CI. `xtask ci` makes "what CI runs" one local command instead of a YAML file that drifts. |
| **D-008** | Added `Schema::relations` and `Schema::fingerprint()` to the IR | The former makes the ImplPlan06 relation-canonicalisation guarantee structural (both sides index the same entry, so they cannot disagree). The latter is needed by P3 and P7 to detect stale generated code; adding both up front avoids an IR change later, which risk R2 identifies as expensive. |

---

## P1 deviation log

Deviations from [ImplPlan02SchemaDslParser.md](ImplPlan02SchemaDslParser.md) made
while implementing P1. None change the architecture, and the `parse(&str) ->
Result<ir::Schema>` boundary is unchanged.

| # | Deviation | Why |
|---|---|---|
| **D-101** | Keyword rules are atomic (`@{ "model" ~ !ident_char }`), not silent | A silent rule has implicit whitespace inserted between the word and its boundary check, so `model` happily matched the start of `modelish`. Atomic emits one pair per keyword, which the AST walk drops. |
| **D-102** | `field_type` is atomic, and the arity marker is read from its text | Pest reports the innermost rule it wanted. With a non-atomic `field_type`, a missing type surfaced as "expected `ident`", which no P1-04 phrasing can rescue. Atomic makes the failure report `field_type`, which is what lets the error say "expected a field type". |
| **D-103** | Validation is split: rules needing attribute spans run in `lower.rs` (V02–V10, V12–V15), the rest in `validate.rs` (V01, V11, V14-empty, V16, V17) | A rule can only point at `@updatedAt` if it can see that attribute, and the IR deliberately does not keep per-attribute spans. Splitting on "what does this rule need to point at" keeps every diagnostic's label accurate. |
| **D-104** | `references:` defaults to the target's primary key when omitted | It is the primary key in every schema that bothers to write it out, and the alternative is an error for something we can resolve unambiguously. |
| **D-105** | Referential defaults are `onDelete: Restrict` (required) / `SetNull` (optional), `onUpdate: Cascade` | Prisma's defaults, and the right ones: deleting out from under a required foreign key must fail, while an optional one can simply be cleared. The IR's `ReferentialAction::default()` (`NoAction`) is a SQL-level default, not a schema-level one. |
| **D-106** | Added `help(...)` to `DuplicateField` and `DuplicateVariant` in `ruprizzle-core` | They were the only two variants without one. The P1-03 fixtures assert the P0-03 standard mechanically against real parser output, and these two failed it. |
| **D-107** | V03's `PascalCase` naming check is not implemented | There is no `NamingConvention` variant in `SchemaError`, and a warning that fires on every deliberately-lowercase model name is worse than none. Duplicate-declaration detection, the part of V03 that matters, is enforced. |
| **D-108** | V18 (dialect capability) is not implemented | It is defined by P2's capability matrix, which does not exist yet. `SchemaError::DialectDegraded` is in place waiting for it. |

---

## Risk register

| # | Risk | Prob | Impact | Mitigation | Trigger to act |
|---|---|---|---|---|---|
| R1 | **Diff engine (P6-02) overruns.** Ordering, cycles, and renames are genuinely hard. | 60% | 🔴 high | Property test early; ship `db push` as the fallback path so the product is usable without perfect diffing | 2 days into P6 with ordering still failing |
| R2 | IR churn after P0 forces rework across crates | 40% | 🔴 high | Claude owns the IR; changes require an ADR entry; all crates consume IR through accessors, never field access patterns spread widely | any IR change after G3 |
| R3 | SQLite table-rebuild drops data in an edge case | 35% | 🔴 high | Dedicated test module (P2-04); every rebuild test asserts row-level data equality, not just schema shape | any failing rebuild test |
| R4 | Generated-code compile times unacceptable at 50+ models | 40% | 🟠 med | Module-per-model already; measure in P8-02; if bad, offer `--split-columns` to move tokens to a separate crate | > 60 s cold for 50 models |
| R5 | `trybuild` error messages become unreadable as generics grow | 30% | 🟠 med | Keep `Column` bounds shallow; resist adding type parameters; ADR-005 exists to hold this line | any `trybuild` expected-output over 20 lines |
| R6 | sqlx 0.9 breaks the runtime mid-build | 20% | 🟠 med | Exact pin; `Executor` trait already isolates sqlx behind our own interface | sqlx 0.9 release during P4–P8 |
| R7 | Scope creep from the consuming application project | 50% | 🟠 med | This repo is ORM-only. Application needs go to the deferral list, not into v1 | any request referencing UI, auth, or RPC |
| R8 | Relation loader goes accidentally quadratic | 25% | 🟡 low | Query-count *and* timing assertions in P5-06; `HashMap` attachment specified explicitly | include bench > 15% overhead |

**R1 is the one to watch.** It is the highest-value feature and the highest-risk
task, which is a bad combination. It is scheduled in week 6 with a buffer week
behind it deliberately. If it slips, the honest fallback is to ship v1 with
`db push` plus hand-written migrations — which is exactly what Diesel and sqlx
offer today, so v1 would still be no worse than the incumbents, just less
differentiated.

---

## Kill criteria

Carried forward from RealityCheck, adapted to ORM-only scope. A failed gate stops
forward motion; it does not get carried.

| Gate | Failure condition | Action |
|---|---|---|
| G1 (wk 1) | sample schema does not parse to correct IR | swap Pest for hand-written recursive descent (+2 days). The IR boundary makes this cheap by design. |
| G2 (wk 2) | DDL does not apply to a live database | stop; the dialect trait is wrong. Re-derive it from the actual DDL both engines need, do not patch. |
| G3 (wk 3) | generated crate does not compile | reduce codegen scope: drop projections and tuple `Projection` impls to 0.2, ship entities + tokens only |
| G4 (wk 4) | CRUD does not round-trip on both DBs | drop SQLite to 0.2, ship Postgres-only, keep the trait |
| G5 (wk 5) | nested include incorrect or unbounded | ship depth-1 include only; document depth-2+ as 0.2 |
| G6 (wk 6) | diff engine incorrect | ship `db push` + hand-written migrations; diffing becomes the 0.2 headline |

Two consecutive failed gates means re-scoping the release, not adding weeks.

---

## Deferred to 0.2 and beyond

Nothing here is a bug or an oversight. Each is a scoping decision, and the
Known Limitations doc (P7-05) publishes this list verbatim.

**0.2 — the obvious next increment**
- MySQL / MariaDB dialects (additive behind `DbDialect`)
- Nested update, upsert, `connect` / `disconnect` / `set`
- Implicit many-to-many sugar over explicit join models
- `db pull` — introspect an existing database into `schema.ruprizzle`
- LSP for `.ruprizzle`: completion, go-to-definition, live diagnostics
- Scalar list columns (Postgres arrays)
- Aggregations and `GROUP BY` in the builder
- Include depth beyond 3, self-referential recursive loading

**0.3+**
- Compile-time SQL verification for raw queries against the known schema
- Studio-equivalent data browser
- Read replicas and connection routing
- Soft deletes, optimistic locking, audit columns as first-class attributes
- JSON path querying, full-text search helpers
- Multi-schema (Postgres `search_path`) support
- Partitioning and sharding awareness

**Explicitly out of scope, indefinitely**
- Cross-database joins
- A generic "any database" abstraction beyond relational SQL
- Anything in the UI, auth, or RPC layers — those belong to the separate project
  that will consume this ORM

---

## Open questions for the project owner

Answer before the corresponding phase begins; none of them block starting.

1. **Crate naming on crates.io.** Is `ruprizzle` available, and is the runtime
   crate `ruprizzle` with the CLI as `ruprizzle-cli`? Check and reserve before P8.
   *(Needed by: P8. Reserve the names now — this is cheap and irreversible if
   someone else takes them.)*
2. **Turso.** ImplPlan03 covers SQLite; Turso is SQLite-compatible with a different
   connection layer. Is Turso a v1 requirement or 0.2? The earlier design docs
   mention it as 0.2 scope, and this plan assumes 0.2.
   *(Needed by: P2.)*
3. **`uuid7()` as the recommended default PK.** It gives better index locality than
   uuid4 but leaks creation time. Acceptable for the target applications?
   *(Needed by: P2-03.)*
4. **Generated code: committed or gitignored?** This plan's `init` gitignores it,
   which keeps diffs clean but means CI must run `generate` before `build`.
   *(Needed by: P7-04.)*
5. **Does the consuming application project have schema requirements** that should
   shape `examples/saas-tenant`? Aligning now makes the later integration a
   non-event rather than a discovery.
   *(Needed by: P8-03.)*
