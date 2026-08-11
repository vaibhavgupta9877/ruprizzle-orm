# Performance enhancements, round 2

**Status:** in progress — Phase A (default `test_before_acquire=false` and `Executor` trimming) and Phase B (native Postgres/Sqlite backends) implemented; Phase C and D still open.
**Date:** 2026-08-11
**Baseline:** `e513f7e` (post Phase 1 + P2-3), `docs/BenchmarkResults.md`
**Supersedes the attribution in:** [`Enhancements1.md`](Enhancements1.md) §3

---

## 1. Executive summary

Round 1 fixed the ORM layer and concluded that the rest of the gap was
"`sqlx` itself — architectural, unreachable." The fixes landed (`LIMIT 1`,
AST-compiled `count`/`exists`, include type-safety, ordinal decode) and
`find_many_1000` moved 1,687 → 1,648 µs. Against Drizzle's 320 µs that is
still 5×, which is why the numbers still disappoint.

**Round 1's conclusion was half right, and the wrong half was the expensive
half.** The per-row cost *is* `sqlx-sqlite`. But it is not architectural, and
the floor is not Drizzle.

A `rusqlite` connection driven from Tokio through `spawn_blocking` — a design
ruprizzle can actually ship — decodes the same rows into the same owned structs
at **0.121 µs/row against `sqlx-sqlite`'s 1.63 µs/row.** That is **13.5×**, and
it is **2.6× faster than Drizzle**, not merely competitive with it.

Round 1 attributed the cost to "thread-per-connection plus a per-row channel"
and treated both as immovable. Layer 3 of §3.2 separates them: a
dedicated long-lived thread with a channel — *exactly* `sqlx-sqlite`'s
architecture — still reaches 0.133 µs/row. **The thread model is not the cost.
The per-row handoff is.** `sqlx-sqlite` materialises one `SqliteRow` per row and
sends each one individually through a `flume` channel bounded at 50. Send the
result set across once instead of ten thousand times and the cost disappears.

And it is not a single-threaded artefact: run both drivers at 1–16 concurrent
tasks and the gap **widens — never below 11× in any cell of two runs** —
because `sqlx-sqlite`'s throughput *falls* under load (≈690 → ≈195 q/s) while
`rusqlite`'s holds up (§3.2b). That measurement was written into this plan as a
go/no-go gate on the whole recommendation. It passes.

Two things follow:

1. There is a **13×** win available on SQLite, gated on replacing
   `sqlx-sqlite` — not on any change to ruprizzle's own code.
2. There is a **separate, unrelated, ~10-line win** that round 1 missed
   entirely: `PoolConfig::default()` sets `test_before_acquire: true`, which
   makes `sqlx` ping the connection before **every** checkout. That costs
   +10.2 µs on every SQLite query (34% of `select_by_pk`), and on Postgres it
   is a **full network round-trip per query** — milliseconds on a hosted
   database. This is almost certainly the single largest real-world latency
   item in the project, and it is a default value.

| | now | after Phase A | after Phase B | after Phase C |
|---|---:|---:|---:|---:|
| `select_by_pk` | 41.2 µs | ~31 µs | ~29 µs | **~27 µs** |
| `find_many_1000` | 1,648 µs | 1,640 µs | ~1,240 µs | **~150 µs** |
| `include_posts` | 16.7 ms | 16.7 ms | ~12.6 ms | **~1.6 ms** |
| Postgres, hosted | +1 RTT/query | **baseline** | −16%/row | n/a |

Phase A is hours. Phase B is the work round 1 already scoped. Phase C is a new,
larger project and the only one that changes the headline number.

---

## 2. Method

Same discipline as round 1: hand-written controls, warm-ups, repeats, ranges
rather than point estimates. Two changes:

- **`rusqlite` is now a control.** Round 1 used better-sqlite3 as the reference
  floor, which conflates "what a synchronous driver achieves" with "what
  another language achieves". `rusqlite` isolates the driver variable. It is a
  dev-dependency pinned to `0.32`, which shares `libsqlite3-sys 0.30.1` with
  `sqlx-sqlite`, so no second copy of SQLite is compiled and there is no
  link-time divergence.
- **Paired interleaving** where two variants are compared directly
  (`pool_config.rs`), so thermal drift and scheduler noise hit both arms
  equally instead of favouring whichever ran first. Round 1 lost a day to a
  sign flip caused by exactly this.

