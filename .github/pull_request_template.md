## What

Brief description of the change.

## Why

Motivation and context.

## Verification

- [ ] `cargo fmt --all --check`
- [ ] `cargo clippy --workspace --all-targets -- -D warnings`
- [ ] `cargo test --workspace`
- [ ] `RUPRIZZLE_REQUIRE_DB=1 cargo test --workspace` (if the change touches runtime or migrations)
- [ ] `cargo test -p ruprizzle --features sqlite-rusqlite` (if the change touches native SQLite)
- [ ] `cargo test -p ruprizzle --features postgres-tokio-postgres` (if the change touches native Postgres)
- [ ] `cargo xtask harden`
