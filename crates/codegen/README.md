# ruprizzle-codegen

[![Crates.io](https://img.shields.io/crates/v/ruprizzle-codegen.svg)](https://crates.io/crates/ruprizzle-codegen)
[![docs.rs](https://docs.rs/ruprizzle-codegen/badge.svg)](https://docs.rs/ruprizzle-codegen)
[![License](https://img.shields.io/crates/l/ruprizzle-codegen.svg)](https://github.com/vaibhavgupta9877/ruprizzle-orm)

Rust entity and query-builder code generation for `ruprizzle-orm`.

`ruprizzle-codegen` takes the validated schema IR produced by [`ruprizzle-parser`](https://crates.io/crates/ruprizzle-parser) and emits the typed client code that powers the `ruprizzle` runtime. This includes model structs, column tokens, relation accessors, insert/update types, and the `Db` root struct that a generated project imports.

## Responsibilities

- **Model code generation** — structs, fields, and `Model` trait implementations.
- **Column tokens** — type-safe column references used in filters and projections.
- **Relation code** — `include` helpers, one-to-one and one-to-many accessors, and back-references.
- **Insert / update types** — typed builders for creating and modifying rows.
- **`Db` root** — the generated client entry point (`Db::connect`, repositories, transactions).
- **Stable output** — generated code is deterministic and lint-clean.

## When to use

You do not typically depend on this crate directly. The [`ruprizzle-cli`](https://crates.io/crates/ruprizzle-cli) `generate` command and the build-time pipeline call it for you. Advanced users who want to embed code generation into their own tools can use the `generate_all` API.

- [Repository](https://github.com/vaibhavgupta9877/ruprizzle-orm)
- [Documentation](https://docs.rs/ruprizzle-codegen)
- [Project homepage](https://vaibhavgupta9877.github.io/ruprizzle-orm)
- [Changelog](https://github.com/vaibhavgupta9877/ruprizzle-orm/blob/main/CHANGELOG.md)

## Keywords

orm, sql, database, codegen, migrations
