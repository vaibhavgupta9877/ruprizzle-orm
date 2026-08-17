# ImplPlan 03 — Dialects & SQL Generation (Phase P2)

**Status:** COMPLETE — both Postgres and SQLite dialect implementations shipped; all P2 tasks below are treated as complete.
**Duration:** 3 days · **Owners:** Vaibhav Gupta (trait design), Vaibhav Gupta (both impls)
**Exit gate G2:** both dialects emit DDL that actually applies to a live database.

---

## Why the trait comes first

The single most expensive mistake this project could make is generating Postgres
SQL directly and abstracting later. Retrofitting a dialect boundary through
codegen and migrations is a multi-week rewrite. We define the seam on day one and
implement two backends immediately, because **two implementations is what proves an
abstraction; one implementation only proves it compiles.**

---

## P2-01 · The `DbDialect` trait

**Owner:** Vaibhav Gupta · **Est:** 5h · File: `crates/dialect/src/lib.rs`

```rust
pub trait DbDialect: Send + Sync {
    fn name(&self) -> &'static str;

    // ---- identifiers & literals ----
    /// Quote an identifier: `"users"` (pg) / `` `users` `` (mysql later).
    fn quote_ident(&self, s: &str) -> String;
    /// Positional placeholder: `$1` (pg) / `?` (sqlite).
    fn placeholder(&self, index: usize) -> String;

    // ---- type mapping ----
    fn column_type(&self, f: &Field) -> Result<String, DialectError>;
    /// Rust type for a column, used by codegen. Differs by dialect:
    /// SQLite has no native UUID, so `Uuid` still maps to Rust `Uuid` but is
    /// stored as TEXT and needs an encode/decode shim.
    fn rust_type(&self, f: &Field) -> RustType;

    // ---- DDL ----
    fn create_table(&self, m: &Model) -> Vec<Stmt>;
    fn drop_table(&self, table: &str) -> Vec<Stmt>;
    fn add_column(&self, m: &Model, f: &Field) -> Vec<Stmt>;
    fn drop_column(&self, table: &str, col: &str) -> Vec<Stmt>;
    fn alter_column(&self, m: &Model, from: &Field, to: &Field) -> Vec<Stmt>;
    fn create_index(&self, m: &Model, ix: &IndexDef) -> Vec<Stmt>;
    fn drop_index(&self, table: &str, name: &str) -> Vec<Stmt>;
    fn add_foreign_key(&self, m: &Model, r: &ResolvedRelation) -> Vec<Stmt>;
    fn create_enum(&self, e: &EnumDef) -> Vec<Stmt>;
    fn alter_enum_add_variant(&self, e: &EnumDef, v: &str) -> Vec<Stmt>;

    // ---- DML fragments used by the query builder ----
    fn returning_supported(&self) -> bool;
    fn upsert_clause(&self, conflict: &[String], update: &[String]) -> String;
    fn limit_offset(&self, limit: Option<u64>, offset: Option<u64>) -> String;
    fn cast_expr(&self, expr: &str, ty: ScalarType) -> String;

    // ---- capabilities, checked at generate time ----
    fn capabilities(&self) -> Capabilities;
}

/// A single DDL statement plus metadata the migration planner needs.
pub struct Stmt {
    pub sql: String,
    pub destructive: bool,       // requires --accept-data-loss
    pub transactional: bool,     // false => must run outside a tx (pg enum alters)
    pub note: Option<String>,    // surfaced in the migration file as a comment
}

pub struct Capabilities {
    pub native_enums: bool,
    pub native_uuid: bool,
    pub alter_column_type: bool,
    pub drop_column: bool,
    pub add_fk_after_create: bool,
    pub returning: bool,
    pub partial_indexes: bool,
    pub deferrable_fks: bool,
    pub json_type: JsonSupport,   // Native | TextEncoded | None
}
```

`Stmt` carrying `destructive` and `transactional` is the piece that makes P6 (the
migration planner) tractable — the planner never has to re-derive per-dialect
knowledge about which operations are dangerous.

**Acceptance:** trait compiles, is object-safe (`Box<dyn DbDialect>` works),
documented with a "how to add a dialect" guide.

---

## P2-02 · Type mapping matrix

**Owner:** Vaibhav Gupta · **Est:** 2h

