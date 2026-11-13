# What's new in 1.7

`1.7.0` is additive over `1.6.0`. It has no breaking changes. The one code change that
can affect you is that a missing row now returns a typed error. The full list is in the
[changelog](https://github.com/vaibhavgupta9877/ruprizzle-orm/blob/main/CHANGELOG.md).

```toml
[dependencies]
ruprizzle = "1.7"
```

## Synchronous SQLite reads (native rusqlite backend)

With `driver=rusqlite` you can run a query on the calling thread, with no `spawn_blocking`
hop and no async state machine. This is the fastest path for point reads from synchronous
code.

```rust
let user = SelectQuery::<User>::new(&pool)
    .filter(User::id.eq(1))
    .fetch_one_blocking()?;
let n = SelectQuery::<User>::new(&pool).count_blocking()?;
let any = SelectQuery::<User>::new(&pool).exists_blocking()?;
```

Available: `fetch_all_blocking`, `fetch_optional_blocking`, `fetch_one_blocking`,
`count_blocking` and `exists_blocking`.

- **Blocking:** these park the calling thread for the whole statement. Use them from
  synchronous code, or from work already on a blocking thread. **Do not call them from an
  async task.** A debug build cannot detect this reliably, so the rule is documentation only.
- **Scope:** they need the native rusqlite pool (`connect` with `driver=rusqlite`) and
  return an error on other backends. They do not support `.include(...)`.

Measured on the 2026-11-12 run (see [BenchmarkResults](BenchmarkResults.md)): synchronous
reads sit at the raw rusqlite floor for 10,000-row fetches.

## Typed not-found

`fetch_one`, `fetch_one_blocking`, `exec_one` and `Tx::fetch_one` now return
`Error::NotFound` (kind `not_found`) when no row matches. Before, they returned
`Error::Message` with the same text.

```rust
match SelectQuery::<User>::new(&pool).filter(User::id.eq(id)).fetch_one().await {
    Ok(user) => { /* ... */ }
    Err(ruprizzle::Error::NotFound) => { /* missing, not broken */ }
    Err(other) => return Err(other),
}
```

**Upgrade:** if you match on `Error::Message` to detect missing rows, switch to
`Error::NotFound`. The message text is unchanged.

## Observability fix (native rusqlite)

Reads on `driver=rusqlite` were missing their `db.query` span, query metrics and
slow-query warning. Since 1.6.0 they are emitted again, on the async and synchronous
paths alike. If you alert on query metrics for a rusqlite deployment, the counts will
rise to match what really ran.

## Security

- `rustls` 0.23.46, `rustls-webpki` 0.103.16 and `webpki-roots` 1.0.12 (advisories
  RUSTSEC-2026-0346, -0347, -0381, -0382). These reach users through the Turso and D1
  adapters.
- Two documented `cargo deny` exceptions, each with a reason in `deny.toml`:
  - RUSTSEC-2026-0459 (bundled SQLite before 3.54.0). The fix needs the sqlx 0.9 upgrade,
    planned for 2.0.0.
  - RUSTSEC-2026-0385 (`ctutils`, non-constant-time mask generation, via `digest`). The
    fix needs Rust 1.87; the workspace MSRV is 1.85. It is removed when the MSRV rises.

## CI

- The `mysql` feature-matrix job no longer sets `RUPRIZZLE_REQUIRE_DB`, because it has no
  Postgres URL. It was failing on every push.
- CI on the pushed tip covers 34 jobs, including the Postgres, MySQL and MariaDB
  integration legs, both native drivers, the Studio feature, semver checks and public API
  diffs.

## Upgrading from 1.6.0

No code changes, unless you match on `Error::Message` for missing rows. See
[UpgradingFromRc1.md](UpgradingFromRc1.md) for the older upgrade path.
