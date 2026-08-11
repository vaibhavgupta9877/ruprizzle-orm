# Performance Enhancements 1 — where the time actually goes

**Status:** partially implemented
**Date:** 2026-08-11
**Baseline:** `0.1.0-alpha.3`
**Branch:** `perf/research-harnesses`
**Trigger:** [`docs/BenchmarkResults.md`](../../../docs/BenchmarkResults.md) shows ruprizzle 1.6–5.8× behind Drizzle on simple SQLite reads.

---

## 1. Executive summary

The premise of this work — "our package is falling behind" — is correct about the
numbers and wrong about the cause. `BenchmarkResults.md` attributes the gap to
ruprizzle's use of `sqlx::Any`:

> ruprizzle is 1.6–5.8× slower on these micro-benchmarks, mostly because
> `sqlx::Any` is async and text-encodes/decodes every bind and row.

Measured layer-by-layer against the same database file, that attribution does not
hold. On `find_many_1000`:

| Layer | µs/op | vs. layer below |
|---|---:|---|
| hand-written `sqlx`, **native** `SqlitePool`, `query_as` | 1 628.3 | — |
| hand-written `sqlx`, **`AnyPool`**, `query_as` | 1 630.7 | **+0.1%** ← the `Any` tax |
| `AnyPool` + ruprizzle's `decode::*` helpers | 1 561.1 | −4% |
| **ruprizzle `SelectQuery::fetch_all`** | **1 679.4** | +3% |
| *Drizzle + better-sqlite3 (recorded)* | *289.7* | — |

**ruprizzle costs ~3% more than hand-written `sqlx` doing the identical work**,
and the four layers are within a few percent of one another — close enough that
the ordering between them changes between runs. The other ~1 340 µs of the gap
to Drizzle is `sqlx` itself. Deleting the entire ORM layer would move
`find_many_1000` from 1 679 µs to 1 628 µs — still 5.6× behind Drizzle.

So the honest framing is:

- **There is no meaningful fat in the query builder, the compiler, or the
  relation loader.** They sit at ~97% of the achievable ceiling, and on the
  relation path the ORM's overhead is not distinguishable from noise at all
  (§3.1). Three of the optimisations one would reflexively reach for (dialect
  boxing, SQL string building, include grouping) measure at 0.018 µs, 0.42 µs
  and "sign not stable across runs" respectively.
- **The ceiling is `sqlx`'s per-row cost**, ~0.9 µs/row on SQLite versus
  better-sqlite3's ~0.29 µs/row. It is architectural (§3.2), not a tuning
  problem.
- **The genuinely valuable work is elsewhere**, and it is mostly not
  micro-optimisation:
  1. A one-line missing `LIMIT 1` that makes `fetch_one`/`fetch_optional`
     **24–39× slower** than it needs to be on any non-unique filter (§4, F1).
  2. `sqlx::Any` **cannot read six of the twelve column types ruprizzle's own
     Postgres DDL emits** — a correctness ceiling, not a speed one (§4, F2).
  3. `.include(...)` is **silently dropped** by `.fetch_all()` (§4, F3).
  4. A driver-tuning knob worth **14–23%** that `Any` makes unreachable (§4, F5).

This document proposes a programme built around those, and explicitly names the
work *not* worth doing (§8).

> **One caveat up front, stated plainly:** end-to-end benchmark numbers are
> SQLite on one Windows workstation. SQLite is not the deployment target. The
> `postgresql-x64-17` Windows service still cannot be started without elevation,
> but the server can be started as a user process with `pg_ctl` once a data
> directory exists. Task **P0-0** was run live on 2026-08-11; its results are in
> §4.2 and the status table. Task **P0-1** (a round-trip integration test with
> rich types) was implemented in `crates/runtime/tests/rich_types.rs` on
2026-08-11.

---

## 2. Method

### Harnesses

Four reproducible harnesses were added under `crates/runtime/examples/`, all
running against the same `local/cross-orm-bench/node/bench.sqlite3`
(1 000 users, 10 000 posts) used by the recorded cross-ORM benchmark:

| Harness | Answers |
|---|---|
| `layer_attribution.rs` | How much of ruprizzle's time is the driver, the decode helpers, and the builder? |
| `sqlx_floor.rs` | What does `sqlx` charge per query, per row, and per column, independent of ruprizzle? |
| `include_breakdown.rs` | Where do the 16–17 ms of `include_posts` go? |
| `hotspots.rs` | What is each individually-suspected ORM inefficiency actually worth? |
| `row_buffer.rs` | What is the sqlx-sqlite row-channel depth worth, and can we reach it? |
| `pg_any_types.rs` | Which Postgres types survive `sqlx::Any`? *(written, not yet run — see P0-0)* |

