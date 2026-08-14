# Design note: Decoding joined result sets

**Scope:** W2-02 explicit joins (`inner_join`, `left_join`, `right_join`, `full_join`).  
**Problem:** A join query returns the columns of *two* models in one row. The public API wants the decoded result to be `(A, B)`, `(A, Option<B>)`, etc., with the same per-model `FromRow` implementations we already generate.

## Constraints

- Models are currently decoded **by ordinal index** (see generated `FromRow` impls using `direct_idx`/`text_idx` and the native `FromOwnedRow`/`FromTokioPostgresRow` impls using `row.get(0)`). This is fast and avoids column-name ambiguity, but it assumes the model’s columns start at index `0` of the row.
- `RowDecode` is a blanket bound that requires `FromRow` for `AnyRow`, `PgRow`, `SqliteRow`, `MySqlRow`, plus native `FromOwnedRow`, `FromRusqliteRow`, and `FromTokioPostgresRow` depending on features.
- `sqlx::Row` is not object-safe, but we can implement it for a local wrapper type.
- `rusqlite::Row` (the owned `Row` in `crates/runtime/src/rusqlite.rs`) has public `values: Vec<RusqliteValue>` and `names: Vec<String>`, so we can construct a sliced view.
- `tokio_postgres::Row` cannot be constructed or sliced; its only stable accessors are `columns()` and `try_get::<usize, T>(idx)`/`try_get_by_name`.

## Options considered

### 1. Shifted row views (recommended)

Implement a local `OffsetRow<'r, R: Row>` that wraps a reference to a concrete `sqlx` row and an `offset: usize`. `OffsetRow` implements `sqlx::Row` by forwarding `columns()` to a sub-slice of the original columns and `try_get_raw` to the original row at `index + offset`.

For model decoding:

- `sqlx` rows: generated models add a generic `impl<'r, R: Row> FromRow<'r, OffsetRow<'r, R>> for M`. Since `M` is a local type in the generated crate, this is allowed. `Join2<A, B>` then decodes `A` at offset `0` and `B` at offset `A::COLUMNS.len()`.
- `rusqlite` owned `Row`: `FromOwnedRow` for `Join2` constructs a sliced `Row` containing only `B`’s values/names and calls `B::from_owned_row(&sliced)`. `FromRusqliteRow` for `Join2` first materialises the live `&RusqliteRow` into an owned `Row`, then delegates to `FromOwnedRow`.
- `tokio_postgres`: the existing `FromTokioPostgresRow` trait is extended with an optional `from_tokio_postgres_row_with_offset(row, offset)` method with a default implementation that ignores the offset (preserving backwards compatibility). Codegen is updated so generated models implement the offset-aware method; `Join2` calls `A` at offset `0` and `B` at offset `A::COLUMNS.len()`.

Pros:
- Keeps the plan’s tuple-of-models API.
- No SQL projection changes for normal queries; only join queries need to emit columns in a predictable order.
- Works with all three driver paths.

Cons:
- Requires codegen updates for `FromRow<OffsetRow>` and `FromTokioPostgresRow` offset.
- Adds a small amount of trait machinery.

### 2. Name-aliased projections

Always emit `SELECT` columns with table-qualified aliases (e.g. `users.id AS users_id`, `posts.id AS posts_id`) and teach every model to decode by alias. This would make `SELECT *` and join projections use the same path.

Pros:
- No need for index shifting.
- Duplicate column names become unambiguous.

Cons:
- Requires changing **all** existing `FromRow` implementations and `compile.rs` to use aliases in every query.
- Breaks hand-written queries that use `SELECT *` without aliases.
- Adds overhead to every single query for a feature that is opt-in.

### 3. Generated join result structs

For each declared relation, codegen produces a `UserWithPosts` struct containing all fields from both models. The public tuple API is a wrapper around this struct.

Pros:
- Simplest to decode.

Cons:
- Does not match the W2-02 spec ("tuples of model types").
- Explodes the number of generated types.
- Loses the ability to reuse `A` and `B` in different join combinations without more codegen.

### 4. Field-by-field manual decoding in a macro

A proc macro or generated `Join2` impl manually decodes each field of `A` and `B` by index.

Pros:
- No changes to model `FromRow`.

Cons:
- Requires a macro that knows the concrete fields of both models; generics cannot do this.
- Incompatible with the generic `SelectQuery::inner_join<J>` API.

## Recommendation

Adopt **Option 1 (shifted row views)**. It is the only approach that preserves the tuple API, avoids a global `FromRow` refactor, and supports all driver paths.

## Implementation plan

1. Add `crates/runtime/src/offset_row.rs`
   - `OffsetRow<'r, R: Row>` implementing `sqlx::Row`.
2. Add `crates/runtime/src/join.rs`
   - `JoinKind` enum (`Inner`, `Left`, `Right`, `Full`).
   - `JoinOn`/`JoinOnSpec` for join conditions.
   - `JoinSpec` with target table, alias, kind, and condition.
   - `JoinQuery<'db, Out>` builder.
3. Add `Join2<A, B>` / `LeftJoin2<A, B>` newtypes in `crates/runtime/src/join.rs`
   - Implement `RowDecode` (and the underlying `FromRow`/`FromOwnedRow`/`FromRusqliteRow`/`FromTokioPostgresRow`) by decoding `A` at offset `0` and `B` at `A::COLUMNS.len()`.
4. Update `SelectQuery`
   - `.inner_join<J>(on)`, `.left_join<J>(on)`, etc., returning `JoinQuery`.
5. Update `compile.rs`
   - `join_select` compiler emitting `SELECT <left>.*, <right>.* FROM <left> <join> <right> AS <alias> ON ...`.
6. Update codegen
   - Add `FromRow<'r, OffsetRow<'r, R>>` impls for generated models.
   - Update `FromTokioPostgresRow` impls to be offset-aware.
7. Tests
   - `both_dbs!` tests for inner/left joins.
   - Snapshot SQL per backend.
