# ruprizzle

[![Crates.io](https://img.shields.io/crates/v/ruprizzle.svg)](https://crates.io/crates/ruprizzle)
[![docs.rs](https://docs.rs/ruprizzle/badge.svg)](https://docs.rs/ruprizzle)
[![License](https://img.shields.io/crates/l/ruprizzle.svg)](https://github.com/vaibhavgupta9877/ruprizzle-orm)

A schema-first ORM for Rust: typed queries, relations, and automatic migrations.

`ruprizzle` is the runtime crate that you use in your application. Combine a declarative `schema.ruprizzle` file with the `ruprizzle-cli` code generator and you get a fully typed query builder, migrations, and relation `include` support for Postgres and SQLite behind a single API.

## Features

- **Generated, type-safe clients** — model structs, column tokens, and the `Db` root are generated from your schema.
- **Drizzle-style query builder** — chain `.filter`, `.select`, `.order`, `.include`, `.page`, and more, and see the SQL.
- **Relations** — one-to-one, one-to-many, and nested `include` with per-relation limits.
- **Migrations** — diff your schema against the live database and generate `up.sql` / `down.sql`.
- **Raw SQL, safely** — `raw!` macro for fragments that are bound, not interpolated.
- **Transactions** — explicit `Tx::begin` / `commit` / `rollback` and isolation-level helpers.
- **Multi-backend** — the same code runs on Postgres and SQLite.

## Example

```rust
use ruprizzle::{Model, InsertQuery, SelectQuery, Column};
use generated_client::{Db, User, UserColumn};

let db = Db::connect("sqlite://app.db").await?;

let user = InsertQuery::<User>::new(db.raw_pool())
    .set(UserColumn::email, "hello@example.com")
    .exec_one()
    .await?;

let users = SelectQuery::<User>::new(db.raw_pool())
    .filter(UserColumn::email.contains("@example.com"))
    .fetch_all()
    .await?;
```

For a full getting-started guide, see the [project homepage](https://vaibhavgupta9877.github.io/ruprizzle-orm).

- [Repository](https://github.com/vaibhavgupta9877/ruprizzle-orm)
- [Documentation](https://docs.rs/ruprizzle)
- [Project homepage](https://vaibhavgupta9877.github.io/ruprizzle-orm)
- [Changelog](https://github.com/vaibhavgupta9877/ruprizzle-orm/blob/main/CHANGELOG.md)

## Keywords

orm, sql, database, postgres, sqlite