Run them with `cargo run --release --example <name> -p ruprizzle`.

### Rules used

- Release profile, 3 warm-up iterations, 30–100 000 iterations depending on cost.
- **Deltas smaller than ~1 ms on the 11 000-row shapes are inside run-to-run
  noise on this machine.** Where that matters, results are reported as ranges
  over five runs rather than as a single number, and the conclusion is stated as
  "not measurable" rather than given a spurious percentage.
- The **control is hand-written `sqlx` doing the same work**, not another ORM on
  another runtime. A cross-language comparison tells you which stack is faster;
  it does not tell you what to change in yours.
- Every claimed inefficiency gets a measured delta before it enters the plan.
  Three candidates were rejected this way (§8).

---

## 3. Measured attribution

### 3.1 `include_posts` — 1 000 parents + 10 000 children

The one shape where ruprizzle already leads the field (16.2 ms vs Prisma 32.6 ms
vs Drizzle 181.5 ms). Broken down:

| Layer | ms/op | delta |
|---|---:|---|
Five runs, reported as ranges — the spread matters for reading the deltas:

| Layer | ms/op (5 runs) | delta |
|---|---:|---|
| A — native `sqlx`, 2 queries, tuple decode, no grouping | 13.8 – 14.9 | transport floor |
| B — `AnyPool`, 2 queries, struct decode by name | 15.5 – 17.3 | **+1.4 to +3.4 (+10–25%)** |
| C — B + hand-rolled `HashMap` grouping and attach | 16.0 – 17.5 | +0.3 to +0.5 |
| D — **ruprizzle `.include(posts()).exec()`** | 15.2 – 17.5 | **−1.5 to +0.6** |

- **~80%** of the time is raw `sqlx` transport.
- **10–25%** is the `Any` wrapper plus name-based column lookup.
- **D − C straddles zero.** Across five runs ruprizzle's full include path is
  sometimes faster and sometimes slower than the hand-rolled equivalent. **The
  ORM's own overhead on this path is not measurable above run-to-run noise** —
  which is a stronger statement than the "+3%" a single run suggested, and the
  reason §7 rejects every grouping-related optimisation.

The relation loader — the part of the codebase that looks most like it should be
optimised — is essentially free. Compiling a 1 000-key `IN` list to a 3 053-byte
statement takes 29 µs; deduplicating 1 000 keys takes 8 µs. Together that is
0.2% of the operation.

### 3.2 The `sqlx` floor

| Operation | µs/op | per row |
|---|---:|---:|
| `SELECT 1` (round-trip floor) | 21.6 | — |
| 1 000 rows × 3 cols | 906.6 | 0.91 |
| 10 000 rows × 3 cols | 8 939.9 | 0.89 |
| 1 000 rows × 1 int col | 442.3 | 0.44 |
| 1 000 rows × 1 text col | 563.3 | 0.56 |
| 10 000 rows via `.fetch()` **stream** | 14 619.7 | **1.46** |

Two structural facts, confirmed from the `sqlx-sqlite` 0.8.6 source:

1. **Every connection owns a dedicated OS thread.** `ConnectionWorker::establish`
   does `thread::Builder::new().spawn(...)`; the async side communicates over
   `flume` channels (`connection/worker.rs`).
2. **Every row crosses that thread boundary individually**, through a channel
   bounded at `row_channel_size`, which **defaults to 50**
   (`options/mod.rs:211`). A 10 000-row fetch blocks on backpressure 200 times.

That is the ~0.9 µs/row, and it is why per-row cost is flat in result-set size
and scales with column count. It also explains the counter-intuitive stream
result: `.fetch()` is **64% slower per row** than `fetch_all`, so making
`Executor::stream_raw` a "true cursor" would make it *slower*, not faster (§8).

Drizzle's recorded 289.7 µs for the same 1 000 rows is 0.29 µs/row —
better-sqlite3 is synchronous, in-process, with no thread hop at all. **No
amount of ORM-layer work closes a 3.1× per-row architectural gap.**

---

## 4. Findings

Ordered by value, not by category. Severity is `perf` / `correctness` / `DX`.

---

### F1 — `fetch_one` / `fetch_optional` emit no `LIMIT 1` · **perf, 24–39×**

`SelectQuery::fetch_optional` compiles the query unchanged, fetches **every**
matching row, decodes **all** of them, then calls `v.remove(0)`
(`crates/runtime/src/query.rs:186-204`). `fetch_one` delegates to it.

Measured on a filter matching 1 000 rows, 5 runs:

| | µs/op |
|---|---:|
| `.filter(age > 0).fetch_optional()` | **1 561 – 1 705** |
| `.filter(age > 0).limit(1).fetch_optional()` | **44 – 64** |
| **saving** | **~1 600 µs, i.e. 24–39×** |

