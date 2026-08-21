# ruprizzle-check

[![Crates.io](https://img.shields.io/crates/v/ruprizzle-check.svg)](https://crates.io/crates/ruprizzle-check)
[![docs.rs](https://docs.rs/ruprizzle-check/badge.svg)](https://docs.rs/ruprizzle-check)
[![License](https://img.shields.io/crates/l/ruprizzle-check.svg)](https://github.com/vaibhavgupta9877/ruprizzle-orm)

Offline schema and query validation for `ruprizzle-orm`.

`ruprizzle-check` validates captured queries and `raw!` SQL fragments against a `schema.ruprizzle` file **without a live database**. That makes it usable in CI, in a pre-commit hook, and on a laptop with no connection — the same class of check `sqlx` gets from `cargo sqlx prepare`, but driven by the schema rather than a saved query cache.

## Responsibilities

- **Query manifest** — read the `QueryManifest` that the runtime and macros emit, listing every query the crate builds and where it came from.
- **Manifest validation** — check each captured query's tables, columns, and relations against the parsed schema, reporting unknown names with their source spans.
- **`raw!` fragment validation** — validate the SQL passed to the `raw!` macro against the same schema, so hand-written fragments do not silently drift when a model is renamed.

## Example

```rust
use ruprizzle_check::{QueryManifest, validate_manifest};

let source = std::fs::read_to_string("schema.ruprizzle")?;
let schema = ruprizzle_parser::parse("schema.ruprizzle", &source)?;

let manifest: QueryManifest =
    serde_json::from_str(&std::fs::read_to_string("target/ruprizzle/queries.json")?)?;

for error in validate_manifest(&schema, &manifest) {
    eprintln!("{error}");
}
```

Most users reach this through the CLI rather than the library:

```bash
ruprizzle check
```

- [Repository](https://github.com/vaibhavgupta9877/ruprizzle-orm)
- [Documentation](https://docs.rs/ruprizzle-check)
- [Project homepage](https://vaibhavgupta9877.github.io/ruprizzle-orm)
- [Changelog](https://github.com/vaibhavgupta9877/ruprizzle-orm/blob/main/CHANGELOG.md)

## Keywords

orm, database, sql, validation