Every layer materialises the same owned structs (`i64`, `String`, `i64`). No
layer is credited for skipping work.

**Environment:** Windows 11, release profile, local SQLite file
(`local/cross-orm-bench/node/bench.sqlite3`, 1,000 users / 10,000 posts),
5–7 timed repeats per measurement.

Harnesses (all in `crates/runtime/examples/`, all runnable):

| harness | answers |
|---|---|
| `perrow.rs` | per-row throughput at five layers, min/median/max |
| `blocking_floor.rs` | is the `rusqlite` floor reachable from async? |
| `pool_config.rs` | what do `PoolConfig` defaults cost? |
| `bottlenecks.rs` | `Any` conversion, two-pass decode, include breakdown |
| `concurrency.rs` | does the `rusqlite` advantage survive concurrent load? (task C-0) |

Round 1's harnesses (`layer_attribution`, `sqlx_floor`, `include_breakdown`,
`hotspots`, `row_buffer`, `pg_any_types`) still apply and were re-run.

---

## 3. Measured attribution

### 3.1 The floor, and where we sit relative to it

`perrow.rs`, median of 5, µs/row:

| layer | users (1k) | posts (10k) | `select_by_pk` |
|---|---:|---:|---:|
| 1 `rusqlite` (sync, in-process) | **0.097** | **0.105** | **9.8 µs** |
| 2 `sqlx` native `SqlitePool` | 1.404 | 1.292 | 39.9 µs |
| 3 `sqlx` native + `row_buffer_size(16384)` | 1.328 | 1.156 | — |
| 4 `sqlx` `AnyPool` | 1.859 | 1.504 | 40.6 µs |
| 5 **ruprizzle `SelectQuery`** | **1.561** | **1.409** | **44.3 µs** |
| — Drizzle + better-sqlite3 (published) | 0.320 | — | 28.8 µs |

ruprizzle is **13–16× off the Rust floor** on row-heavy reads and **4.5× off**
on point queries. better-sqlite3, the thing round 1 treated as the target, is
itself **3× slower than `rusqlite`**.

> The layer-5-faster-than-layer-4 inversion on the users row (1.561 vs 1.859)
> did not reproduce in `bottlenecks.rs` §4, where the same comparison came out
> at +2.2%. Treat rows 4 and 5 as indistinguishable; the sections run
> sequentially over ~3 minutes and drift at that scale. Nothing in the plan
> depends on the ordering of those two.

### 3.2 Is the floor reachable from an async ORM?

This is the question that decides whether §3.1 is actionable or trivia.
`blocking_floor.rs`, median of 5:

| layer | `find_many_1000` | µs/row | `select_by_pk` |
|---|---:|---:|---:|
| 1 `rusqlite` inline (blocks the reactor — not shippable) | 106.7 µs | 0.107 | 9.5 µs |
| 2 **`rusqlite` via `spawn_blocking`** | **120.9 µs** | **0.121** | **26.8 µs** |
| 3 `rusqlite` on a dedicated thread + channel | 133.3 µs | 0.133 | 24.5 µs |
| 4 `sqlx-sqlite` native (incumbent) | 1,629.2 µs | 1.629 | 40.4 µs |

**Yes.** The thread hop costs ~15 µs per *query* — which is why point queries
only improve 1.5× while 1,000-row queries improve 13.5×. It is a fixed cost,
not a per-row cost, and it amortises to nothing on exactly the workloads where
ruprizzle currently looks worst.

### 3.2b The advantage widens under concurrency (task C-0)

A 13× measured one-caller-at-a-time is worthless if both designs converge under
load — both end up bounded by SQLite itself. `concurrency.rs`, 8 connections
per arm, read-only, best of 3, **total queries/sec** across all tasks:

| concurrent tasks | `rusqlite` q/s | `sqlx-sqlite` q/s | speedup |
|---:|---:|---:|---:|
| 1 | 9,846 / 9,619 | 680 / 703 | 13.7–14.5× |
| 2 | 11,938 / 10,888 | 486 / 500 | 21.8–24.6× |
| 4 | 8,695 / 3,625 | 340 / 326 | 11.1–25.6× |
| 8 | 4,180 / 3,081 | 205 / 191 | 16.2–20.4× |
| 16 | 4,334 / 4,139 | 205 / 191 | 21.1–21.6× |