Emitted SQL: `SELECT * FROM \`users\` WHERE (\`users\`.\`age\` > ?)` — no `LIMIT`.

This is the largest single ORM-layer win in the codebase and it is a two-line
change. It is invisible in the current benchmark suite because every
`fetch_one` there is filtered by primary key, where the database returns one row
anyway. Real applications call `fetch_optional` on non-unique filters constantly.

**Fix:** override `limit` to `Some(1)` inside `fetch_optional` when no explicit
limit is set. Keep `fetch_one`'s "no row found" error semantics.

**Risk:** none. A caller who set `.limit(n)` explicitly keeps it; a caller who
did not cannot observe the difference except in speed.

---

### F2 — `sqlx::Any` cannot read half the column types ruprizzle's own Postgres DDL emits · **correctness**

`crates/dialect/src/postgres.rs` maps ruprizzle scalars to Postgres DDL types:

| ruprizzle type | emitted DDL | readable through `sqlx::Any`? |
|---|---|---|
| `String` | `TEXT` | yes |
| `Int` / `BigInt` | `INTEGER` / `BIGINT` | yes |
| `Float` | `DOUBLE PRECISION` | yes |
| `Boolean` | `BOOLEAN` | yes |
| `Bytes` | `BYTEA` | yes |
| `Decimal` | `NUMERIC` | **no** |
| `DateTime` | `TIMESTAMPTZ` | **no** |
| `Date` | `DATE` | **no** |
| `Time` | `TIME` | **no** |
| `Uuid` | `UUID` | **no** |
| `Json` | `JSONB` | **no** |

`sqlx-postgres-0.8.6/src/any.rs` implements `TryFrom<&PgTypeInfo> for
AnyTypeInfo` over exactly `Bool`, `Void`, `Int2`, `Int4`, `Int8`, `Float4`,
`Float8`, `Bytea`, `Text`, `Varchar`, `citext`. Everything else returns
`Err(AnyDriverError("Any driver does not support the Postgres type ..."))`.

That conversion is not optional and not deferred. `AnyRow::map_from`
(`sqlx-core/src/any/row.rs`) calls it **per column, per row**, with `?`. So the
failure happens during row conversion inside the driver — before any of
ruprizzle's `decode::text` / `decode::json` helpers ever run. Those helpers
cannot rescue a row that the driver refused to construct.

The docs currently describe this as a performance characteristic:

> **Rich types round-trip as text.** `Uuid`, `Decimal`, `DateTime`, `Date`,
> `Time`, and `Json` are sent and received as text because the underlying
> `sqlx::Any` driver does not encode them natively. — `docs/KnownLimitations.md`

That is accurate for **SQLite**, where `Any` maps by value affinity and text
genuinely round-trips. For **Postgres** the reading half does not work at all.
The bind half is likely broken too: `AnyArguments::convert_to` re-encodes a
text `Value` as `&str`, so `WHERE id = $1` against a `uuid` column sends an
OID-25 parameter and Postgres rejects `uuid = text`.

**Why no test caught it:** the end-to-end bench schema
(`crates/runtime/benches/end_to_end/schema.ruprizzle`) uses only `BigInt` and
`String`. `Performance.md` even records the reason — "the bench schema uses
`BIGINT` and `TEXT` columns" — and treats it as a measurement caveat rather than
a coverage gap.

**Fix:** F4 (native backends). There is no fix that keeps `Any`.

**Verification:** P0-0 ran `cargo run --example pg_any_types` against a live
PostgreSQL 17.10 instance on 2026-08-11. The pass/fail table above matches the
live run exactly. The `uuid = text` bind probe also failed as predicted.

---

### F3 — `.include(...)` is silently dropped by `.fetch_all()` · **correctness / DX**

Includes are loaded only in `SelectQuery::exec`, which lives in a separate impl
block gated on `I: IncludeSet<M>` (`query.rs:349-368`). `fetch_all` is defined
on the general `SelectQuery<'db, M, Out, I>` and never touches `self.includes`
(`query.rs:172-183`).

So this compiles, runs, succeeds, and returns users whose `posts` are `Absent`:

```rust
let users = SelectQuery::<User>::new(&db)
    .include(posts())
    .fetch_all()      // include silently ignored
    .await?;
```

Encountered directly while building `include_breakdown.rs`: the harness reported
`users=1000 loaded=0 attached_posts=0` and a suspiciously fast 1.61 ms before the
call was changed to `.exec()`. The failure surfaces later as a panic from
`Related::get` — *"relation was not loaded — add an `.include()` to the query"* —
which names the exact remedy the caller already applied.

**Fix options**, cheapest first:

1. Make `fetch_all` on a query carrying a non-`()` include set a **compile
   error**, by moving `fetch_all` into an impl block constrained to `I = ()`.
   Callers get a type error pointing at `.exec()`. No runtime cost.
