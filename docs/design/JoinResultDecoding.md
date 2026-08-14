# Design note: Decoding joined result sets

**Status:** Fully implemented for the W2-02 scope. Self-join aliasing remains a known
limitation.

**Scope:** W2-02 explicit joins (`inner_join`, `left_join`, `right_join`, `full_join`).

**Problem:** A join query returns the columns of *two* models in one row. The public API
wants the decoded result to be `(A, B)`, `(A, Option<B>)`, etc., with the same per-model
`FromRow` implementations we already generate.

## Constraints

- Models are currently decoded **by ordinal index** (see generated `FromRow` impls using
  `direct_idx`/`text_idx` and the native `FromOwnedRow`/`FromTokioPostgresRow` impls using
  `row.get(0)`). This is fast and avoids column-name ambiguity, but it assumes the model's
  columns start at index `0` of the row.
- `RowDecode` is a blanket bound that requires `FromRow` for `AnyRow`, `PgRow`, `SqliteRow`,
  `MySqlRow`, plus native `FromOwnedRow`, `FromRusqliteRow`, and `FromTokioPostgresRow`
  depending on features.
- `sqlx::Row` carries a `'static` bound **and** a `Database::Row = Self` requirement, so a
  lifetime-carrying wrapper view can never implement `sqlx::Row` directly.
- `rusqlite::Row` (the owned `Row` in `crates/runtime/src/rusqlite.rs`) has public
  `values: Vec<RusqliteValue>` and `names: Vec<String>`, so we can construct a sliced view.
- `tokio_postgres::Row` cannot be constructed or sliced; its only stable accessors are
  `columns()` and `try_get::<usize, T>(idx)` / `try_get_by_name`.

## Options considered

### 1. Shifted row views (recommended, adapted)

Implement a local `OffsetRow<'r, R: Row>` that wraps a reference to a concrete `sqlx` row and
an `offset: usize`. Because `OffsetRow` cannot itself be an `sqlx::Row`, it exposes
`try_get`/`try_get_raw` and forwards `index` to the underlying row at `index + offset`.

For model decoding:

- `sqlx` rows: generated models implement `JoinSide<R>`, a new trait with a single method
  `from_offset_row<'r>(row: &OffsetRow<'r, R>)`. `Join2<A, B>` then decodes `A` at offset `0`
  and `B` at offset `A::COLUMNS.len()`.
- `rusqlite` owned `Row`: `FromOwnedRow` for `Join2` constructs a sliced `Row` containing
  only `B`’s values/names and calls `B::from_owned_row(&sliced)`. `FromRusqliteRow` for
  `Join2` first materialises the live `&RusqliteRow` into an owned `Row`, then delegates to
  `FromOwnedRow`.
- `tokio_postgres`: `FromTokioPostgresRow` for `Join2` is currently a clear "not yet
  implemented" error. The same offset-shifting approach can be applied once the SQL-builder
  surface is in place.

Pros:

- Keeps the plan’s tuple-of-models API.
- No SQL projection changes for normal queries; only join queries need to emit columns in a
  predictable order.
- Works with all three driver paths (two are complete, one is stubbed).

Cons:

- Requires codegen updates for `JoinSide<R>` implementations.
- Adds a small amount of trait machinery.

### 2. Name-aliased projections

Always emit `SELECT` columns with table-qualified aliases (e.g. `users.id AS users_id`,
`posts.id AS posts_id`) and teach every model to decode by alias. This would make `SELECT *`
and join projections use the same path.

Pros:

- No need for index shifting.
- Duplicate column names become unambiguous.

Cons:

- Requires changing **all** existing `FromRow` implementations and `compile.rs` to use aliases
  in every query.
- Breaks hand-written queries that use `SELECT *` without aliases.
- Adds overhead to every single query for a feature that is opt-in.

### 3. Generated join result structs

For each declared relation, codegen produces a `UserWithPosts` struct containing all fields
from both models. The public tuple API is a wrapper around this struct.

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

Adopt **Option 1 (shifted row views)**. It is the only approach that preserves the tuple API,
avoids a global `FromRow` refactor, and supports all driver paths. The concrete realisation is
slightly different from the first sketch: `OffsetRow` is a helper view rather than an
`sqlx::Row`, and `JoinSide<R>` is the offset-aware decode trait instead of a blanket
`FromRow<OffsetRow<'r, R>>`.

## Implementation status

Done:

1. `crates/runtime/src/offset_row.rs`
   - `OffsetRow<'r, R: Row>` with `try_get` / `try_get_raw` and `ColumnIndex<OffsetRow>` for
     `usize` and `&str`.
2. `crates/runtime/src/join.rs`
   - `JoinSide<R: Row>` trait.
   - `Maybe<B>`, `Join2<A, B>`, `LeftJoin2<A, B>` newtypes.
   - `Maybe<B>` now implements `Model` and `JoinSide<R>` so it can appear as a nullable
     side in `right_join` and `full_join` results.
   - `sqlx::FromRow` impls for `AnyRow`, `PgRow`, `SqliteRow`, `MySqlRow`.
   - `FromOwnedRow` / `FromRusqliteRow` impls for the `sqlite-rusqlite` feature.
   - `FromTokioPostgresRow` stubs ("not yet implemented").
3. `JoinKind` enum, `JoinOn` condition type, and `JoinSpec` in
   `crates/runtime/src/join.rs`.
4. `SelectQuery::inner_join` / `left_join` / `right_join` / `full_join` returning
   `Join2`, `LeftJoin2`, and `Maybe`-wrapped sides.
5. `OffsetRow::as_raw` and `OffsetRow::offset` accessors so generated `JoinSide`
   implementations can reuse the existing decode helpers.
6. `compile.rs` `join_select` compiler emitting
   `SELECT <left>.*, <right>.* FROM <left> <join> <right> AS <alias> ON ...`.
7. Codegen: per-dialect `JoinSide<R>` implementations for generated models.
8. Tests: `both_dbs!` coverage for inner and left joins, and SQL unit tests for all four
   join kinds.

Remaining:

- Self-joins with table aliasing: an `inner_join_aliased` helper exists, but a fully
  type-safe self-join DSL (where the right-hand `Column` tokens are bound to the alias)
  is left as future work.
