# Known limitations

This is an honest list of what ruprizzle does and does not do. It is a
feature, not an apology: knowing the boundaries up front is how you decide
whether the tool is right for your project.

## Current alpha

- **Migrations** do not handle mutual foreign-key cycles automatically. Cycles
  must be broken by hand across migrations.
- **Heuristic renames** (detecting a column was renamed rather than dropped +
  added) are not implemented. Use `@renamedFrom` to give the diff an explicit
  hint.
- **`db push`** does not write migration files and is only for prototyping.
- **Compile-time query checking** (`sqlx-data.json` / `offline` mode) is not
  implemented.
- **No LSP** yet; syntax highlighting is available as a TextMate grammar.
- **`Decimal` on SQLite** is stored as text. If you need real decimal math on
  SQLite, use `String` and parse in application code.
- **SQLite `Json`** is stored as text and cannot be queried with JSON
  operators.

## Deferrals to 0.2

- Full LSP (completion, diagnostics, go-to-definition).
- Migration squashing.
- Support for additional databases (MySQL, MSSQL).

## When to choose something else

- Use **sqlx** directly if you want hand-written SQL and compile-time checked
  queries today.
- Use **Diesel** if you need a mature, stable query builder and are comfortable
  with its DSL.
- Use **SeaORM** if you want an active-record style API and do not need
  schema-first code generation.