2. Or make `fetch_all` load includes and delete `exec`'s special case, so the
   two names behave identically.

Option 1 is preferable: `exec` and `fetch_all` returning different data for the
same builder is the actual defect, and the type system can state that.

---

### F4 — Native driver backends instead of `sqlx::Any` · **perf ~20% + unblocks F2, F5**

`Any` costs, measured and observed:

- **+10–25% on the `include_posts` path** (§3.1, B−A) — and that delta bundles
  the `AnyRow` re-materialisation with name-based struct decode, so the wrapper
  alone is at the lower end. Call it **~15%**, and re-measure on Postgres before
  quoting it anywhere.
- **Every value is converted twice in each direction.** Outbound:
  ruprizzle `Value` → `AnyValueKind` (`AnyArguments`) → `PgArguments`
  (`convert_to`). Inbound: `PgRow` → owned `AnyRow` (`map_from`) → target type.
  ruprizzle adds a third copy on strings: `Value::Str(Arc<str>)` is turned back
  into an owned `String` on every bind (`value.rs:200-203`), and `Value::Bytes`
  is re-collected element-by-element into a fresh `Vec<u8>` (`value.rs:224-227`),
  despite both already being cheap-to-clone `Arc`s.
- **It hides driver-specific tuning** (F5) and **the whole native type system**
  (F2).
- **It blocks `COPY`** (F6).

**Design.** Keep `Executor` object-safe — it is what lets one query run against a
pool or a transaction — and dispatch on an enum rather than a generic:

```rust
pub enum Backend {
    Postgres(sqlx::PgPool),
    Sqlite(sqlx::SqlitePool),
}
```

Generated `FromRow` impls become per-backend, which codegen already has the
schema information to emit. `Any` stays available behind a default feature so
the alpha's public API does not break in a patch release.

This is the largest item in the programme and the only one that needs real
design work. Sequenced accordingly (§5, Phase 2).

---

### F5 — `row_buffer_size` is worth 14–23% and is unreachable · **perf**

`SqliteConnectOptions::row_buffer_size` sets the depth of the worker→async row
channel. Default 50. `ruprizzle::connect` builds an `AnyPool`, and the `Any`
driver constructs `SqliteConnectOptions` from the URL alone, so no caller can
set it:

| `row_buffer_size` | users (1 k rows) | posts (10 k rows) |
|---:|---:|---:|
| 50 *(default)* | 920.2 µs | 9 313.3 µs |
| 200 | 856.9 µs (−7%) | 8 538.2 µs (−8%) |
| 1 000 | 814.8 µs (−11%) | 7 394.6 µs (−21%) |
| 4 096 | 798.8 µs (−13%) | 7 198.9 µs (−23%) |
| 16 384 | 795.5 µs (−14%) | 7 192.7 µs (−23%) |

Returns flatten past ~1 000. This is free performance that the abstraction is
currently spending. It also generalises: `PoolConfig` exposes six pool knobs and
zero driver knobs, so the same blindness applies to Postgres options such as
`statement_cache_capacity`.

**Fix:** rides on F4. Extend `PoolConfig` with an optional per-backend section;
default SQLite `row_buffer_size` to 1 024.

---

### F6 — Bulk insert: one giant `INSERT`, no `COPY` · **perf**

`InsertManyQuery` builds a single multi-row `INSERT ... VALUES (...), (...)` and
chunks it against `max_query_params` (`compile.rs:194-246`). For 1 000 rows ×
3 columns that is 3 000 placeholders, 3 000 `Value` clones
(`push_bind(val.clone())`), 3 000 `AnyValueKind`s, then 3 000 re-encodes into
`PgArguments`. It then decodes 1 000 returned rows via `RETURNING *` — work
Prisma and Drizzle do not do in the recorded comparison, as
`BenchmarkResults.md` already notes.

Postgres `COPY FROM STDIN` bypasses the parameter budget, the placeholder
parsing, and the per-value round of the extended protocol entirely. `sqlx`
exposes it as `PgConnection::copy_in_raw`. Typical speedups for this shape are
large; the honest position is that we have **not measured it** — the local
Postgres was unreachable this session.

**Fix (after F4):** a `COPY` fast path when the backend is Postgres, there is no
`ON CONFLICT`, and the caller did not ask for `RETURNING`. Fall back to the
current path otherwise. Add `.returning(false)` so callers can opt out of
decoding rows they do not want.

**Gate:** benchmark first, on Postgres. If it is not ≥3× on 10 000 rows, drop it.

---

### F7 — Decode helpers pay for discarded errors · **perf, 15–17× on the miss path**

