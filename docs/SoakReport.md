# Soak test report

## Harness

`crates/runtime/tests/soak.rs` runs a configurable mixed load:

- `RUPRIZZLE_SOAK_DURATION_SECONDS` — how long to run (default `30`).
- `RUPRIZZLE_SOAK_WORKERS` — concurrent workers (default `8`).

Workers repeatedly perform one of `INSERT`, `UPDATE`, `SELECT`, or `DELETE`
on a `soak_kv` table using different keys. Every seventh operation is wrapped
in a transaction to exercise connection acquire/release cycles. Every five
seconds the harness prints:

- elapsed time
- total operations
- errors
- pool size, idle, in-use, and waiter counts
- process RSS in bytes

## Smoke run (10 s, 8 workers) — before key-cycle fix

```text
soak health: elapsed=15.9074ms ops=2 errors=0 size=4 idle=0 in_use=4 waiters=0 memory_bytes=15462400
soak health: elapsed=5.00219s ops=4873 errors=0 size=4 idle=0 in_use=4 waiters=0 memory_bytes=16437248
soak health: elapsed=10.0021929s ops=9043 errors=0 size=4 idle=0 in_use=4 waiters=0 memory_bytes=16551936
soak finished: 9051 operations, 2261 rows remaining
```

On the local SQLite file backend the test observed zero errors and stable
memory (≈ 15.5 MiB → 16.5 MiB). Pool saturation stayed within the configured
`max_connections = 4` with no waiters.

On the same hardware against a local PostgreSQL instance:

```text
soak health: elapsed=10.0003162s ops=113077 errors=0 size=4 idle=0 in_use=4 waiters=0 memory_bytes=15929344
soak finished: 113078 operations, 28266 rows remaining
```

Again zero errors and stable memory.

## Smoke run (10 s, 8 workers) — after key-cycle fix

`crates/runtime/tests/soak.rs` was fixed so each key goes through a full
`INSERT → UPDATE → SELECT → DELETE` cycle. This keeps the table size bounded
and makes the `UPDATE`/`SELECT`/`DELETE` operations meaningful.

```text
soak health: elapsed=145.2µs ops=0 errors=0 size=4 idle=0 in_use=4 waiters=0 memory_bytes=10383360
soak health: elapsed=5.0034805s ops=3648 errors=0 size=4 idle=0 in_use=4 waiters=0 memory_bytes=10952704
soak health: elapsed=10.0111943s ops=6943 errors=0 size=4 idle=0 in_use=4 waiters=0 memory_bytes=10952704
soak health: elapsed=10s ops=6951 errors=0 size=4 idle=3 in_use=1 waiters=0 memory_bytes=10977280
soak finished: 6951 operations, 6 rows remaining
```

## Native `rusqlite` smoke run (60 s, 8 workers) — after backend fixes

After the `rusqlite` backend fixes (WAL mode, 60-second busy timeout,
Condvar-based connection checkout, `tokio::task::spawn_blocking`, and a corrected
`soak.rs` that uses `fetch_all` for `SELECT`), the native `rusqlite` backend
passes a 60-second soak with zero errors:

```powershell
$env:RUPRIZZLE_TEST_RUSQLITE=1
$env:RUPRIZZLE_SOAK_DURATION_SECONDS=60
$env:RUPRIZZLE_SOAK_WORKERS=8
cargo test -p ruprizzle --test soak --features 'sqlite-rusqlite,ruprizzle-testkit/sqlite-rusqlite' --release -- sqlite --nocapture
```

```text
soak health: elapsed=60s ops=2584520 errors=0 size=0 idle=0 in_use=0 waiters=0 memory_bytes=12189696
soak finished: 2584520 operations, 6 rows remaining
```

`size`/`idle`/`in_use` are reported as `0` because `RusqlitePool` does not yet
expose pool saturation to the `Pool` metrics facade; the important signal is the
zero error count and stable memory.

## 48-hour run — root cause fixed, 1-hour validation in progress

A 48-hour SQLite `rusqlite` soak was started on 2026-08-17 19:09 UTC with the
following configuration:

```text
RUST_LOG=info
RUST_BACKTRACE=1
RUPRIZZLE_SOAK_DURATION_SECONDS=172800
RUPRIZZLE_SOAK_WORKERS=8
cargo test -p ruprizzle --test soak --features sqlite-rusqlite --release -- sqlite --nocapture
```

The process was stopped at 2026-08-17 19:56 UTC after approximately 47 minutes
because the error count was climbing monotonically:

```text
soak health: elapsed=2790.0010417s ops=384624 errors=304 size=4 idle=0 in_use=4 waiters=0 memory_bytes=3031040
soak health: elapsed=2835.0050862s ops=385688 errors=316 size=4 idle=0 in_use=4 waiters=0 memory_bytes=3031040
```

The root causes have since been fixed:

1. `crates/runtime/src/rusqlite.rs` now opens connections with `PRAGMA
   journal_mode = WAL`, a 60-second `busy_timeout`, and an explicit
   `foreign_keys = ON` setting.
2. `RusqlitePool` now uses a `Condvar`-based checkout model: `execute`,
   `fetch_all`, and `begin_transaction` wait for an available connection instead
   of returning `PoolExhausted`.
3. All synchronous `rusqlite` work is dispatched through
   `tokio::task::spawn_blocking`, so the async runtime is not pinned by
   blocking pool waits.
4. `crates/runtime/tests/soak.rs` uses `fetch_all_raw` for the `SELECT` op and
   passes the correct number of parameters for `DELETE`.

After these fixes, 60-second and 1-hour `rusqlite` runs pass with zero errors.
The full 48-hour run is the remaining W4-02 gate and will be updated here once
it completes.

## 1-hour `rusqlite` validation run

A 1-hour extended run was started with the native `rusqlite` backend to confirm
that the 60-second smoke fix scales:

```powershell
$env:RUPRIZZLE_TEST_RUSQLITE=1
$env:RUPRIZZLE_SOAK_DURATION_SECONDS=3600
$env:RUPRIZZLE_SOAK_WORKERS=8
cargo test -p ruprizzle --test soak --features 'sqlite-rusqlite,ruprizzle-testkit/sqlite-rusqlite' --release -- sqlite --nocapture
```

Final result:

```text
soak health: elapsed=3600s ops=84242039 errors=0 size=0 idle=0 in_use=0 waiters=0 memory_bytes=13008896
soak finished: 84242039 operations, 7 rows remaining
test soak_mixed_load_with_connection_churn::sqlite ... ok
```

The run completed with zero `database is locked` errors and a stable working-set
memory footprint, so the 60-second fix has been promoted to a 1-hour validation.
The 48-hour W4-02 gate is the next step.

## 48-hour run instructions

The W4 exit gate calls for a 48-hour sustained run on the native `rusqlite`
backend. The feature must be enabled on `ruprizzle-testkit` as well, or the test
harness will silently fall back to the `sqlx` SQLite path.

On PowerShell:

```powershell
$env:RUST_LOG="info"
$env:RUST_BACKTRACE=1
$env:RUPRIZZLE_TEST_RUSQLITE=1
$env:RUPRIZZLE_SOAK_DURATION_SECONDS=172800
$env:RUPRIZZLE_SOAK_WORKERS=32
cargo test -p ruprizzle --test soak --features 'sqlite-rusqlite,ruprizzle-testkit/sqlite-rusqlite' --release -- sqlite --nocapture
```

On Unix:

```bash
RUST_LOG=info \
RUST_BACKTRACE=1 \
RUPRIZZLE_TEST_RUSQLITE=1 \
RUPRIZZLE_SOAK_DURATION_SECONDS=172800 \
RUPRIZZLE_SOAK_WORKERS=32 \
cargo test -p ruprizzle --test soak --features 'sqlite-rusqlite,ruprizzle-testkit/sqlite-rusqlite' --release -- sqlite --nocapture
```

For the PostgreSQL variant:

```bash
export RUPRIZZLE_TEST_PG_URL=postgres://...
RUPRIZZLE_SOAK_DURATION_SECONDS=172800 \
RUPRIZZLE_SOAK_WORKERS=32 \
cargo test -p ruprizzle --test soak --release -- postgres --nocapture
```

Redirect `stderr` to a log and post-process with:

```bash
grep 'soak health' soak.log
```

Watch for:

- `errors` > 0 (other than the expected tail at shutdown).
- `waiters` sustained > 0 (pool exhaustion).
- `memory_bytes` growing without bound.

## Connection churn and failover

This smoke run exercises connection churn through repeated `begin`/`commit`
cycles. The planned 48-hour run should also include a forced database restart
or `Pool::close()`/`connect_with()` cycle mid-run to verify that the pool
recovers and that in-flight work receives a clean `AcquireTimeout` or
`ConnectionFailure` rather than a panic.
