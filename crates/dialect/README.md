# ruprizzle-dialect

[![Crates.io](https://img.shields.io/crates/v/ruprizzle-dialect.svg)](https://crates.io/crates/ruprizzle-dialect)
[![docs.rs](https://docs.rs/ruprizzle-dialect/badge.svg)](https://docs.rs/ruprizzle-dialect)
[![License](https://img.shields.io/crates/l/ruprizzle-dialect.svg)](https://github.com/vaibhavgupta9877/ruprizzle-orm)

SQL dialect abstraction and Postgres + MySQL/MariaDB + SQLite implementations for `ruprizzle-orm`.

`ruprizzle-dialect` turns the ORM's intermediate representation into database-specific SQL. It hides the differences between Postgres, MySQL/MariaDB, and SQLite behind a single `DbDialect` trait so that higher-level crates (codegen, migrate, runtime) can generate SQL without coupling to a specific backend.

## Responsibilities

- **`DbDialect` trait** — the common interface for SQL generation.
- **Postgres dialect** — native `CREATE TABLE`, `ALTER TABLE`, enum, and index statements.
- **MySQL/MariaDB dialect** — MySQL DDL, `ON DUPLICATE KEY UPDATE`, and portable migration statements.
- **SQLite dialect** — table rebuilds, `ALTER TABLE` emulation, and check constraints for enum-like behaviour.
- **Type mapping** — translate `ruprizzle` scalar kinds into native SQL types for each backend.
- **Capability checks** — warn when a schema uses features a backend cannot support (e.g. `Decimal` on SQLite).

## Example

```rust
use ruprizzle_core::ir::Schema;
use ruprizzle_dialect::{SqliteDialect, full_create_table};

let schema: Schema = /* parsed schema */;
let dialect = SqliteDialect;
for stmt in full_create_table(&dialect, &schema, &schema.models["User"]) {
    println!("{}", stmt.sql);
}
```

Most users do not call the dialect directly. The [`ruprizzle`](https://crates.io/crates/ruprizzle) runtime and the CLI use it internally.

- [Repository](https://github.com/vaibhavgupta9877/ruprizzle-orm)
- [Documentation](https://docs.rs/ruprizzle-dialect)
- [Project homepage](https://vaibhavgupta9877.github.io/ruprizzle-orm)
- [Changelog](https://github.com/vaibhavgupta9877/ruprizzle-orm/blob/main/CHANGELOG.md)

## Keywords

orm, sql, database, postgres, mysql, sqlite
