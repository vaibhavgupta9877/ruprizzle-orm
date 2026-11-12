# SQL safety review

This page records how ruprizzle keeps user-controlled values out of SQL text, what was
checked, and what remains a caller responsibility. It was last reviewed on 2026-11-12
against `main` at `1.6.0` plus the changes in `CHANGELOG.md` `[Unreleased]`.

## The rules

1. **Values are bound, never spliced.** Every query value (`Value`) goes to the driver
   as a bind parameter (`$1`, `?`, and so on). Filters, inserts, updates, `LIKE`
   patterns, `IN` lists and Studio search terms all follow this rule.
2. **Identifiers come from the schema and are quoted.** Table and column names are
   taken from the parsed `.ruprizzle` schema or from database introspection, and are
   passed through `quote_ident`. That function doubles embedded quote characters
   (`"` on PostgreSQL and SQLite, `` ` `` on MySQL), so a name cannot end the identifier.
3. **Keywords and numbers are formatted from typed values.** `LIMIT`, `OFFSET`,
   isolation levels and `CASCADE` come from integers or enums, never from text.
4. **Raw SQL is an explicit opt-in.** `execute_raw`, `RawFragment` and `Filter::raw`
   take SQL text written by the developer. Their values are still bound parameters.
   Interpolating untrusted text into the SQL string itself is outside the guarantee.

## What was checked

The review covered the 41 `format!` sites that build SQL text in `crates/runtime`,
`crates/dialect`, `crates/migrate` and `crates/cli`.

| Site | Source of interpolated text | Verdict |
|---|---|---|
| `runtime/src/query.rs` `LIMIT n` / `OFFSET n` | `u64` fields on the builder | Safe: integer |
| `runtime/src/query.rs` insert read-back | Quoted model table, key and projection | Safe: quoted schema names; key value bound |
| `runtime/src/tx.rs` isolation level | `IsolationLevel::as_sql()`, a closed enum | Safe |
| `migrate/src/runner.rs` `DROP TABLE` (reset) | Quoted table names from the reset table list | Safe: quoted identifier |
| `migrate/src/change.rs`, `plan.rs`, `dialect/src/*` | Schema-derived identifiers, quoted | Safe: quoted identifiers |
| `cli/src/studio/handlers/table.rs` search | `LIKE` patterns from the search box | Safe: bound as `%term%` |
| `cli/src/studio/handlers/table.rs` `ORDER BY` / `WHERE id = ?` | Quoted primary-key column; row id bound | Safe |
| `cli/src/studio/handlers/*` `model_name` path parameter | Looked up with `schema.model(name)` before any SQL | Safe: fails closed on unknown names |
| `cli/src/studio/db.rs` counts | Quoted table and column names from the schema or introspection | Safe |
| `cli/src/seed.rs` | Seed values are bound; messages only use field names | Safe |

The remaining `format!` matches are error or display strings. They do not reach a
database driver.

## Caller responsibilities

- Do not build SQL text from request data with `execute_raw` or with `format!`.
  Pass the data through bind parameters.
- `Filter::raw` and `RawFragment` treat their literal parts as trusted SQL. Only
  placeholders are safe to fill with user data.
- Studio is a local development tool. It binds to loopback and requires its session
  token (see `docs/Operations.md`), which limits who can reach these handlers.

## Re-running the review

`cargo xtask harden` runs an injection audit that lists `format!` sites which look
like SQL. Every listed site must be checked against the table above before a release.