`decode::boolean` tries `i64` and falls back to `bool`
(`crates/runtime/src/decode.rs:115-120`). `decode::text` / `text_opt` try
`String` and fall back to `Vec<u8>`. On a miss, `sqlx` constructs, boxes and
formats an `Error::ColumnDecode` — including `format!("{index:?}")` — that is
then thrown away.

| 1 000 × `boolean()` | µs |
|---|---:|
| on an `INTEGER` column (first attempt hits) | ~12 |
| on a `TEXT` column (first attempt misses, error boxed and dropped) | **~199** |
| ratio across 4 runs | **15–17×** |

The comment on `boolean` says it "tries the integer path first" because SQLite
stores booleans as integers — which means **Postgres, with a native `BOOL`, takes
the slow path on every boolean column of every row.** A model with three boolean
columns over 1 000 rows burns ~560 µs building errors nobody reads.

**Fix:** the backend is known at decode time once F4 lands, so the fallback
disappears. Before then, cheap partial fix: branch on the column's `AnyTypeInfo`
rather than on a failed decode.

---

### F8 — `SELECT *` and name-based column lookup · **perf, 15–18% of decode**

Empty projection compiles to `SELECT *` (`compile.rs:43-54`), so:

- the wire carries whatever the table has, not what the model needs;
- ordinals are unknown, forcing generated `FromRow` impls to look every column up
  by name — a `HashMap<UStr, usize>` probe per column per row.

| decode 1 000 rows × 3 cols | µs |
|---|---:|
| by name (`decode::direct(row, "id")`) | ~53 |
| by ordinal (`row.get(0)`) | ~45 |
| overhead across 4 runs | **15–18%** |

15–18% of decode time, ~1% end-to-end. Small on its own; worth doing because the
same change is a prerequisite for making the wire format narrow, and codegen
already knows every model's column list.

**Fix:** add `const COLUMNS: &'static [&'static str]` to `Model`; emit it from
codegen; make `select` default to the explicit list; resolve ordinals once per
result set instead of per row.

---

### F9 — `count()` rewrites SQL with string surgery · **correctness on Postgres**

`SelectQuery::count` finds the first `" FROM "` in the compiled SQL and splices
`SELECT COUNT(*)` in front (`query.rs:225-236`). `exists()` does the same. Two
problems:

1. **`ORDER BY` and `LIMIT` survive into the aggregate query.** Observed:

   ```
   base:  SELECT * FROM `users` WHERE (`users`.`age` > ?) ORDER BY `users`.`age` DESC LIMIT 10
   count: SELECT COUNT(*) FROM `users` WHERE (...) ORDER BY `users`.`age` DESC LIMIT 10  -> Ok(1000)
   ```

   SQLite tolerates it. **Postgres will not**: `users.age` appears in `ORDER BY`
   but not in a `GROUP BY`, which is an error. So `.order_by(...).count()` is
   broken on the primary target database.

2. **`" FROM "` is matched textually**, so a raw filter fragment or a string
   literal containing ` FROM ` splices the query in the wrong place.

**Fix:** compile a count/exists query from the AST — skip `order`, `limit`,
`offset`, and swap the projection — rather than editing the string afterwards.
Cheap, and removes a whole class of failure.

---

### F10 — `stream()` is buffered, and a real cursor would be slower · **docs**

`Executor::stream_raw` for `Pool` calls `fetch_all_raw` and replays the buffer
(`executor.rs:130-135`). The doc comment is honest that this "bounds decode cost
rather than peak memory" and anticipates replacing it with a true cursor.

The measurement says do not: `sqlx`'s `.fetch()` stream costs **1.46 µs/row
versus `fetch_all`'s 0.89** — 64% worse. The current buffered implementation is
the fast one.

**Fix:** none to the code. Update the doc comment and `KnownLimitations.md` to
say that streaming is buffered *by choice*, with the number attached, so the next
person does not "fix" it into a regression.

---

## 5. Programme

### Phase 0 — verify (before anything else)

| # | Task | Exit criterion |
|---|---|---|
| **P0-0** | Reproduce F2 on a live Postgres. Run `pg_any_types.rs`. Start the local service (`postgresql-x64-17`, needs elevation) or use `RUPRIZZLE_TEST_PG_URL`. | Table of pass/fail per type, committed to this document. **If F2 does not reproduce, Phase 2 is re-scoped.** |
| **P0-1** | Add a rich-type model (`Uuid`, `DateTime`, `Decimal`, `Json`) to the integration schema and a round-trip test, run under `RUPRIZZLE_REQUIRE_DB=1`. | Test exists and fails for the right reason on Postgres, passes on SQLite. |

### Phase 1 — cheap, high-value, no design risk

