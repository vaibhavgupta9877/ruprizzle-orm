# ruprizzle-neon

**Status: in-memory test double — not a working Neon serverless Postgres adapter, and not published to crates.io.**

## What this crate actually does

`ruprizzle-neon` implements `ruprizzle::Executor` against a process-local `HashMap`.
It performs **no network I/O**: no WebSocket or HTTP session is opened to a Neon endpoint. Any credential passed to
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

## Connecting to Neon serverless Postgres for real today

Neon speaks ordinary PostgreSQL over TLS. Pass the Neon connection string straight
to `ruprizzle::connect` — no adapter crate is needed.

## License

Licensed under either of MIT or Apache-2.0.