Two independent runs, both shown, because the `rusqlite` arm is unstable at
4 tasks (8,695 vs 3,625 q/s) on this machine. **Do not cite a point estimate
from this table.** The robust claim is the floor: across every cell of both
runs the speedup is **never below 11×**.

The advantage does not merely survive concurrency, it widens.
`sqlx-sqlite`'s throughput *falls* as concurrency rises (≈690 → ≈195 q/s, a
3.5× loss) and is essentially flat from 8 tasks on, while `rusqlite` stays far
above its own single-task rate. Both degrade past 4 tasks — expected, with 8
connections on a shared file and a finite core count — but `sqlx-sqlite`
degrades harder, because every additional concurrent reader adds another
per-row channel to service.

C-0 was written into this plan as a go/no-go gate on Phase C. **It passes**, on
the ≥11× floor rather than on any single number.

### 3.2c Why

Layer 3 is the load-bearing measurement. It reproduces `sqlx-sqlite`'s
architecture — a long-lived OS thread owning the connection, results crossing a
channel — and still lands at 0.133 µs/row, **12× better than `sqlx-sqlite`**.
So the cost is not "async", not "worker thread", not "channel". It is
specifically the **per-row** send: `sqlx-sqlite` allocates a `SqliteRow` per row
and pushes each one through `flume::bounded(row_buffer_size)`, default **50**
(`sqlx-sqlite-0.8.6/src/options/mod.rs:211`). Ten thousand rows means ten
thousand sends and 200 producer/consumer stalls.

That also explains G4 below: raising `row_buffer_size` recovers ~10% by
reducing the stalls, but cannot touch the per-row allocation, so it plateaus
far short of the floor. Round 1 correctly measured that knob and correctly
declined to over-invest in it.

### 3.3 `PoolConfig::default()` charges a ping per query

`pool_config.rs`, 7 interleaved repeats:

| | min | median | max |
|---|---:|---:|---:|
| `select_by_pk`, `test_before_acquire = true` | 39.68 | **40.37** | 52.15 |
| `select_by_pk`, `test_before_acquire = false` | 29.76 | **30.15** | 36.73 |

**+10.22 µs/query (+33.9%), and the ranges do not overlap.** On
`find_many_1000` the same ping is −2.0% with overlapping ranges — i.e. not
measurable, exactly as a fixed per-checkout cost should behave.

`crates/runtime/src/pool.rs:36` sets this default. `sqlx` then runs, in
`sqlx-core-0.8.6/src/pool/inner.rs:469` (`check_idle_conn`):

```rust
if options.test_before_acquire {
    if let Err(error) = conn.ping().await { ... }
}
```

And `ping` for Postgres (`sqlx-postgres-0.8.6/src/connection/mod.rs:176`) is:

```rust
fn ping(&mut self) -> BoxFuture<'_, Result<(), Error>> {
    Box::pin(async move {
        self.write_sync();
        self.wait_until_ready().await
    })
}
```

`write_sync` queues a `Sync` message; `wait_until_ready` flushes and blocks
until the server answers `ReadyForQuery`. **That is a full network round-trip
to the database before every query.** On a local socket it is tens of
microseconds. On a hosted Postgres — the project's stated production target —
it is one to tens of milliseconds, and it roughly **doubles the latency of
every simple query**.

This is source-verified, not live-reproduced: the local `postgresql-x64-17`
service is Stopped and starting it needs elevation. Task **P0-1** closes that.

### 3.4 The `sqlx::Any` tax is larger than round 1 reported

Round 1 measured +0.1% on `find_many_1000` and retracted the `Any` attribution
in `BenchmarkResults.md`. That retraction went too far.

From §3.1, native vs `Any`, same query, same decode target:

- posts (10k): 1.292 → 1.504 µs/row, **+16.4%**
- users (1k): 1.404 → 1.859 µs/row, +32%

And on the raw fetch path with no decoding at all (`bottlenecks.rs` §3):

| | native `SqliteRow` | `AnyRow` | tax |
|---|---:|---:|---:|
| 2 integer columns | 663 µs | 1,025 µs | **+54.5%** |
| 3 columns incl. text | 846 µs | 1,620 µs | **+91.4%** |

The +54.5% on *pure integers* is the important one — there is no string
allocation to blame. It is `AnyValueKind` boxing, the per-row `Vec`, and
per-column type-info conversion. Part of the raw-fetch tax is work merely moved
earlier (the `Any` driver allocates each `String` during row conversion, then
`try_get::<String>` clones it *again* into the model — two allocations per text
cell where native does one), which is why the end-to-end figure is smaller than
the raw-fetch figure. But end-to-end it is still **~16%**, not ~0%.