| # | Task | Finding | Expected |
|---|---|---|---|
| **P1-1** | `LIMIT 1` in `fetch_optional` / `fetch_one` | F1 | **24×** on non-unique filters |
| **P1-2** | Compile `count()` / `exists()` from the AST | F9 | fixes Postgres breakage |
| **P1-3** | Make `.fetch_all()` on a query with includes a compile error | F3 | removes a silent-wrong-answer class |
| **P1-4** | Stop re-allocating `Value::Str` / `Value::Bytes` on every bind | F4 | small; removes a copy per bind |
| **P1-5** | Correct `BenchmarkResults.md` (§7) | — | stops the misattribution propagating |
| **P1-6** | Document `stream()` as deliberately buffered, with the number | F10 | prevents a future regression |

Phase 1 is small enough to land as one PR per item and carries no API redesign.

### Phase 2 — native backends

| # | Task | Finding |
|---|---|---|
| **P2-1** | `Backend` enum (`Postgres` / `Sqlite`), `Executor` dispatching on it, `Any` retained behind a default feature | F4 |
| **P2-2** | Per-backend `FromRow` from codegen; native types; delete the text round-trip | F2, F7 |
| **P2-3** | `Model::COLUMNS`; explicit projection; ordinal decoding | F8 |
| **P2-4** | Driver options in `PoolConfig`; SQLite `row_buffer_size` default 1 024 | F5 |
| **P2-5** | Postgres `COPY` fast path for `create_many` — **only if P2-5a measures ≥3×** | F6 |

Acceptance for Phase 2: `include_posts` ≤ 13 ms (from 15–17.5), rich types
round-trip natively on Postgres, and no public API break for existing `Any`
users. Measure over five runs, not one — see §2.

### Phase 3 — the SQLite ceiling *(optional, evaluate after Phase 2)*

The 3.1× per-row gap to better-sqlite3 is `sqlx-sqlite`'s thread-per-connection
plus per-row channel handoff. The only way past it is a synchronous backend
(`rusqlite`) behind the same `Executor`. That is a large amount of work to win a
benchmark on a database most users of a schema-first Rust ORM will not deploy.
**Recommendation: do not start this until someone asks for it.** Record the
ceiling honestly instead (§7).

---

## 6. Implementation status

This section records which items have landed, which are blocked, and which are
still open. Commit hashes are from the `perf/research-harnesses` branch.

### Phase 0

| # | Task | Status | Notes |
|---|---|---|---|
| **P0-0** | Reproduce F2 on a live Postgres | **implemented** | Server started with `pg_ctl` (the Windows service still needs elevation). `pg_any_types.rs` run against PostgreSQL 17.10 reproduced all six rich-type failures and the `uuid = text` bind error. |
| **P0-1** | Rich-type integration test | **implemented** | `crates/runtime/tests/rich_types.rs` added. Passes on SQLite; on Postgres it asserts `InsertQuery` fails with a `uuid`/`jsonb` type-mismatch error because `sqlx::Any` sends rich types as text. **verified**: full round-trip now passes on native Postgres. |

### Phase 1

| # | Task | Finding | Status | Notes |
|---|---|---|---|---|
| **P1-1** | `LIMIT 1` in `fetch_optional` / `fetch_one` | F1 | **implemented** | `83a2a9e`. Query compile now injects `limit = Some(1)` for unbounded `fetch_one`/`fetch_optional` calls. |
| **P1-2** | Compile `count()` / `exists()` from the AST | F9 | **implemented** | `83a2a9e`. New `compile::count` and `compile::exists` strip `ORDER BY` / `LIMIT` / `OFFSET`. |
| **P1-3** | Make `.fetch_all()` on a query with includes a compile error | F3 | **implemented** | `83a2a9e`. `fetch_all` is now only defined on `SelectQuery<'db, M, Out, ()>`; `exec()` remains the include-aware path. |
| **P1-4** | Stop re-allocating `Value::Str` / `Value::Bytes` on every bind | F4 | **deferred** | Impossible under `sqlx::Any`: `sqlx::Encode<Any>` requires owned `String`/`Vec<u8>`. Will be resolved by P2-2 native backends. |
| **P1-5** | Correct `BenchmarkResults.md` | — | **implemented** | `83a2a9e` (analysis) and `e513f7e` (re-run and update). Re-ran cross-ORM and attribution harnesses; attribution now separates ORM, `Any`, and driver floor. |
| **P1-6** | Document `stream()` as deliberately buffered | F10 | **implemented** | `KnownLimitations.md` updated in `83a2a9e`; `Executor::stream_raw` doc comment updated to state buffering is deliberate. |

### Phase 2