| IR type | Postgres column | SQLite column | Rust type |
|---|---|---|---|
| `String` | `TEXT` (or `VARCHAR(n)` via `@db.VarChar(n)`) | `TEXT` | `String` |
| `Int` | `INTEGER` | `INTEGER` | `i32` |
| `BigInt` | `BIGINT` | `INTEGER` | `i64` |
| `Float` | `DOUBLE PRECISION` | `REAL` | `f64` |
| `Decimal` | `NUMERIC(p,s)` | `TEXT` ⚠ | `rust_decimal::Decimal` |
| `Boolean` | `BOOLEAN` | `INTEGER` (0/1) | `bool` |
| `DateTime` | `TIMESTAMPTZ` | `TEXT` (RFC3339 UTC) | `chrono::DateTime<Utc>` |
| `Date` | `DATE` | `TEXT` | `chrono::NaiveDate` |
| `Time` | `TIME` | `TEXT` | `chrono::NaiveTime` |
| `Uuid` | `UUID` | `TEXT` | `uuid::Uuid` |
| `Json` | `JSONB` | `TEXT` | `serde_json::Value` |
| `Bytes` | `BYTEA` | `BLOB` | `Vec<u8>` |
| `enum E` | native `CREATE TYPE` | `TEXT` + `CHECK` | generated Rust enum |

⚠ **`Decimal` on SQLite is lossy-by-storage.** SQLite has no exact numeric type.
We store as `TEXT` to preserve precision and decode via `rust_decimal`'s string
form. Emit a **warning at generate time**, not silence — a user putting money in a
SQLite column deserves to be told what is happening.

**The rule that keeps us honest:** identical Rust types across dialects. The
application code must not change when you switch `provider`. Dialect differences
live entirely in storage and encode/decode shims.

---

## P2-03 · `PostgresDialect`

**Owner:** Vaibhav Gupta · **Est:** 6h

Notable behaviours:

```rust
// Enums are real types. Creating one is not transactional-safe to mix with
// inserts in older PG, and ADD VALUE cannot run inside a transaction block
// before PG 12; we target PG 14+ and still mark it non-transactional for safety.
fn create_enum(&self, e: &EnumDef) -> Vec<Stmt> {
    vec![Stmt {
        sql: format!(
            "CREATE TYPE {} AS ENUM ({})",
            self.quote_ident(&e.db_name),
            e.variants.iter()
                .map(|v| format!("'{}'", escape_literal(&v.db_name)))
                .collect::<Vec<_>>().join(", ")
        ),
        destructive: false, transactional: true, note: None,
    }]
}

fn upsert_clause(&self, conflict: &[String], update: &[String]) -> String {
    if update.is_empty() {
        format!("ON CONFLICT ({}) DO NOTHING", conflict.join(", "))
    } else {
        format!(
            "ON CONFLICT ({}) DO UPDATE SET {}",
            conflict.join(", "),
            update.iter().map(|c| format!("{c} = EXCLUDED.{c}")).collect::<Vec<_>>().join(", ")
        )
    }
}
```

Capabilities: everything `true`, `json_type: Native`, `returning: true`.

Default-value functions:
- `uuid4()` → `gen_random_uuid()` (pgcrypto is in core since PG 13)
- `uuid7()` → generated **client-side** in Rust; PG has no built-in until v18. Emit
  no DB default; the insert builder supplies the value.
- `now()` → `NOW()`
- `autoincrement()` → `GENERATED BY DEFAULT AS IDENTITY` (not `SERIAL`; identity
  columns are the modern, standard-compliant form)

---

## P2-04 · `SqliteDialect`

**Owner:** Vaibhav Gupta · **Est:** 6h

SQLite's ALTER TABLE is the hard part. Capabilities:

```rust
Capabilities {
    native_enums: false,
    native_uuid: false,
    alter_column_type: false,   // <-- drives the table-rebuild strategy
    drop_column: true,          // since 3.35, but only for simple columns
    add_fk_after_create: false, // FKs must be declared in CREATE TABLE
    returning: true,            // since 3.35
    partial_indexes: true,
    deferrable_fks: true,
    json_type: JsonSupport::TextEncoded,
}
```

**The 12-step table rebuild.** When `alter_column` is requested and
`alter_column_type == false`, emit the SQLite-sanctioned sequence:

```sql
PRAGMA foreign_keys=OFF;
CREATE TABLE "users__new" ( ...new definition... );
INSERT INTO "users__new" ("id","email") SELECT "id","email" FROM "users";
DROP TABLE "users";
ALTER TABLE "users__new" RENAME TO "users";
-- recreate every index that existed on the old table
PRAGMA foreign_key_check;
PRAGMA foreign_keys=ON;
```

This must recreate **all** indexes and triggers, and the column list in the
`INSERT ... SELECT` must be the *intersection* of old and new columns. Getting the
intersection wrong silently drops data — this function gets its own dedicated test
module with at least: add column, drop column, widen type, narrow type, add
not-null-with-default, and rename.

Enum emulation:

```sql
role TEXT NOT NULL DEFAULT 'USER'
    CHECK (role IN ('USER','ADMIN'))
```

Adding an enum variant therefore means rebuilding the CHECK constraint, i.e. a
table rebuild. Note this in the generated migration so the cost is visible.

---

## P2-05 · Dialect conformance suite

