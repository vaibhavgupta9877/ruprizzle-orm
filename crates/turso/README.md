# ruprizzle-turso

**Status: in-memory test double — not a working Turso / libSQL adapter, and not published to crates.io.**

## What this crate actually does

`ruprizzle-turso` implements `ruprizzle::Executor` against a process-local `HashMap`.
It performs **no network I/O**: no libSQL connection is opened, no Turso primary is contacted, no local
replica file is read or written, and `sync()` always returns an error. Any credential passed to
the builder is stored and never transmitted. Every value written through it is lost
when the process exits.

It exists so code written against the `Executor` trait can be exercised without a
database, and so the shape of a future real adapter is pinned down. `publish = false`
is set in `Cargo.toml`; do not use it as a production backend.

## Supported subset

- `SELECT ... FROM <table>` returns every row previously inserted for `<table>`.
  Filters, joins, ordering and limits are ignored.
- `INSERT INTO <table> (a, b) VALUES (...)` appends one row, naming the bound values
  after the declared column list (`col_0`, `col_1`, ... when none is declared).
- Every other statement is accepted and discarded.

## Connecting to Turso / libSQL for real today

Point `ruprizzle::connect` at the SQLite file of an embedded replica you
synchronise yourself with the `turso` CLI or the libSQL client.

## License

Licensed under either of MIT or Apache-2.0.
