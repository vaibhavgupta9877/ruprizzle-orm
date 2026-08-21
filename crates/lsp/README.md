# ruprizzle-lsp

[![Crates.io](https://img.shields.io/crates/v/ruprizzle-lsp.svg)](https://crates.io/crates/ruprizzle-lsp)
[![docs.rs](https://docs.rs/ruprizzle-lsp/badge.svg)](https://docs.rs/ruprizzle-lsp)
[![License](https://img.shields.io/crates/l/ruprizzle-lsp.svg)](https://github.com/vaibhavgupta9877/ruprizzle-orm)

Language server for the `ruprizzle-orm` schema DSL.

`ruprizzle-lsp` gives `schema.ruprizzle` files the editor experience a schema-first workflow needs: errors as you type, completion for models and scalar types, go-to-definition across relations, and hover documentation. It stays deliberately small by re-using the ORM's own parser and IR — it does not generate code and never touches a database.

## Capabilities

- **Diagnostics** — the parser's own errors and spans, published on open and on change.
- **Completion** — model names, field types, attributes, and relation targets.
- **Go to definition** — jump from a relation field to the model it points at.
- **Hover** — the resolved type and attributes of the symbol under the cursor.

## Installation

```bash
cargo install ruprizzle-lsp
```

The server speaks LSP over stdio, so any conforming editor can drive it:

```bash
ruprizzle-lsp
```

A ready-made VS Code extension lives in [`editor/`](https://github.com/vaibhavgupta9877/ruprizzle-orm/tree/main/editor) in the repository.

## Library use

The crate also exposes its `Backend` so the server can be embedded in another `tower-lsp` host rather than spawned as a process.

- [Repository](https://github.com/vaibhavgupta9877/ruprizzle-orm)
- [Documentation](https://docs.rs/ruprizzle-lsp)
- [Project homepage](https://vaibhavgupta9877.github.io/ruprizzle-orm)
- [Changelog](https://github.com/vaibhavgupta9877/ruprizzle-orm/blob/main/CHANGELOG.md)

## Keywords

orm, database, lsp, language-server
