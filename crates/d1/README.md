# ruprizzle-d1

**Status: in-memory test double — not a working Cloudflare D1 adapter, and not published to crates.io.**

## What this crate actually does

`ruprizzle-d1` implements `ruprizzle::Executor` against a process-local `HashMap`.
It performs **no network I/O**: the Cloudflare D1 REST API is never called and the crate does not run inside a
Worker. Any credential passed to
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

## Connecting to Cloudflare D1 for real today

There is no supported path yet. Use the Cloudflare `wrangler` tooling or the D1
HTTP API directly until a real adapter lands.

## License

Licensed under either of MIT or Apache-2.0.
