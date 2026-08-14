# Known limitations

This is an honest list of what ruprizzle does and does not do. It is a
feature, not an apology: knowing the boundaries up front is how you decide
whether the tool is right for your project.

## Current beta

- **Heuristic renames** are suggested automatically. Add `@renamedFrom` to
  confirm or ignore the prompt; the diff never renames silently.
- **`db push`** does not write migration files and is only for prototyping.
- **Compile-time query checking** (`sqlx-data.json` / `offline` mode) is not
  implemented.
- **No LSP** yet; syntax highlighting is available as a TextMate grammar.
- **`Decimal` on SQLite** is stored as text. If you need real decimal math on
  SQLite, use `String` and parse in application code.
- **SQLite `Json`** is stored as text, but the JSON1 extension is used for
  `json_extract`, `json_type`, and `json_set`. JSON containment (`@>`) is only
  a partial key-existence approximation because SQLite JSON1 has no containment
  operator.
- **Postgres arrays** cannot be used as bind values. `Value::Array` is rejected
  at runtime.
- **Rich types through `sqlx::Any` are limited.** On SQLite, `Uuid`,
  `Decimal`, `DateTime`, `Date`, `Time`, and `Json` round-trip as text. The
  `sqlite-rusqlite` feature parses them from text at decode time, which is
  faster but still stores them as text in the database. On Postgres, the
  `postgres-tokio-postgres` feature decodes native types directly; without it,
  `sqlx::Any` may not decode several rich types. See
  [ADR-009](../ProjectPlan/ImplementationPlan/ImplPlan10AppendixDecisions.md).
- **`SelectQuery::stream` is buffered, not a true cursor.** The current
  implementation buffers the full result set and yields decoded rows. Using
  `sqlx`'s `.fetch()` stream is **~64% slower per row** on SQLite, so the
  buffered design is deliberate. See [BenchmarkResults](BenchmarkResults.md).

## Deferrals to 0.2

- Full LSP (completion, diagnostics, go-to-definition).
- Support for additional databases (MSSQL).

## When to choose something else

- Use **sqlx** directly if you want hand-written SQL and compile-time checked
  queries today.
- Use **Diesel** if you need a mature, stable query builder and are comfortable
  with its DSL.
- Use **SeaORM** if you want an active-record style API and do not need
  schema-first code generation.
