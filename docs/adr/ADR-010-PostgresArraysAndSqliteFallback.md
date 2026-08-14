# ADR-010 — Postgres arrays and a SQLite JSON fallback

## Context

`Value::Array` exists in the runtime but is rejected at bind time on every
backend. Postgres arrays are a headline feature (`text[]`, `int4[]`, `uuid[]`);
SQLite has no native array type. We need a design that is honest about backend
capability rather than silently divergent.

## Decision

1. **Postgres gets native arrays.** When the runtime is talking to Postgres
   through either `sqlx::Postgres` or `tokio-postgres`, `Value::Array` binds as a
   Postgres array of the element type. Nested arrays and mixed element types are
   not supported; the array must be one-dimensional and homogeneous.

2. **SQLite uses an explicit JSON fallback.** `Value::Array` bound to SQLite is
   encoded as a JSON string and decoded back at read time. The schema side
   records this as a `text[]` column that stores JSON. This makes the
   degradation explicit: users see `text[]` in the schema, but the data on disk
   is JSON text.

3. **`sqlx::Any` + Postgres still errors until supported.** `sqlx::Any` does not
   expose a typed array argument in the tested version. Instead of the bare
   "array bind values are not supported yet" message, we return an actionable
   `Error` telling the user to use the native Postgres feature
   (`postgres-tokio-postgres` or `sqlx::Postgres` directly) for arrays.

4. **Schema-side `String[]`, `Int[]`, etc.** The schema parser already accepts
   `T[]` field notation. Scalar lists are allowed for primitive scalar types and
   enums. They map to Postgres native arrays where possible and to the JSON
   fallback on SQLite.

5. **Filter operators for Postgres.** For one-dimensional Postgres arrays we
   support `contains` (`@>`), `contained_by` (`<@`), and `overlaps` (`&&`). These
   are only meaningful on Postgres; on SQLite the JSON fallback cannot be
   indexed and the operators are not supported.

## Consequences

- Postgres users get first-class arrays. Migration DDL emits `text[]`,
  `int4[]`, etc.
- SQLite users can still model `T[]` columns, but the data is stored as JSON.
  The same Rust type (`Vec<T>`) works on both backends.
- `sqlx::Any` users that hit Postgres get a clear error rather than a confusing
  runtime rejection.
- The query builder gains typed array operators behind `Column<M, Vec<T>>`.