Round 1's +0.1% came from an unlucky pairing on a noisy machine. `docs/
BenchmarkResults.md` currently says "the `sqlx::Any` wrapper adds ~61 µs (4%)",
which is closer but still low. Correction listed in §7.

### 3.5 `include_posts` is entirely per-row cost

`bottlenecks.rs` §5, 1,000 users + 10,000 posts:

| step | µs |
|---|---:|
| a. fetch 1,000 users | 1,659 |
| b. fetch 10,000 posts (no filter) | 14,783 |
| c. fetch 10,000 posts `WHERE author_id IN (1,000 binds)` | 13,823 |
| **a + b** | **16,442** |
| measured `include_posts` end-to-end | ~16,700 |

The two fetches account for **98.5%** of the operation. The 1,000-element `IN`
list costs *nothing* (c is 6.5% *faster* than b — index path, and within noise).
`dedup`, the `HashMap<Key, Vec<C>>` grouping, and `Related::Loaded` wrapping
are all below the noise floor combined.

**There is nothing to optimise in `include.rs`.** It inherits whatever the
driver does, and it will inherit the 13× too. Projected under Phase C:
16.7 ms → **~1.6 ms**, which is 20× faster than Prisma and 110× faster than
Drizzle's correlated-subquery SQLite path.

### 3.6 Small, real, and worth one commit

| item | cost | where |
|---|---:|---|
| ruprizzle `Executor` wrapper over raw `sqlx` | **+2.2%** (36 µs / 1,650) | `executor.rs:58-92` |
| `Value::Str(Arc<str>)` → `String` on every bind | 1 alloc per bound string | `value.rs` (fixed in P1-4) |

The `Executor` wrapper cost was the `sql: String` clone and the
`Instant::now()` / `tracing::debug!` call site, paid on every query whether or
not a subscriber was installed. Phase A-2 removes the clone (SQL is now
`Cow<'static, str>` and borrowed for `sqlx`) and skips `Instant::now()` unless
the `ruprizzle::query` target is enabled. The bind re-allocation is round 1's deferred P1-4; it
is forced by `Any`'s `Encode<'q, Any>` lifetime and dissolves on its own under
a native backend.

---

## 4. What *not* to do

Each of these was measured and rejected. Recording them so they are not
re-proposed.

| candidate | measurement | verdict |
|---|---|---|
| Stream decode instead of `Vec<AnyRow>` + second pass | −0.9% one run, +5.4% the next; straddles zero | **No.** The `Executor` returning `Vec<AnyRow>` is not a bottleneck. Revisit only for peak memory, never for speed. |
| Optimise the include grouping / `dedup` | below noise inside a 16.4 ms op (§3.5) | **No.** |
| Chunk the `IN` list differently | −6.5%, wrong sign, within noise | **No.** |
| Chase `row_buffer_size` as the main fix | ~10%, plateaus at 12× off the floor (§3.2) | **Not the fix.** Take it for free under Phase B; do not build a project around it. |
| Reduce dialect boxing / SQL string building | 0.018 µs and 0.42 µs (round 1) | **No.** Unchanged. |
| A true streaming cursor | 64% *slower* (round 1) | **No.** Unchanged. |

Everything in round 1's §7 still holds. Nothing there has been reopened.

---

## 5. Programme

### Phase A — the default that costs a round-trip (hours)

| # | task | evidence | effect | status |
|---|---|---|---|---|
| **A-1** | `PoolConfig::default().test_before_acquire = false` | §3.3 | **−10.2 µs/query** on SQLite; **−1 network RTT/query** on Postgres | implemented |
| **A-2** | Trim `Executor::fetch_all_raw`: take `sql` as `Cow<'_, str>`, drop `Instant::now()` unless the `ruprizzle::query` target is enabled | §3.6 | ~−2% | implemented |

**A-1 is the highest value-per-line change in the project** and it is a
one-word diff. It is not free, and the docs must say so:

- With `test_before_acquire = false`, a connection killed server-side between
  checkouts surfaces as a **query error** instead of being silently replaced.
- `max_lifetime` (1800 s) and `idle_timeout` (600 s) still recycle connections,
  so this only exposes *unexpected* death: failover, an idle-connection
  reaper, a restart.
