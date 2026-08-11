# ruprizzle-macros

[![Crates.io](https://img.shields.io/crates/v/ruprizzle-macros.svg)](https://crates.io/crates/ruprizzle-macros)
[![docs.rs](https://docs.rs/ruprizzle-macros/badge.svg)](https://docs.rs/ruprizzle-macros)
[![License](https://img.shields.io/crates/l/ruprizzle-macros.svg)](https://github.com/vaibhavgupta9877/ruprizzle-orm)

Proc-macro support for the `ruprizzle` runtime.

`ruprizzle-macros` is intentionally small. The ORM's type safety comes from generated column tokens rather than macro magic, so the only macro currently provided is `raw!`, an injection-safe way to embed raw SQL fragments into a query.

## `raw!` macro

`raw!` takes a format string with `{}` placeholders and a list of expressions. The placeholders are replaced with dialect-correct bind markers (`?` or `$1`), and the expression values are bound as parameters — they are **never interpolated** into the SQL text.

```rust
use ruprizzle::{Column, SelectQuery};
use ruprizzle::raw;

let fragment = raw!(
    "created_at > {}",
    chrono::Utc::now() - chrono::Duration::days(7)
);
```

This is useful for expressions the typed query builder does not yet expose while keeping the query safe from SQL injection.

- [Repository](https://github.com/vaibhavgupta9877/ruprizzle-orm)
- [Documentation](https://docs.rs/ruprizzle-macros)
- [Project homepage](https://vaibhavgupta9877.github.io/ruprizzle-orm)
- [Changelog](https://github.com/vaibhavgupta9877/ruprizzle-orm/blob/main/CHANGELOG.md)

## Keywords

orm, sql, database, macros, proc-macro
