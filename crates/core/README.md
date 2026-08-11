# ruprizzle-core

[![Crates.io](https://img.shields.io/crates/v/ruprizzle-core.svg)](https://crates.io/crates/ruprizzle-core)
[![docs.rs](https://docs.rs/ruprizzle-core/badge.svg)](https://docs.rs/ruprizzle-core)
[![License](https://img.shields.io/crates/l/ruprizzle-core.svg)](https://github.com/vaibhavgupta9877/ruprizzle-orm)

Shared intermediate representation (IR), diagnostics, spans, and schema fingerprints for the `ruprizzle-orm` ecosystem.

`ruprizzle-core` is the foundational crate that every other `ruprizzle-*` crate builds on. It defines the in-memory data model for a parsed `schema.ruprizzle` file: datasources, models, fields, enums, relations, providers, and native database types. It also provides the `Span` and diagnostic machinery used to emit precise, multi-error messages during parsing and validation.

## Responsibilities

- **IR types** (`ir::Schema`, `ir::Model`, `ir::Field`, `ir::Relation`, etc.) — the canonical schema model.
- **Provider and native-type descriptors** — how scalar types map to Postgres and SQLite.
- **Span tracking and source locations** — used by the parser and validator for accurate error reporting.
- **Diagnostics and suggestions** — structured errors with help text and fix hints.
- **Schema fingerprinting** — a stable hash used to detect schema drift and to version generated clients.

## When to use

You rarely depend on this crate directly unless you are building a tool on top of `ruprizzle-orm` (a custom generator, linter, or migration helper). Most users should depend on [`ruprizzle`](https://crates.io/crates/ruprizzle) or the CLI.

- [Repository](https://github.com/vaibhavgupta9877/ruprizzle-orm)
- [Documentation](https://docs.rs/ruprizzle-core)
- [Project homepage](https://vaibhavgupta9877.github.io/ruprizzle-orm)
- [Changelog](https://github.com/vaibhavgupta9877/ruprizzle-orm/blob/main/CHANGELOG.md)

## Keywords

orm, sql, database, ir, schema