**Owner:** Vaibhav Gupta · **Est:** 5h

One test suite, run against every dialect — the mechanism that keeps SQLite from
rotting.

```rust
// crates/dialect/tests/conformance.rs
fn conformance<D: DbDialect>(d: &D) {
    // For each of the 4 example schemas:
    //  1. create_table SQL applies cleanly to a live DB
    //  2. every column round-trips a representative value of its type
    //  3. add_column / drop_column / alter_column apply and preserve rows
    //  4. indexes and FKs are queryable in the DB catalog afterwards
    //  5. capability flags match observed reality (assert what we claim)
}
```

Step 5 matters: a dialect that *claims* `alter_column_type: true` but fails at
runtime is worse than one that admits it cannot. The test asserts the claim
against the real engine.

**Acceptance:** conformance suite green for Postgres and SQLite.

---

## P2-06 · Generate-time capability diagnostics

**Owner:** Vaibhav Gupta · **Est:** 2h

Validation rule V18 from ImplPlan02 runs here, because it needs the dialect:

```
Warning: ruprizzle::dialect::degraded_type

  ⚠ `Decimal` is stored as TEXT on SQLite
   ╭─[schema.ruprizzle:22:3]
 22 │   price Decimal @db.Decimal(12, 2)
    ·         ───┬───
    ·            ╰── exact precision is preserved, but SQL-level
    ·                arithmetic and ordering on this column are lexicographic
   ╰────
  help: for money on SQLite, consider `Int` storing minor units (cents)
```

Cases to cover: `Decimal` on SQLite, enums on SQLite, `Json` querying on SQLite,
`@db.*` native-type attributes unsupported by the active provider.

---

## P2-07 · Adding a new dialect

The seam is intentionally small. To add a backend, e.g. MySQL:

1. Add a `Provider::MySql` variant in `crates/core/src/ir.rs`.
2. Implement `DbDialect` for a new `MysqlDialect` in `crates/dialect/src/mysql.rs`.
3. Implement the required methods: quoting, placeholders, `column_type`,
   `rust_type`, every DDL operation, and `capabilities()`.
4. Add the new implementation to `dialect_for` in
   `crates/dialect/src/common.rs`.
5. Add a row to `crates/dialect/tests/conformance.rs` so the backend is exercised
   against a live database.

The trait methods return `Vec<Stmt>` because one logical change (e.g. altering a
SQLite column) requires many physical statements. `Stmt` carries `destructive`
and `transactional` metadata so the migration planner can reason about cost and
safety without rediscovering dialect quirks.

### Implementation deviations from the initial sketch

- `DbDialect::create_table`, `add_column`, and `alter_column` take `&Schema` in
  addition to `&Model` so the dialect can resolve enum variants and foreign-key
  targets without re-deriving naming.
- `ResolvedRelation` gained a `target_table` field so `add_foreign_key` can emit
  the correct referenced table even when the target uses `@@map`.
- `Field::has_column` now returns `false` for `FieldKind::Relation`; the foreign
  key columns are the scalar fields named in the relation's `fields:` argument.
  This prevents the dialect from emitting duplicate `author` and `author_id`
  columns.

---

## Phase P2 checklist

- [x] P2-01 `DbDialect` trait, object-safe, documented
- [x] P2-02 type matrix implemented in both dialects
- [x] P2-03 Postgres dialect complete
- [x] P2-04 SQLite dialect complete, incl. correct 12-step rebuild
- [x] P2-05 conformance suite green on both engines — **verified against live Postgres**
- [x] P2-06 capability warnings emitted at generate time
- [x] "Adding a new dialect" guide written (proves the seam is real)
- [x] **G2 signed off by Vaibhav Gupta** — see the evidence below.

### G2 sign-off evidence (2026-08-10)

Verified: `cargo build --workspace` clean, `cargo clippy --workspace --all-targets
-- -D warnings` clean, `crates/dialect` 17 unit tests green, and `conformance.rs`
6/6 green with **both** backends actually contacted — PostgreSQL 17.10 running
locally at the default test URL plus SQLite.

This was previously signed off on skipped tests, and the mechanism is worth
recording because it will recur. `both_dbs!` prints `skipping {backend}` and
returns when a backend is unreachable, and cargo swallows stderr for passing
tests, so an unqualified `cargo test` reports "ok" whether or not Postgres was
ever reached. The tell was timing: every DB suite finished in almost exactly
5.01s, which is the connection timeout, not test work. With a live server the
same suites finish in ~0.24s.

**Guard against regression — CI must run:**

```
RUPRIZZLE_REQUIRE_DB=1 cargo test --workspace
```

`RUPRIZZLE_REQUIRE_DB=1` converts an unreachable backend from a silent skip into
a hard failure. Without it, a green build proves nothing about Postgres.
