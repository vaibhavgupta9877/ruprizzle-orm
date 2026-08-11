# ruprizzle-migrate

[![Crates.io](https://img.shields.io/crates/v/ruprizzle-migrate.svg)](https://crates.io/crates/ruprizzle-migrate)
[![docs.rs](https://docs.rs/ruprizzle-migrate/badge.svg)](https://docs.rs/ruprizzle-migrate)
[![License](https://img.shields.io/crates/l/ruprizzle-migrate.svg)](https://github.com/vaibhavgupta9877/ruprizzle-orm)

Schema diffing, migration planning, and application for `ruprizzle-orm`.

`ruprizzle-migrate` compares the current database schema against the target schema and produces a set of SQL migration steps. It supports both Postgres and SQLite, and handles constraints, index changes, type changes, and table rebuilds for backends with limited `ALTER TABLE` support.

## Responsibilities

- **Schema diffing** — compute the difference between an in-memory `Schema` and the live database.
- **Migration planning** — turn a diff into ordered, backend-specific SQL statements.
- **Migration application** — run `up.sql`, `down.sql`, and idempotency checks against a database.
- **Statement splitting** — safely split migration files while respecting dollar-quoted strings and string literals.
- **Checksums** — detect modified migrations and warn before applying.

## Example

```rust
use ruprizzle_migrate::{diff, up_sql};
use ruprizzle_dialect::PostgresDialect;

let current = /* schema in the database */;
let target  = /* desired schema */;
let dialect = PostgresDialect;

let changes = diff(&current, &target, &dialect);
let sql = up_sql(&changes, &dialect, &target);
```

Most users run migrations through the [`ruprizzle-cli`](https://crates.io/crates/ruprizzle-cli) `migrate` commands rather than calling the library directly.

- [Repository](https://github.com/vaibhavgupta9877/ruprizzle-orm)
- [Documentation](https://docs.rs/ruprizzle-migrate)
- [Project homepage](https://vaibhavgupta9877.github.io/ruprizzle-orm)
- [Changelog](https://github.com/vaibhavgupta9877/ruprizzle-orm/blob/main/CHANGELOG.md)

## Keywords

orm, sql, database, migrations, schema