| # | Task | Finding | Status | Notes |
|---|---|---|---|---|
| **P2-1** | `Backend` enum and `Executor` dispatch; keep `Any` behind a feature | F4 | **implemented** | `53e9791` (P2-1a) and `b127593` (P2-1b). `Pool` is now an enum; `Any` dispatch is wired and tested; native variants are stubbed with `unimplemented!()` until P2-2. |
| **P2-2** | Per-backend `FromRow`, native types, remove text round-trip | F2, F7 | **partially implemented** | `5988da6`: decode helpers are generic over `Row`. `26d4a39`: codegen emits `FromRow<AnyRow>`, `FromRow<PgRow>` and `FromRow<SqliteRow>`. `Executor` now dispatches to native `Postgres`/`Sqlite` pools; `Value` implements `Encode`/`Type` for both; rich types round-trip on Postgres. |
| **P2-3** | `Model::COLUMNS`; explicit projection; ordinal decoding | F8 | **implemented** | `5f21c1c`. `Model::COLUMNS` added; `compile::select` defaults to explicit list; codegen emits `decode::_idx` helpers and ordinal `FromRow`. Measured name-lookup win: ~24% of decode, ~0.7% end-to-end. |
| **P2-4** | Driver options in `PoolConfig`; SQLite `row_buffer_size` default 1 024 | F5 | **implemented** | `PoolConfig` gains `row_buffer_size`; `connect_with` builds a native `Pool::Sqlite` for `sqlite:` URLs and sets `SqliteConnectOptions::row_buffer_size`. Tests and benchmarks updated to compile and pass with native backends. |
| **P2-5** | Postgres `COPY` fast path for `create_many` | F6 | **pending** | Gated on P2-1 and a measured ≥3× improvement; not yet benchmarked. |

### Next steps after this session

The remaining work that is either in progress or unblocked:

1. **P2-5** — Postgres `COPY` fast path for `create_many`. Gated on a live
   Postgres benchmark showing ≥3× improvement.

3. **P1-4** — Stop re-allocating `Value::Str` / `Value::Bytes` on every bind.
   Deferred until native backends land; it is impossible to fix under
   `sqlx::Any`.

### Findings not yet addressed

- **F7** (`decode::boolean` / `text` discarded-error cost): needs P2-2 so the backend is known at decode time.
- **F10**: `KnownLimitations.md` and `Executor::stream_raw` doc comment updated; no code change required.

---

## 7. Benchmark methodology corrections

`docs/BenchmarkResults.md` needs four changes. The measurements are sound; the
analysis is not.

1. **Add a hand-written `sqlx` control column.** Without it, the table measures
   "Rust async stack vs Node sync driver" and reads as "ruprizzle vs Drizzle".
   With it, ruprizzle's actual overhead — 3% — is visible.
2. **Retract the `Any` attribution.** The current text names `sqlx::Any` as the
   main cause of the simple-read gap; measured, `Any` accounts for +0.1% on
   `find_many_1000`. Replace with the per-row floor analysis (§3.2).
3. **Note that `include_posts` compares different query strategies, not
   different ORMs.** ruprizzle and Prisma issue a bounded number of batched
   queries; Drizzle's SQLite path emits a correlated subquery per parent. The
   doc says this in prose already — it belongs next to the number.
4. **Re-run on Postgres before the numbers are cited anywhere public.** Every
   row in the table is SQLite, which is not the primary target, and the ordering
   is dominated by driver architecture that changes completely over a network.

Also worth adding: the `to_sql()` micro-benchmark (ruprizzle 0.57 µs vs Drizzle
8.41 µs) is real but end-to-end irrelevant — full `to_sql()` for select-by-PK
measures 0.42 µs against a 21.6 µs round-trip floor. Presenting it as a
throughput advantage overstates it. It is better framed as what it is: evidence
the builder is not where time goes.

---

## 7. Explicitly not doing

Each of these was measured and rejected. Recorded so the effort is not spent
later.

| Candidate | Measured | Verdict |
|---|---|---|
| Cache `Executor::dialect()` instead of `Box<dyn DbDialect>` per query | **0.018 µs/call** | Noise. 0.0001% of a round trip. |
| Optimise SQL string building / `quote_ident` allocations | full `to_sql()` = **0.42 µs** vs 21.6 µs round-trip | Noise. |
| Faster hasher / arena for include grouping | D − C sign **not stable** across 5 runs | Nothing to win. |
| Avoid the key `dedup` clone in `fetch_children` | **8 µs** per 1 000 keys | Nothing to win. |
| Make `stream()` a true cursor | `.fetch()` is **64% slower per row** | Would be a regression (F10). |
| Parallelise include chunks | 2 chunks at 1 000 keys; bounded by the same pool | Contention, not speedup. |

The pattern: **the ORM layer is not the problem.** Every remaining win is in the
driver boundary or in emitting better SQL.

---

## 8. Risks