- `sqlx`'s own default is also `true`, so this is a deliberate divergence, not
  a bug fix. Ship it as a documented, reversible default with a `PoolConfig`
  field already in place — the knob exists, only the default changes.
- Recommended framing: correctness-critical deployments behind an aggressive
  proxy set it back to `true` and pay the RTT knowingly.

### Phase B — native backends (round 1's Phase 2, unchanged and still right)

```rust
enum Backend { Postgres(PgPool), Sqlite(SqlitePool) }
```

| # | task | effect | status |
|---|---|---|---|
| **B-1** | `Backend` enum behind `Pool`; `Executor` dispatches per variant | enables the rest | implemented |
| **B-2** | Drop `sqlx::Any` from the read path | **−16%/row** (§3.4) | implemented |
| **B-3** | Expose `row_buffer_size`, journal mode, cache size on the SQLite variant | **−10%/row** (§3.1 row 3) | implemented |
| **B-4** | Native `Encode` per backend; `Value::Str` binds by reference | removes the per-bind allocation (§3.6), unblocks round 1's P1-4 | implemented |
| **B-5** | Native decode per backend | **fixes round 1's F2** — `UUID`, `TIMESTAMPTZ`, `NUMERIC`, `JSONB`, `DATE`, `TIME` are unreadable through `Any` on Postgres today | implemented |

Combined: ~25% on row-heavy reads, plus the correctness hole closed.

**B-5 remains the real reason to do Phase B.** F2 is a correctness ceiling, not
a speed one: `AnyTypeInfo::try_from(&PgTypeInfo)` rejects six of the twelve
types ruprizzle's own Postgres DDL emits, and the row fails inside the driver
before `decode::*` ever runs. Still unverified live (P0-1).

### Phase C — `rusqlite` backend (the 13×)

The only phase that changes the headline number.

| # | task |
|---|---|
| ~~**C-0**~~ | ~~Gate: measure both drivers under concurrency~~ — **done, §3.2b, passes at ≥11×** |
| **C-1** | Feature `sqlite-rusqlite`, off by default; `Backend::SqliteNative(RusqlitePool)` |
| **C-2** | Connection pool of `rusqlite::Connection`, each pinned to a blocking task; `prepare_cached` for statement reuse |
| **C-3** | `Value` → `rusqlite::ToSql` and `rusqlite::Row` → model decode, replacing the `Any` path |
| **C-4** | Transactions: `Tx` must pin one connection for its lifetime — `spawn_blocking` per statement is not enough, the connection cannot migrate between blocking tasks mid-transaction |
| **C-5** | Route migrations and the CLI through the same backend |
| **C-6** | Run the full existing suite (`tests/integration`, `local/deep-tests`, testkit) against both SQLite backends; they must agree row-for-row |
| **C-7** | Re-run the cross-ORM benchmark; update `docs/BenchmarkResults.md` |

Projected from §3.2: `find_many_1000` ~150 µs (from 1,648), `include_posts`
~1.6 ms (from 16.7), `select_by_pk` ~27 µs (from 41.2).

**Scope honestly.** This is a second driver implementation, not a
configuration change. Costs and risks:

- Two SQLite code paths to keep behaviourally identical — C-6 is not optional,
  it is the deliverable that makes C shippable.
- Blocking-pool sizing interacts with the Tokio runtime's `max_blocking_threads`
  (512 by default). A busy server doing many small queries burns a blocking
  thread per query for ~15 µs; measure under concurrency before shipping.
- ~~The 13× is a single-threaded result and may collapse under load.~~
  **Measured (§3.2b): it widens, never below 11×.** C-0 is closed.
- `rusqlite 0.32` is pinned to share `libsqlite3-sys 0.30.1` with
  `sqlx-sqlite`. If Phase C ships, that pin becomes a real compatibility
  constraint on both crates' upgrade cadence — or `sqlx-sqlite` is dropped
  entirely and the constraint disappears.

**Recommendation:** A and B are done; C is next. C-0 was the gate and it has been measured (§3.2b) — the advantage
holds at ≥11× under concurrent load, so Phase C is justified. The remaining
open question is not *whether* it is worth it but blocking-pool sizing under a
real server's mixed read/write load.

### Phase D — verification debt

