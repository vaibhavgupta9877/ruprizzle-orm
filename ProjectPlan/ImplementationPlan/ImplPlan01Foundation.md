# ImplPlan 01 — Foundation (Phase P0)

**Duration:** 2 days · **Owners:** Claude (IR design), Devin (CI, harness)
**Exit gate:** workspace builds, CI green, both databases reachable from tests.

> **Status: ✅ COMPLETE.** All five tasks landed; 34 tests pass; `cargo xtask ci`
> (fmt, clippy `-D warnings`, test, docs) is green. The code is now the source of
> truth — the sketches below are kept for intent, with pointers to what shipped.
> Deviations are logged in [ImplPlan10AppendixDecisions.md](ImplPlan10AppendixDecisions.md#p0-deviation-log).
> One verification gap is recorded under [Known gaps](#known-gaps).

---

## P0-01 · Workspace scaffold ✅

**Owner:** Devin · **Est:** 3h

```
ruprizzle-orm/
├── Cargo.toml                 # workspace root, edition 2024, resolver 3
├── rust-toolchain.toml        # pinned 1.95 + rustfmt, clippy
├── crates/
│   ├── core/                  # ruprizzle-core       ✅ implemented
│   ├── parser/                # ruprizzle-parser     (P1)
│   ├── dialect/               # ruprizzle-dialect    (P2)
│   ├── codegen/               # ruprizzle-codegen    (P3)
│   ├── runtime/               # ruprizzle            (P4)
│   ├── migrate/               # ruprizzle-migrate    (P6)
│   ├── macros/                # ruprizzle-macros     (P4)
│   ├── cli/                   # ruprizzle-cli        ✅ surface, P7 behaviour
│   └── testkit/               # ruprizzle-testkit    ✅ implemented (D-001)
├── tests/integration/         # cross-crate suite    ✅ implemented
├── xtask/                     # cargo xtask ci       ✅ implemented
└── docker-compose.yml         # postgres:17-alpine, tmpfs data
```

Crates not yet implemented ship a documented placeholder that names the phase
that fills them in, so the dependency graph is complete and `cargo build
--workspace` is meaningful from day one.

**Pin discipline:** `sqlx` is the only dependency whose breakage is existential.
Pinned in `[workspace.dependencies]` and bumped deliberately.

**Acceptance met:** `cargo build --workspace` and `cargo test --workspace` succeed
on a clean checkout.

---

## P0-02 · Core IR (`ruprizzle-core`) ✅

**Owner:** Claude · **Est:** 5h · **Shipped:** `crates/core/src/ir.rs`

The contract every other crate speaks. Churn here is the single biggest schedule
risk, which is why it was specified before anything consumed it.

Shipped surface: `Schema`, `Datasource`, `Provider`, `DatasourceUrl`, `Generator`,
`EnumDef`, `EnumVariant`, `Model`, `Field`, `FieldKind`, `ScalarType`,
`FieldAttrs`, `NativeType`, `PrimaryKey`, `IndexDef`, `IndexField`, `SortOrder`,
`UniqueDef`, `DefaultValue`, `Literal`, `DefaultFn`, `RelationRef`,
`ResolvedRelation`, `RelationKind`, `ReferentialAction`, plus `IR_VERSION`.

**Design notes worth defending — all realised:**

- **`IndexMap`, never `HashMap`.** Declaration order determines generated-code
  order and migration diff order. A hash map would make both churn between runs
  on identical input. This is a correctness requirement for the diff engine, not
  a micro-optimisation, and it is asserted by
  `declaration_order_survives_serialisation`.
- **`Span` on every node,** so P1 diagnostics and P6 migration warnings can point
  at the exact source line. Adding spans later is a painful refactor; adding them
  first cost nothing.
- **`Schema` is `Serialize`/`Deserialize`,** because the migration snapshot format
  *is* the serialized IR (ADR-007). One type, two uses, no second representation
  to drift.
- **Physical names resolved at lowering.** `Model::table` and `Field::column` are
  computed once, so no downstream crate needs to know the naming rules.
- **`Field::has_column()`** distinguishes column-bearing fields from navigation
  properties, which is the distinction P2 (DDL) and P3 (codegen) both need and
  neither should re-derive.

Two additions beyond the original sketch, both earning their place:

- `Schema::relations: Vec<ResolvedRelation>` with `RelationRef::resolved` indexing
  into it. This is the ImplPlan06 canonicalisation guarantee expressed in the type
  system: both sides of a relation reach the *same* entry, so they cannot disagree
  about foreign keys or referential actions.
- `Schema::fingerprint()` — a SHA-256 of the canonical serialisation, so P3 can
  stamp generated files and P6/P7 can detect stale generated code.

**Acceptance met:** IR compiles; `crates/core/tests/ir_roundtrip.rs` proves JSON
round-tripping, fingerprint stability, order preservation, and relation
canonicalisation; rustdoc on every public item, enforced by `-D warnings`.

---

## P0-03 · Diagnostics (`ruprizzle-core::diagnostic`) ✅

**Owner:** Claude · **Est:** 3h · **Shipped:** `crates/core/src/diagnostic.rs`,
`crates/core/src/suggest.rs`

Prisma's best non-technical feature is its error messages. Budgeting for that up
front rather than retrofitting is why this is a P0 task.

Shipped: `SchemaError` with **22 variants** covering validation rules V01–V18,
`SchemaErrors` (the reportable bundle), and `Diagnostics` (the accumulator).
Implementing the full rule set here rather than in P1 was cheap — the variants are
mechanical — and it means P1 writes validation logic against errors that already
exist (D-006).

Rules, all enforced:

- Every error carries a `#[label]` on the offending span and a `help()` that says
  what to **do**, not just what is wrong. Asserted mechanically by
  `every_error_offers_a_fix`, so a missing `help(...)` fails the build rather than
  relying on review to catch it.
- Suggestions use Levenshtein distance over the in-scope identifier set, with a
  threshold that scales to input length — `suggest::closest` returns `None` rather
  than guessing, because a confidently wrong suggestion is worse than none.
- Errors **accumulate**. `Diagnostics` collects; nothing short-circuits. Warnings
  are routed separately by declared severity and never fail a build.

`SchemaErrors` holds the source once and the individual errors inherit it through
miette's `#[related]`, so no variant carries its own copy of the schema text.

**Acceptance met:** `reports_every_error_in_one_pass` puts three distinct mistakes
through one run and snapshots the rendered report — three errors, three correct
spans, three fixes:

```
Error: unknown type `Strng`
  snippet line 3:   email Strng
    label at line 3, columns 9 to 13: not a known scalar, enum, or model
  diagnostic help: did you mean `String`?
```

---

## P0-04 · Test harness ✅

**Owner:** Devin · **Est:** 5h · **Shipped:** `crates/testkit/`, `tests/integration/`

Three tiers, each with a different speed/fidelity trade-off:

| Tier | What | Runs on | Speed |
|---|---|---|---|
| Unit | parser, IR lowering, diff algebra | every `cargo test` | ms |
| Snapshot | generated Rust, generated SQL, diagnostics | every `cargo test` | ms |
| Integration | real Postgres + real SQLite | every `cargo test` (skips if absent) | seconds |

**Snapshot testing** uses `insta`. The highest-leverage decision in P0: codegen
output is large and reviewing it by eye does not scale, but `cargo insta review`
turns every change into a visible diff. Wired up and already carrying the
diagnostic-rendering snapshots.

**Integration harness.** `TestDb` gives each test its own database — a fresh
`rz_<uuid>` schema on Postgres, a temp-directory file on SQLite — so the suite
runs concurrently without collisions, and tears both down on drop. Two details
that matter more than they look:

- Postgres `search_path` is set in `after_connect`, on *every* pooled connection.
  Setting it once would send a test that outgrows one connection back to `public`
  halfway through.
- SQLite sets `PRAGMA foreign_keys = ON`. It is off by default, so without this
  every referential-integrity test from P2 onward would pass vacuously on SQLite
  while genuinely testing only Postgres.

**The dual-database rule.** Every integration test is written once and run against
both backends:

```rust
both_dbs! {
    setup = SMOKE_DDL;
    async fn insert_then_select(db: TestDb) { /* ... */ }
}
```

This generates `insert_then_select::postgres` and `insert_then_select::sqlite`. It
is what keeps SQLite from silently rotting while Postgres gets all the attention —
the failure mode RealityCheck flagged explicitly.

**Skip policy (D-002).** When Postgres is unreachable the case skips with a printed
notice, so a contributor without Docker still gets a green `cargo test`. CI sets
`RUPRIZZLE_REQUIRE_DB=1`, which turns the skip into a failure. Verified in both
directions: without the flag the suite is green and prints the notice; with it,
all five Postgres cases fail loudly. The skip cannot hide breakage.

**Acceptance met:** `both_dbs!` works; five harness tests cover writes, isolation,
cascade deletes, no-setup cases, and backend/pool agreement; `insta` is in CI.

---

## P0-05 · CI pipeline ✅

**Owner:** Devin · **Est:** 3h · **Shipped:** `.github/workflows/ci.yml`, `xtask/`

| Job | Command | Blocking |
|---|---|---|
| fmt | `cargo fmt --all --check` | yes |
| clippy | `cargo clippy --workspace --all-targets -- -D warnings` | yes |
| test | `cargo test --workspace` (no database — proves the skip path) | yes |
| integration | same, with a Postgres service and `RUPRIZZLE_REQUIRE_DB=1` | yes |
| generated-code-lint | guard assertion until P3 (D-003) | yes |
| msrv | build on the pinned `rust-version`, read from the manifest | yes |
| docs | `cargo doc --workspace --no-deps` with `RUSTDOCFLAGS=-D warnings` | yes |

The `test` and `integration` split is deliberate: one job proves the suite is
green for a contributor without Docker, the other proves the databases are
genuinely exercised. Running only the second would let the first silently rot.

`generated-code-lint` is unusual and important — our output is other people's
source code, so a warning in it is our bug and must fail *our* build. There is no
generator until P3, so the job currently asserts that fact: if `ruprizzle generate`
starts producing output and nobody wires up the real check, the job fails and says
what to replace it with. A job that silently passed would be worse than no job.

`cargo xtask ci` runs fmt, lint, test, and docs locally in CI order, so "what CI
runs" is one command rather than a YAML file that drifts from reality.

**Acceptance met:** all seven jobs defined; the four runnable without a database
verified locally via `cargo xtask ci`.

---

## Known gaps

**Postgres was not exercised on the development machine.** No container runtime is
installed there, so all five `::postgres` cases skipped. The SQLite half of every
test passed, and the skip policy was verified in both directions — but the
Postgres code paths in `TestDb` (schema creation, `search_path`, cascade
teardown) are **unverified until CI runs them**. This is the first thing to
confirm on the initial CI run, before P1 work depends on the harness.

---

## Phase P0 checklist

- [x] P0-01 workspace builds
- [x] P0-02 IR defined, serde round-trips, fully documented
- [x] P0-03 diagnostics render with spans via `miette`
- [x] P0-04 `both_dbs!` + `insta` harness working
- [x] P0-05 seven CI jobs defined; four verified locally
- [x] Decision log started in ImplPlan10 for deviations from this file
