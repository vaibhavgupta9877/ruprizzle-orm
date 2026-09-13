# Known limitations

This is an honest list of what ruprizzle does and does not do. It is a
feature, not an apology: knowing the boundaries up front is how you decide
whether the tool is right for your project.

## Current release (`1.5.1`)

- **Heuristic renames** are suggested automatically. Add `@renamedFrom` to
  confirm or ignore the prompt; the diff never renames silently.
- **`db push`** does not write migration files and is only for prototyping.
- **LSP for `schema.ruprizzle`** is available via `ruprizzle-lsp` and the VS Code
  extension in `editor/`. Syntax highlighting is also available as a TextMate
  grammar.
- **Offline query checking** (`ruprizzle check`) is available using query
  manifests captured at test time. See [ADR-012](adr/ADR-012-OfflineQueryChecking.md).
- **`Decimal` on SQLite** is stored as text by the default `sqlx::Any` path.
  The `sqlite-rusqlite` feature parses it back from text at decode time, which
  removes the `sqlx::Any` text round-trip but still stores it as text on disk.
  If you need real decimal math on SQLite, use `Int` minor units (e.g. cents)
  or a PostgreSQL backend.
- **SQLite `Json`** is stored as text, but the JSON1 extension is used for
  `json_extract`, `json_type`, and `json_set`. The `sqlite-rusqlite` feature
  also decodes `Json` without the `sqlx::Any` text round-trip. JSON containment
  (`@>`) is only a partial key-existence approximation because SQLite JSON1 has
  no containment operator.
- **Array columns (`T[]`)** are supported for scalar and enum types. PostgreSQL
  stores them as native arrays; SQLite and MySQL store them as JSON text using
  the dialect's JSON facilities. Array filter operators (`has`, `has_every`,
  `has_some`, `contains`, `contained_by`, `overlaps`, `is_empty`,
  `is_not_empty`) are implemented across all three backends.
- **Rich types through `sqlx::Any` are limited.** On SQLite, `Uuid`,
  `Decimal`, `DateTime`, `Date`, `Time`, and `Json` round-trip as text. The
  `sqlite-rusqlite` feature parses them from text at decode time, which is
  faster but still stores them as text in the database. On Postgres, the default
  `postgres://` connection uses the native `sqlx::Postgres` driver and binds
  rich types directly, so `sqlx::Any` text marshalling only applies if you
  explicitly construct `Pool::Any(...)` or use a non-default URL. The
  `postgres-tokio-postgres` feature is available as an additional native driver
  with its own performance characteristics. See
  [ADR-009](adr/ADR-009-RuntimeDialectSelection.md).
- **`SelectQuery::stream` is buffered, not a true cursor.** The current
  implementation buffers the full result set and yields decoded rows. Using
  `sqlx`'s `.fetch()` stream is **~64% slower per row** on SQLite, so the
  buffered design is deliberate. See [BenchmarkResults](BenchmarkResults.md).
- **`SelectQuery::stream_unbuffered` is available for true streaming.** It uses
  `sqlx`'s `.fetch()` cursor for the SQLx backends and a server-side portal for
  `postgres-tokio-postgres`. To satisfy `sqlx`'s lifetime model, the owned SQL
  string and bind values are leaked for the lifetime of the stream (they are
  typically small). `stream_unbuffered` must not be used inside a transaction,
  because a transaction holds a single connection and a streaming cursor would
  prevent any other statement from running on it.
- **MySQL/MariaDB carries an unpatched advisory in its auth path.** `sqlx-mysql`
  pulls in `rsa 0.9.x`, which is affected by
  [RUSTSEC-2023-0071](https://rustsec.org/advisories/RUSTSEC-2023-0071) (a Marvin
  timing side-channel in RSA decryption). No patched `rsa` release exists as of
  2026-08-21, so the exception is recorded in `deny.toml` rather than fixed. The
  side-channel is reachable only through MySQL's `caching_sha2_password` RSA key
  exchange, which is skipped when the connection uses TLS or a unix socket.
  **Use TLS or a unix socket for MySQL connections.** Until a patched `rsa` or
  `sqlx` ships, MySQL/MariaDB is supported and tested but is not marketed as
  production-grade; Postgres and SQLite carry no such exception.

## Release-specific gaps (`1.5.1`)

- **Ruprizzle Studio has no authentication.** It binds `127.0.0.1` by default and
  refuses `--allow-writes` on a non-loopback host without `--yes-i-know`; do not
  expose it on an untrusted network.
- **The Turso and D1 adapters are unverified against the live hosted services.**
  `ruprizzle-turso` (Hrana 2 over HTTP) and `ruprizzle-d1` (Cloudflare REST API)
  are tested end-to-end against local HTTP fakes, not real Turso/D1 databases.
  Both send one HTTP request per statement, so neither supports interactive
  transactions, and `stream_raw` on each is a streaming interface over a fully
  buffered response. Turso has no embedded-replica support (that needs the
  native libSQL library), and D1 has no in-Worker/`wasm32` binding.
- **There is no Neon adapter, by design.** Neon is ordinary Postgres over TLS;
  its connection string goes straight to `ruprizzle::connect`.
- **Studio templates still use `askama` 0.12.**

## Deferred / not implemented

- Polymorphic relations.
- Row-level security and multi-tenancy (`@@tenant`/`@@policy` are rejected at
  validation; they were removed from the LSP so they cannot appear to work).
- Vector / pgvector search.
- Support for additional databases (MSSQL).

## When to choose something else

- Use **sqlx** directly if you want hand-written SQL and compile-time checked
  queries today.
- Use **Diesel** if you need a mature, stable query builder and are comfortable
  with its DSL.
- Use **SeaORM** if you want an active-record style API and do not need
  schema-first code generation.
