# ruprizzle-cli

[![Crates.io](https://img.shields.io/crates/v/ruprizzle-cli.svg)](https://crates.io/crates/ruprizzle-cli)
[![docs.rs](https://docs.rs/ruprizzle-cli/badge.svg)](https://docs.rs/ruprizzle-cli)
[![License](https://img.shields.io/crates/l/ruprizzle-cli.svg)](https://github.com/vaibhavgupta9877/ruprizzle-orm)

Command-line interface for `ruprizzle-orm`.

`ruprizzle-cli` is the entry point for working with `ruprizzle-orm` outside of Rust code. It parses the `schema.ruprizzle` file, generates typed clients, validates schemas, and runs migrations against Postgres or SQLite.

## Commands

- `ruprizzle init` — scaffold a new project with a schema, `.env`, and migrations directory.
- `ruprizzle generate` — parse the schema and emit the generated Rust client.
- `ruprizzle validate` — parse and validate the schema without writing files; useful in CI.
- `ruprizzle format` — rewrite the schema file in canonical form.
- `ruprizzle migrate` — plan, apply, and inspect schema migrations.
- `ruprizzle db push` — push schema changes directly without migration files.

## Installation

```bash
cargo install ruprizzle-cli
ruprizzle --help
```

## Quick start

```bash
ruprizzle init --provider sqlite
ruprizzle generate
ruprizzle migrate dev
```

See the [project homepage](https://vaibhavgupta9877.github.io/ruprizzle-orm) for the full guide.

- [Repository](https://github.com/vaibhavgupta9877/ruprizzle-orm)
- [Documentation](https://docs.rs/ruprizzle-cli)
- [Project homepage](https://vaibhavgupta9877.github.io/ruprizzle-orm)
- [Changelog](https://github.com/vaibhavgupta9877/ruprizzle-orm/blob/main/CHANGELOG.md)

## Keywords

orm, sql, database, cli, migrations
