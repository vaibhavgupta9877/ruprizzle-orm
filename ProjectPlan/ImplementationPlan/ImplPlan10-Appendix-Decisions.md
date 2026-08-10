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
**Why:** see the comparison table in ImplPlan06-P5-03. JOINs cause row explosion,
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
