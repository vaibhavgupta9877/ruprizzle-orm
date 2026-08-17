# ADR-012: Offline query checking

## Status

Accepted.

## Context

The runtime query builder and `raw!` macro produce SQL at compile time or at
run time. Today those strings are only validated by a live database during
execution. We want CI to catch mistakes — unknown tables, misspelled columns,
malformed SQL fragments — before the application is deployed, without requiring
a database connection.

## Decision

Use a manifest of captured SQL plus a schema snapshot:

- `RUPRIZZLE_RECORD_QUERIES=1` makes the runtime record every `to_sql()` output
  into an in-memory buffer.
- `ruprizzle_check::write_manifest` serialises the buffer to
  `query-manifest.json`.
- `RUPRIZZLE_OFFLINE_SCHEMA=path/to/schema.ruprizzle` makes the `raw!` macro
  parse the schema at compile time and reject unknown table/column references
  with a `syn` error.
- `ruprizzle check --schema schema.ruprizzle --manifest query-manifest.json`
  validates captured queries against the parsed schema.

The validation is intentionally coarse: it tokenises the SQL and checks
identifiers against model/table and column names. It is not a full SQL parser;
that would be brittle and re-invent `sqlx`'s query analysis. The goal is to
 catch obvious typos and drift, not prove query correctness.

## Consequences

- CI can run `RUPRIZZLE_RECORD_QUERIES=1 cargo test && ruprizzle check ...` to
  detect schema drift.
- `raw!` fragments gain compile-time guard rails when `RUPRIZZLE_OFFLINE_SCHEMA`
  is set.
- The validation does not understand aliases, CTEs, or subqueries in depth.
  False negatives are expected for complex SQL.