| Risk | Mitigation |
|---|---|
| F2 does not reproduce on a live Postgres and Phase 2's urgency is overstated | P0-0 gates Phase 2. Phase 1 is unaffected either way. |
| The `Backend` enum forks generated code and doubles codegen test surface | Keep `Any` as the default feature; make native backends opt-in for one release; snapshot-test both emissions. |
| `COPY` semantics differ from `INSERT` (no `RETURNING`, different error shape) | Restrict the fast path to the cases where they coincide; gate on a measured ≥3×. |
| All numbers are from one Windows workstation, SQLite, single process | Re-run on Postgres (P0-0) and in CI before publishing any of it. |
| Fixing F3 by making `fetch_all` a compile error breaks existing callers | It breaks exactly the callers that are silently getting wrong data. Pre-1.0, that is the right trade; call it out in `CHANGELOG.md`. |

---

## 9. Appendix — raw measurements

Intel Core Ultra 7 265K, 20 logical cores, 32 GB RAM, Windows 11.
`cargo build --release`, SQLite file, single process, `ruprizzle 0.1.0-alpha.3`.

### `select_by_pk`

| Layer | µs/op |
|---|---:|
| native `SqlitePool` `query_as` | 50.0 |
| `AnyPool` `query_as` | 43.8 |
| `AnyPool` + `decode::*` helpers | 48.1 |
| ruprizzle `SelectQuery` | 41.8 |
| `SELECT 1` round-trip floor | 21.6 |

Spread is within run-to-run variance at this scale; the operation is dominated by
the ~22 µs round trip. No layer is distinguishable from another.

### `find_many_1000` / `find_filtered_ordered`

| Layer | find_many | filtered+ordered |
|---|---:|---:|
| native `SqlitePool` `query_as` | 1 628.3 | 1 604.2 |
| `AnyPool` `query_as` | 1 630.7 | 1 614.2 |
| `AnyPool` + `decode::*` | 1 561.1 | 1 672.5 |
| ruprizzle `SelectQuery` | 1 679.4 | 1 651.4 |
| ruprizzle vs native | **1.03×** | **1.03×** |

### `include_posts` (1 000 + 10 000), 5 runs

| Layer | run 1 | 2 | 3 | 4 | 5 |
|---|---:|---:|---:|---:|---:|
| A native, 2 queries, no grouping | 13.77 | 13.78 | 13.95 | 14.93 | 13.92 |
| B `Any`, 2 queries, struct decode | 17.18 | 15.93 | 16.26 | 15.54 | 17.28 |
| C B + hand-rolled grouping | 16.91 | 16.44 | 16.76 | 15.99 | 17.54 |
| D ruprizzle `.include().exec()` | 17.46 | 15.85 | 15.22 | 16.25 | 15.75 |

D − C: +0.55, −0.59, −1.54, +0.26, −1.79 ms. Mean is slightly negative; the sign
is not stable. Sanity check on D every run:
`users=1000 loaded=1000 attached_posts=10000`.

### Hotspots

| Measurement | value |
|---|---:|
| `fetch_optional` without `LIMIT` (1 000 matches) | 1 561 – 1 705 µs |
| same with `.limit(1)` | 44 – 64 µs |
| → ratio over 5 runs | 24×, 39×, 34×, 38×, 34× |
| `decode::boolean`, first attempt hits (×1 000) | ~12 µs |
| `decode::boolean`, first attempt misses (×1 000) | ~199 µs |
| → ratio over 4 runs | 16×, 15×, 17×, 16× |
| decode 1 000×3 by name vs ordinal | 15%, 18%, 17%, 18% |
| `Executor::dialect()` | 0.018 µs |
| `to_sql()` select-by-PK | 0.424 µs |
| compile `IN` (1 000 keys) | 28.96 µs → 3 053 B, 1 000 binds |
| dedup 1 000 keys | 8.09 µs |

### `row_buffer_size` sweep

| size | users (1 k) | posts (10 k) |
|---:|---:|---:|
| 50 | 920.2 µs | 9 313.3 µs |
| 200 | 856.9 µs | 8 538.2 µs |
| 1 000 | 814.8 µs | 7 394.6 µs |
| 4 096 | 798.8 µs | 7 198.9 µs |
| 16 384 | 795.5 µs | 7 192.7 µs |

---

## 10. See also

- [`docs/BenchmarkResults.md`](../../../docs/BenchmarkResults.md) — the cross-ORM run this responds to (needs §6 corrections)
- [`docs/Performance.md`](../../../docs/Performance.md) — Postgres vs hand-written `sqlx`
- [`docs/KnownLimitations.md`](../../../docs/KnownLimitations.md) — needs the F2 and F10 corrections
- `ProjectPlan/ImplementationPlan/ImplPlan10AppendixDecisions.md` — ADR-009, the `sqlx::Any` decision this proposes to revisit
