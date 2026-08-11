# ruprizzle-parser

[![Crates.io](https://img.shields.io/crates/v/ruprizzle-parser.svg)](https://crates.io/crates/ruprizzle-parser)
[![docs.rs](https://docs.rs/ruprizzle-parser/badge.svg)](https://docs.rs/ruprizzle-parser)
[![License](https://img.shields.io/crates/l/ruprizzle-parser.svg)](https://github.com/vaibhavgupta9877/ruprizzle-orm)

Grammar-driven `.ruprizzle` schema parser with span-preserving diagnostics.

`ruprizzle-parser` turns a `schema.ruprizzle` source string into the typed intermediate representation defined in [`ruprizzle-core`](https://crates.io/crates/ruprizzle-core). It is built on a PEG grammar (using `pest`) and produces rich, location-aware errors so that mistakes in a schema are reported at the exact line and column with actionable help text.

## Responsibilities

- **Schema parsing** — parse `datasource`, `generator`, `model`, `enum`, and relation declarations.
- **Lowering** — convert the raw AST into the canonical `ir::Schema` used by the rest of the ORM.
- **Validation** — enforce rules such as unique model names, valid relation arity, and supported native types.
- **Error reporting** — emit structured, multi-error diagnostics with source spans and suggestions.

## Example

```rust
use ruprizzle_parser::parse;

let source = r#"
    datasource db {
        provider = "sqlite"
        url      = env("DATABASE_URL")
    }

    model User {
        id    Int    @id @default(autoincrement())
        email String @unique
    }
"#;

let schema = parse("schema.ruprizzle", source).expect("valid schema");
```

Most users do not call the parser directly; the [`ruprizzle-cli`](https://crates.io/crates/ruprizzle-cli) and code generator handle this for you.

- [Repository](https://github.com/vaibhavgupta9877/ruprizzle-orm)
- [Documentation](https://docs.rs/ruprizzle-parser)
- [Project homepage](https://vaibhavgupta9877.github.io/ruprizzle-orm)
- [Changelog](https://github.com/vaibhavgupta9877/ruprizzle-orm/blob/main/CHANGELOG.md)

## Keywords

orm, sql, database, parser, schema
