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
The W4-02 48-hour gate has been waived; the resumable segmented run reached
15.56 h / 1.46 B ops / 0 errors and is recorded in the "48-hour gate — waived"
section below.

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

The 48-hour W4-02 gate was started on 2026-08-18 14:50 UTC and was later
stopped and replanned as a resumable segmented soak:

```powershell
$env:RUPRIZZLE_TEST_RUSQLITE=1
$env:RUST_BACKTRACE=1
$env:RUPRIZZLE_SOAK_DURATION_SECONDS=172800
$env:RUPRIZZLE_SOAK_WORKERS=8
cargo test -p ruprizzle --test soak --features "sqlite-rusqlite,ruprizzle-testkit/sqlite-rusqlite" --release -- sqlite --nocapture
```

The resumable harness stores state in `local/soak-48h/soak-rusqlite.db`, writes
logs to `local/soak-48h/soak.log` and `local/soak-48h/soak.err`, and has been
waived after 15.56 h / 1.46 B ops / 0 errors.

## 48-hour run instructions

> **Note:** The W4-02 48-hour gate is **waived** on the evidence already
> recorded in the "48-hour gate — waived" section below. These instructions
> remain available for optional future soak validation.

The W4 exit gate originally called for a 48-hour sustained run on the native
`rusqlite` backend. The feature must be enabled on `ruprizzle-testkit` as well,
or the test harness will silently fall back to the `sqlx` SQLite path.

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
cycles. Any future optional 48-hour run should also include a forced database restart
or `Pool::close()`/`connect_with()` cycle mid-run to verify that the pool
recovers and that in-flight work receives a clean `AcquireTimeout` or
`ConnectionFailure` rather than a panic.

## 48-hour run — terminated at ~11 hours

The 48-hour `rusqlite` run started on 2026-08-18 14:50 UTC stopped before
completion. The last health line in `logs/soak-48h-rusqlite.err` was:

```text
soak health: elapsed=40215.0007672s ops=889382385 errors=2 size=0 idle=0 in_use=0 waiters=0 memory_bytes=6320128
```

That is approximately **11 h 10 m** and **889 M operations**. The test process
(`soak-9c6b6ecac4cbf8a3.exe`, PID 31040) was no longer running at
2026-08-19 ~02:00 UTC, and the log does not contain a `soak finished` or
test-result line, so the run did not complete cleanly.

Approximately two hours into the run (`elapsed=8520s`), the harness recorded two
`soak op error: disk I/O error` events and a thread panic while printing to
stderr:

```text
soak op error: disk I/O error
thread 'soak_mixed_load_with_connection_churn::sqlite' (36088) panicked at /rustc/59807616e1fa2540724bfbac14d7976d7e4a3860/library\std\src\io\stdio.rs:1165:9:
failed printing to stderr: Insufficient system resources exist to complete the requested service. (os error 1450)
soak op error: disk I/O error
```

The error count remained at 2 for the remaining ~9 hours of the run, but the
premature termination and the I/O / system-resource events mean this is **not**
a passing 48-hour gate. The root cause was investigated, the resumable harness
was fixed, and W4-02 has been waived on 15.56 h / 0-errors evidence.

## Resumable segmented soak — W4-02 replan

Because the test machine cannot stay on for 48 continuous hours, the gate was
replanned as a resumable segmented soak:

- A new test file, `crates/runtime/tests/soak_resumable.rs`, stores cumulative
  progress in a `soak_state` table inside the same SQLite database it stresses.
- Each segment uses the persistent database in `local/soak-48h/` (workspace
  folder, not `C:` or a temp directory). The database and logs are in
  `local/soak-48h/` which is gitignored.
- `crates/testkit/src/lib.rs` now honours `RUPRIZZLE_SOAK_DB_PATH` for SQLite,
  allowing `TestDb` to use a persistent file instead of a `tempfile` temp
  directory.
- Health and error lines are written to `local/soak-48h/soak.log` and
  `local/soak-48h/soak.err`; `eprintln!` was replaced with a non-panicking
  write to avoid the `failed printing to stderr: ... (os error 1450)` crash
  that killed the original 48-hour worker.
- The runner `local/run-soak-segment.ps1` runs one segment, auto-detecting
  whether to resume from an existing database.

### 60-second verification

```powershell
$env:RUPRIZZLE_SOAK_DURATION_SECONDS=60
.\local\run-soak-segment.ps1
```

Result:

```text
soak health: elapsed=60.0021532s ops=2380046 errors=0 size=0 idle=0 in_use=0 waiters=0 memory_bytes=13283328
soak segment finished: cumulative_elapsed=60.033s ops=2380049 errors=0 rows=6; rerun with RUPRIZZLE_SOAK_RESUME=1
```

Zero errors, stable memory, and the cumulative state was persisted to the
SQLite database, so the next `run-soak-segment.ps1` invocation will resume and
add the next segment until `48 * 3600` seconds of cumulative elapsed time is
reached.

### Segmented 48-hour run instructions

> **Note:** The W4-02 gate is **waived**; these scripts remain available only
> for optional future soak validation.

```powershell
# First (or next) segment — 6 hours by default.
.\local\run-soak-segment.ps1

# Custom segment length, e.g. 1 hour.
$env:RUPRIZZLE_SOAK_DURATION_SECONDS=3600
.\local\run-soak-segment.ps1

# Repeat until the test prints `soak finished` instead of `soak segment finished`,
# or stop once the W4-02 evidence is accepted (currently waived).
```

Watch `local/soak-48h/soak.log` for `errors` > 0, `waiters` sustained > 0, or
unbounded `memory_bytes` growth.

---

## 48-hour gate — waived

**Date:** 2026-08-21

The maintainer has decided that the cumulative evidence from the resumable
segmented `rusqlite` soak is sufficient and that the remaining 32.4 % of the
48-hour W4-02 gate will not be pursued. The gate is **waived**, not failed.

### Final accepted evidence

| Metric | Value |
|---|---|
| Cumulative elapsed | **56,028.6 s (15.56 h)** of 172,800 s (48 h) — 32.4 % |
| Total operations | **1,464,277,925** |
| Total errors | **0** |
| `soak_kv` rows at last save | 5 |
| `soak.err` size | 0 bytes |
| Last state save | 2026-08-20 23:26:31 (state file last write) |
| Peak RSS | ~19.0 MiB; plateaued at ~18.2 MiB |

The earlier continuous 48-hour run that stopped at ~11 h with two `disk I/O error`
events and an `os error 1450` stderr panic has not re-occurred in the resumable
harness. The fixes that produced this clean run are WAL mode, a 60-second busy
timeout, Condvar-based connection checkout, `tokio::task::spawn_blocking`, and
non-panicking log writes. Pool saturation (`waiters=4` in ~10 % of health samples)
was observed but produced no errors.

`local/soak-48h/soak-rusqlite.db` retains the final state. The
`local/run-soak-segment.ps1` and `local/run-soak-48h.ps1` scripts remain available
for optional future soak validation, but the W4-02 release gate is closed.
