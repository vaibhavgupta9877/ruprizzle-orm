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

## Smoke run (10 s, 8 workers)

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

## 48-hour run instructions

The W4 exit gate calls for a 48-hour sustained run. Start it from the project
root with CI logging:

```bash
RUST_LOG=info \
RUST_BACKTRACE=1 \
RUPRIZZLE_SOAK_DURATION_SECONDS=172800 \
RUPRIZZLE_SOAK_WORKERS=32 \
cargo test -p ruprizzle --test soak --features sqlite-rusqlite --release -- --nocapture
```

For the PostgreSQL variant:

```bash
export RUPRIZZLE_TEST_PG_URL=postgres://...
RUPRIZZLE_SOAK_DURATION_SECONDS=172800 \
RUPRIZZLE_SOAK_WORKERS=32 \
cargo test -p ruprizzle --test soak --release -- --nocapture
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