| # | task |
|---|---|
| **P0-1** | Start local Postgres; run `pg_any_types` (round 1's F2) **and** measure `test_before_acquire` RTT against a real server (§3.3) |
| **P0-2** | Rich-type round-trip test per dialect, as a regression guard for B-5 |

Both were carried over unresolved from round 1. §3.3 has now added a second,
larger reason to care.

---

## 6. Risks

| risk | mitigation |
|---|---|
| A-1 makes a dead connection surface as a query error | Document it; keep the `PoolConfig` field; recommend `true` behind aggressive proxies |
| ~~Phase C's 13× does not survive concurrency~~ | **Closed.** Measured at ≥11× in every cell across 1–16 tasks (§3.2b) |
| Blocking-pool sizing under mixed read/write load | Still open. §3.2b is read-only; measure with writers before C-1 ships |
| Two SQLite backends drift | C-6: the whole suite runs against both, and they must agree |
| `libsqlite3-sys` version pin between `rusqlite` and `sqlx-sqlite` | Either accept the pin or drop `sqlx-sqlite` for SQLite entirely |
| All numbers are one Windows box, SQLite, single-threaded | Stated on every table; §7 corrections; C-0 adds the concurrency axis |
| Postgres remains unmeasured | P0-1 |

---

## 7. Corrections owed to `docs/BenchmarkResults.md`

Round 1 added an attribution section that is now partly wrong. When Phase A
lands, that document needs:

1. **The `Any` tax is ~16%, not ~4%** (§3.4). The current text says "the
   `sqlx::Any` wrapper adds ~61 µs (4%)".
2. **The floor is `rusqlite` at ~0.10 µs/row, not better-sqlite3 at ~0.32.**
   The current text frames `sqlx-sqlite`'s 0.9 µs/row against better-sqlite3's
   0.3 and calls the difference a language/driver-model gap. It is not — it is
   13× available to a Rust ORM.
3. **Retract "the remaining ~1,300 µs of the gap to Drizzle is `sqlx` itself"**
   as an explanation-that-excuses. It is true and it is *fixable*. The current
   phrasing reads as a closed matter.
4. **Add a `rusqlite` control column.** Without it the table compares an async
   Rust stack against a synchronous Node binding and silently attributes the
   difference to the ORM.
5. Per-row figures should be stated per-row. `find_many_1000` and
   `include_posts` differ 10× in row count and their totals are not comparable.

---

## 8. Raw data

<details>
<summary><code>perrow.rs</code> — five layers, median of 5</summary>

```
=== users: 1000 rows x (i64, String, i64) ===
layer                            min us     median        max      us/row  vs floor
----------------------------------------------------------------------------------
1 rusqlite (sync)                  91.6       97.3      136.1       0.097    1.00x
2 sqlx native                    1366.4     1403.8     1414.5       1.404   14.43x
3 sqlx native +row_buffer        1317.7     1328.2     1331.2       1.328   13.65x
4 sqlx Any                       1846.7     1858.6     1880.6       1.859   19.10x
5 ruprizzle SelectQuery          1546.6     1561.1     1600.9       1.561   16.04x

=== posts: 10 000 rows x (i64, i64, String) ===
1 rusqlite (sync)                1002.0     1053.5     1086.8       0.105    1.00x
2 sqlx native                   12305.8    12921.4    13229.7       1.292   12.27x
3 sqlx native +row_buffer       11091.2    11555.3    12063.6       1.156   10.97x
4 sqlx Any                      14837.3    15037.7    15215.7       1.504   14.27x
5 ruprizzle SelectQuery         13814.2    14094.7    14588.4       1.409   13.38x

=== select_by_pk: 1 row ===
1 rusqlite (sync)                   9.5        9.8       10.1       9.842    1.00x
2 sqlx native                      38.7       39.9       40.4      39.943    4.06x
4 sqlx Any                         40.5       40.6       42.2      40.588    4.12x
5 ruprizzle SelectQuery            43.0       44.3       47.0      44.286    4.50x
```
</details>

<details>
<summary><code>blocking_floor.rs</code> — is the floor reachable from async?</summary>

```
=== find_many_1000 (1000 rows) ===
layer                                           min us    median            per row  vs floor
1 rusqlite inline (blocks reactor)                97.1     106.7      0.107 us/row    1.00x
2 rusqlite via spawn_blocking                    107.3     120.9      0.121 us/row    1.13x
3 rusqlite on dedicated thread + channel         123.4     133.3      0.133 us/row    1.25x
4 sqlx-sqlite native (incumbent)                1613.4    1629.2      1.629 us/row   15.27x

=== select_by_pk (1 row) ===
1 rusqlite inline (blocks reactor)                 9.3       9.5      9.525 us/row    1.00x
2 rusqlite via spawn_blocking                     24.9      26.8     26.850 us/row    2.82x
3 rusqlite on dedicated thread + channel          23.2      24.5     24.471 us/row    2.57x
4 sqlx-sqlite native (incumbent)                  40.0      40.4     40.396 us/row    4.24x
```
</details>

<details>
<summary><code>concurrency.rs</code> — task C-0, best of 3, total q/s, two runs</summary>

```
run 1                                          run 2
 tasks   rusqlite  sqlx-sqlite  speedup         tasks   rusqlite  sqlx-sqlite  speedup
------------------------------------------     ------------------------------------------
     1       9846          680   14.48x              1       9619          703   13.68x
     2      11938          486   24.55x              2      10888          500   21.77x
     4       8695          340   25.56x              4       3625          326   11.12x
     8       4180          205   20.39x              8       3081          191   16.15x
    16       4334          205   21.13x             16       4139          191   21.63x

find_many_1000, read-only, 8 connections per arm, test_before_acquire=false
on both. The rusqlite arm is unstable at 4 tasks; the sqlx arm is stable
throughout. Minimum speedup across all ten cells: 11.12x.
```
</details>

<details>
<summary><code>pool_config.rs</code> — 7 interleaved repeats</summary>

```
select_by_pk (1 row)
  test_before_acquire=true   min    39.68  median    40.37  max    52.15 us
  test_before_acquire=false  min    29.76  median    30.15  max    36.73 us
  -> ping cost: +10.22 us/query (+33.9%)   [ranges do not overlap]

find_many_1000 (1000 rows)
  test_before_acquire=true   min  1546.96  median  1552.15  max  1578.88 us
  test_before_acquire=false  min  1512.41  median  1583.49  max  1682.40 us
  -> ping cost: -31.34 us/query (-2.0%)   [ranges overlap]
```
</details>

<details>
<summary><code>bottlenecks.rs</code> — Any conversion, two-pass decode, include</summary>

```
=== 3. AnyRow conversion cost (fetch only, no decode) ===
native SqliteRow  2 int cols   (id, age)           663.07 us/op
AnyRow            2 int cols   (id, age)          1024.71 us/op   (+54.5%)
native SqliteRow  3 cols w/text(id, email, age)     846.25 us/op
AnyRow            3 cols w/text(id, email, age)    1619.79 us/op   (+91.4%)

=== 4. Vec<AnyRow> then decode, vs decode as rows arrive ===
fetch_all -> Vec<AnyRow> -> decode (ruprizzle)    1723.30 us/op
fetch (stream) -> decode each row -> drop         1635.66 us/op   (+5.4%)
query_as::<Any, User> (sqlx's own map)            1672.21 us/op   (+2.2%)
   (run 1 of the same pair gave -0.9%; straddles zero)

=== 5. include_posts breakdown (1000 users, 10000 posts) ===
a. fetch 1000 users only                          1658.70 us/op
b. fetch 10000 posts, no IN list                 14783.36 us/op
c. fetch 10000 posts WHERE id IN (1000 binds)    13823.04 us/op   (-6.5%)
  parents + children (b) = 16442.1 us; measured include_posts is ~16700 us

=== 6. isolated round-trip costs ===
Connection::ping (sqlite worker round-trip)          0.47 us/op
SELECT 1  (Any, pooled)                             13.42 us/op

=== 7. ruprizzle Executor overhead over raw sqlx (same pool) ===
raw sqlx::query fetch_all + decode                1687.59 us/op
via ruprizzle Executor::fetch_all_raw             1723.99 us/op   (+2.2%)
```
</details>

---

## 9. See also

- [`Enhancements1.md`](Enhancements1.md) — round 1. F1/F3/F9 fixed in
  `83a2a9e`, F8 in `5f21c1c`. F2 still open (see P0-1). §7 "explicitly not
  doing" still holds in full.
- `docs/BenchmarkResults.md` — needs the §7 corrections.
- `docs/KnownLimitations.md` — the rich-type note is still SQLite-accurate and
  Postgres-inaccurate until B-5.
- `crates/runtime/src/pool.rs:36` — the A-1 one-word diff.
