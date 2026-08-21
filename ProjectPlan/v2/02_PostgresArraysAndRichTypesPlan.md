# Plan 02: Postgres Array Bind Values & Rich Native Types

**Date:** 2026-08-22  
**Author:** Vaibhav Gupta <vaibhavgupta9877@gmail.com>  
**Status:** Ready for Execution  
**Milestone:** v2.0.0-alpha.2  
**Primary Crates:** `crates/core`, `crates/dialect`, `crates/runtime`, `crates/codegen`

---

## 1. Context, Objectives & Scope

PostgreSQL developers heavily rely on native array types (`TEXT[]`, `INT[]`, `UUID[]`, `TIMESTAMP[]`) for tags, permissions/roles, and multi-tenant scopes. In `ruprizzle` 1.0, array values had partial support and were serialized to JSON strings in several SQLx encoders or lacked rich query filter operators.

### Objectives
1. **First-Class Array Binding:** Ensure `Value::Array(Vec<Value>)` serializes into native PostgreSQL array binary and text formats across `sqlx::Postgres` and native `tokio-postgres`.
2. **Rich Query Filter API:** Provide type-safe array operations on `Column<M, Vec<T>>`:
   - `.has(elem)` $\to$ `val = ANY(column)`
   - `.has_every(slice)` $\to$ `column @> ARRAY[...]`
   - `.has_some(slice)` $\to$ `column && ARRAY[...]`
   - `.is_empty()` $\to$ `cardinality(column) = 0 OR column IS NULL`
   - `.len()` / `.array_length()` $\to$ `cardinality(column)`
3. **Cross-Dialect Fallback:** On SQLite and MySQL (which lack native SQL array types), provide seamless transparent JSON array emulation via JSON1 / MySQL JSON operators (`json_contains`, `json_overlaps`, `json_length`) or strict compile-time diagnostics.

---

## 2. Technical Architecture & Design

### 2.1 Type System & IR (`crates/core`)

- `FieldKind::List(Box<FieldKind>)` already exists in `crates/core/src/ir.rs`.
- We add helper methods to `Field` and `ScalarType`:
  ```rust
  impl ScalarType {
      pub fn is_array_compatible(&self) -> bool;
      pub fn pg_array_type_name(&self) -> &'static str;
  }
  ```

### 2.2 Array Query DSL & Operations (`crates/runtime`)

In `crates/runtime/src/filter.rs` and `crates/runtime/src/col.rs`:

```rust
impl<M, T> Column<M, Vec<T>>
where
    M: Model,
    T: Encodable + 'static,
{
    /// `val = ANY(col)` or `JSON_CONTAINS(col, val)`
    pub fn has(&self, val: impl Into<T>) -> Filter<M>;

    /// `col @> ARRAY[...]` (Set containment: contains all elements)
    pub fn has_every(&self, values: impl IntoIterator<Item = T>) -> Filter<M>;

    /// `col && ARRAY[...]` (Set overlap: contains at least one element)
    pub fn has_some(&self, values: impl IntoIterator<Item = T>) -> Filter<M>;

    /// Check if array column is empty
    pub fn is_empty(&self) -> Filter<M>;

    /// Check if array column is non-empty
    pub fn is_not_empty(&self) -> Filter<M>;
}
```

### 2.3 Dialect SQL Generation (`crates/dialect`)

In `crates/dialect/src/postgres.rs`:
- `.has(x)` compiles to `param = ANY(column)`.
- `.has_every(x)` compiles to `column @> param`.
- `.has_some(x)` compiles to `column && param`.
- `.is_empty()` compiles to `(cardinality(column) = 0 OR column IS NULL)`.

In `crates/dialect/src/sqlite.rs` and `mysql.rs`:
- Uses JSON table/operator expressions (`EXISTS (SELECT 1 FROM json_each(column) WHERE value = ?)` on SQLite, `JSON_CONTAINS` on MySQL).

### 2.4 Value Encoding & Decoding (`crates/runtime`)

- In `crates/runtime/src/value.rs`:
  - Enhance `sqlx::Encode<'q, sqlx::Postgres>` for `&'q Value` when `Value::Array(items)` is provided:
    - Homogeneous type resolution (Text, Int4, Int8, Float8, Uuid, Date, Timestamp, Json).
    - Handle null element arrays gracefully.
- In `crates/runtime/src/tokio_postgres.rs`:
  - Implement `tokio_postgres::types::ToSql` for `Value::Array`.
- In `crates/runtime/src/decode.rs`:
  - Decode `Vec<T>` from Postgres array wire format and SQLite/MySQL JSON strings.

---

## 3. Step-by-Step Implementation Tasks

### Task 1: Extend Dialect Code Generation
- [ ] In `crates/dialect/src/postgres.rs`:
  - Add array operator compilation rules (`ANY`, `@>`, `&&`, `cardinality`).
- [ ] In `crates/dialect/src/sqlite.rs` & `mysql.rs`:
  - Add JSON fallback compilation rules for array operations.

### Task 2: Implement Runtime Filter Builders
- [ ] In `crates/runtime/src/filter.rs`:
  - Add `FilterOp::ArrayContains`, `FilterOp::ArrayHasEvery`, `FilterOp::ArrayHasSome`, `FilterOp::ArrayIsEmpty`.
- [ ] In `crates/runtime/src/col.rs`:
  - Implement `Column<M, Vec<T>>` operator methods.

### Task 3: Value Serialization & Tokio-Postgres Driver
- [ ] In `crates/runtime/src/value.rs`:
  - Implement typed Postgres array encoding for all primitive types (`i32`, `i64`, `f64`, `String`, `Uuid`, `DateTime<Utc>`, `NaiveDate`, `NaiveTime`).
- [ ] In `crates/runtime/src/tokio_postgres.rs`:
  - Implement `ToSql` for array values.
- [ ] In `crates/runtime/src/decode.rs`:
  - Implement `FromRow` array decoding for `Vec<String>`, `Vec<i32>`, `Vec<i64>`, `Vec<Uuid>`.

### Task 4: Integration & Proptests
- [ ] Add `crates/runtime/tests/arrays_v2.rs`:
  - Test `.has()`, `.has_every()`, `.has_some()`, `.is_empty()` round-tripping on Postgres.
  - Test JSON fallback behavior on SQLite and MySQL.
- [ ] Add property tests for arbitrary array length and random value round-trips.

---

## 4. Verification & Testing Strategy

```powershell
# 1. Workspace test suite
cargo test -p ruprizzle --test arrays_v2

# 2. Integration test across real Postgres/SQLite/MySQL
cargo test -p ruprizzle-deep-tests --test postgres_arrays

# 3. Format & Clippy
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
```

---

## 5. Definition of Done

1. `Column<M, Vec<T>>` provides type-safe `.has()`, `.has_every()`, `.has_some()`, and `.is_empty()` filter methods.
2. Full round-trip testing passing on PostgreSQL (native arrays), SQLite (JSON1 emulation), and MySQL (JSON emulation).
3. Zero memory leaks or extra heap allocations during parameter serialization.
