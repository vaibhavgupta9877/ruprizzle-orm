# Pending Bugs — Pre-v1 Analysis

**Analysed:** `0.1.1-beta.1` (commit `c3ef7f0` + `af3ce27`)
**Date:** 2026-08-13
**Method:** Source audit of the runtime, pool, and relation-loading layers, with an
executable reproducer written for every finding marked CONFIRMED. Reproducers were run
against a real SQLite database and then deleted; none remain in the tree.
**Fix plan:** [`../../ProjectPlan/BugFixes.md`](../../ProjectPlan/BugFixes.md)

---

## Summary

| ID | Finding | Severity | Status |
|---|---|---|---|
| [BUG-01](#bug-01) | Dropping a `Tx` permanently leaks a `rusqlite` connection; the pool dies | **Critical** | ✔️ **Fixed** (FIX-01) |
| [BUG-02](#bug-02) | `RusqlitePool::acquire` panics with divide-by-zero when all connections are in transactions | **Critical** | ✔️ **Fixed** (FIX-02) |
| [BUG-03](#bug-03) | Dropping a `Tx` returns a `tokio-postgres` connection to the pool with an open `BEGIN` | **Critical** | ✔️ **Fixed** (FIX-03) — reproduced on PG 17.10 first |
| [BUG-04](#bug-04) | `fetch_one` / `fetch_optional` silently discard `.include()`, then panic on access | **High** | ✔️ **Fixed** (FIX-04) |
| [BUG-05](#bug-05) | Divide-by-zero panic on an insert with an empty column set | **High** | ✔️ **Fixed** (FIX-05) |
| [BUG-06](#bug-06) | `RusqliteTransaction` derives `Clone`, allowing a connection to be returned twice | Medium | ✔️ **Fixed** (FIX-06) |
| [BUG-07](#bug-07) | `PoolStats` always reports zeros for the `rusqlite` backend | Medium | ✅ Confirmed by inspection |
| [BUG-08](#bug-08) | `IncludeList` drops children when two parents share a join key | Medium | ✔️ **Fixed** (FIX-08) |
| [BUG-09](#bug-09) | `InsertManyQuery` accepts heterogeneous rows and derives the column set from row 0 | Medium | ✅ Confirmed by inspection |
| [BUG-10](#bug-10) | `driver=rusqlite` without the feature yields an opaque sqlx error | Medium | ✅ Confirmed, reproduced |
| [PERF-01](#perf-01) | Full-table include fast path loads the entire child table into memory, unbounded | **High** | Confirmed by inspection |
| [PERF-02](#perf-02) | `Tx` takes a mutex and re-boxes the dialect on every statement | Medium | Confirmed by inspection |
| [PERF-03](#perf-03) | `fetch_optional` decodes the full result set, then `remove(0)` | Low | Confirmed by inspection |
| [PERF-04](#perf-04) | `dedup` clones every join key; grouping allocates a `Vec` per parent | Low | Confirmed by inspection |
| [PERF-05](#perf-05) | `is_postgres` acquires and drops a pooled connection to read a constant | Low | Confirmed by inspection |

**Headline:** three critical defects, all in transaction lifecycle management, all with the
same root cause — **neither `Tx` nor either native transaction type implements `Drop`.**
The `sqlx`-backed variants are saved by `sqlx::Transaction`'s own `Drop`; the two native
driver paths added in `0.1.0-alpha.3` and `0.1.1-beta.1` have no such protection. The
`rusqlite` path is the one `docs/BenchmarkResults.md` recommends for latency-sensitive
SQLite work, and `docs/README.md` markets on its speed.

**Why the test suite did not catch these.** All 218 tests commit or roll back their
transactions explicitly. None drops one. None opens more transactions than the pool has
connections. None calls `fetch_one` with an `.include()`. The gaps are in the shape of the
tests, not their number — which is precisely the argument for the mutation testing proposed
in the v1 plan's W4-05.

---

## Critical

### BUG-01

**Dropping a `Tx` permanently leaks a `rusqlite` connection; the pool dies after N drops.**

- **Location:** `crates/runtime/src/rusqlite.rs:229-333`, `crates/runtime/src/tx.rs`
- **Severity:** Critical — permanent, unrecoverable resource exhaustion in a production path

`RusqlitePool::begin_transaction` **removes** the connection from the pool
(`conns.pop()`, `rusqlite.rs:144-152`). It is returned only by
`RusqliteTransaction::commit` or `::rollback`, both of which consume `self` and call
`pool.return_conn(conn)`.

There is **no `Drop` impl** on `RusqliteTransaction`, and none on `Tx`. So any path that
drops a transaction without explicitly committing or rolling back loses that connection
from the pool forever. That includes the single most common pattern in Rust:

```rust
let tx = pool.begin().await?;
tx.execute_raw(sql, binds).await?;   // <-- `?` returns early, dropping `tx`
tx.commit().await?;                  //     never reached
```

Every failed operation permanently shrinks the pool by one. Once the pool empties, every
subsequent `begin()` fails with `"rusqlite connection pool exhausted"` and **never
recovers** — no timeout, no reaping, no reconnection. The process must be restarted.

The doc comment at `rusqlite.rs:224-228` states the intended invariant — *"only returned on
commit or rollback"* — without noticing that the third case exists.

**Reproducer** (run against `--features sqlite-rusqlite`, pool of 2):

```
dropped tx 0
dropped tx 1
after 2 drops, begin -> Err("rusqlite connection pool exhausted")
```

**Fix:** implement `Drop` for `RusqliteTransaction` issuing `ROLLBACK` and returning the
connection. Because `commit`/`rollback` consume `self`, the fields must move into an
`Option` (or the type must be split) so `Drop` can tell "already finished" from "abandoned".
Emit a `tracing::warn!` on the abandoned path — silently rolling back is correct but worth
observing.

---

### BUG-02

**`RusqlitePool::acquire` panics with divide-by-zero when every connection is checked out.**

- **Location:** `crates/runtime/src/rusqlite.rs:134`
- **Severity:** Critical — a library panic reachable from ordinary application input

```rust
let idx = self.inner.next.fetch_add(1, Ordering::Relaxed) % conns.len();
```

`conns` is drained by `begin_transaction`'s `pop()`. When as many transactions are open as
the pool has connections, `conns.len()` is `0` and the remainder operation panics:

```
thread panicked at crates\runtime\src\rusqlite.rs:134:19:
attempt to calculate the remainder with a divisor of zero
```

This needs no dropped transactions and no misuse — with `max_connections = 10`, ten
concurrent transactions plus one ordinary query is enough. Combined with BUG-01 it is
strictly worse: each leaked connection brings the pool closer to the panicking state, so a
service that merely returns errors early will eventually start panicking on unrelated
queries.

This violates the project's own stated invariant — *"No new panics on any path reachable
from user input; `Related::get()` remains the single sanctioned panic"*
(`ProductionReadinessPlan.md`, Global Constraints). **`cargo xtask harden` does not catch
it**, because the audit greps for `unwrap()`/`expect()` and this is an arithmetic panic.

**Reproducer:** pool of 1, open one transaction, then run any query.

**Fix:** return `Error::PoolExhausted` (or reuse the existing exhaustion error) when
`conns.is_empty()`, before the modulo. Then extend `xtask harden` to flag bare `%` and `/`
on non-constant divisors in library source — the audit's blind spot is the more important
half of this fix.

---

### BUG-03

**Dropping a `Tx` returns a `tokio-postgres` connection to the pool with an open `BEGIN`.**

- **Location:** `crates/runtime/src/tokio_postgres.rs:64-70, 120-126, 160-190`
- **Severity:** Critical — silent cross-request data-integrity hazard
- **Status:** **Reproduced and fixed.** Confirmed against PostgreSQL 17.10 with the
  `postgres-tokio-postgres` feature before the fix was written: after abandoning a
  transaction on a single-connection pool, the *next* write issued through the pool was
  invisible to a second session, because it executed inside the abandoned transaction. See
  `crates/runtime/tests/tx_lifecycle.rs`.

`TokioPostgresPool::begin` issues `BEGIN` and wraps a `deadpool_postgres::Object`.
`TokioPostgresTransaction` has **no `Drop` impl**. `Object`'s own `Drop` returns the
connection to `deadpool`, so unlike BUG-01 there is no leak — which makes this worse, not
better: the connection goes back into rotation **with a transaction still open**.

The recycling method cannot save it:

```rust
recycling_method: if config.test_before_acquire {
    RecyclingMethod::Verified   // runs a check query — succeeds inside a transaction
} else {
    RecyclingMethod::Fast       // default: no query at all
},
```

`PoolConfig::test_before_acquire` defaults to `false`, so the default configuration does no
cleanup whatsoever. The next request to receive that connection executes its statements
**inside the previous request's abandoned transaction**. Depending on what the abandoned
transaction did, the next request either has its writes silently rolled back later, or hits
`current transaction is aborted, commands ignored until end of transaction block` on every
statement until the connection is recycled by age.

This is a data-integrity bug that crosses request boundaries, and it is silent.

**Fix:** implement `Drop` for `TokioPostgresTransaction`. Since `ROLLBACK` is async and
`Drop` is not, spawn the rollback onto the runtime before the `Object` is released — the
same approach `sqlx::Transaction` uses. Switching to `RecyclingMethod::Clean` is worth doing
as defence in depth but is not sufficient alone.

---

## High

### BUG-04

**`fetch_one` / `fetch_optional` silently discard `.include()`, then panic on access.**

- **Location:** `crates/runtime/src/query.rs:63, 186-235`
- **Severity:** High — silent wrong data, surfacing as a panic with a misleading message

`fetch_all`, `stream`, and `page` are deliberately declared on
`impl<'db, M, Out> SelectQuery<'db, M, Out, ()>` — the `()` bound makes it a **compile
error** to call them on a query carrying includes. The doc comments say exactly why:

> *"Only available when the query has no `.include(...)`: fetching all rows without loading
> declared includes would silently return the wrong data."*

`fetch_optional` and `fetch_one` sit on the **generic** `impl<'db, M, Out, I>` at line 63.
They accept includes, and they never call `self.includes.load(...)`. The guard was applied
to three of the five terminal methods and missed two.

The user-visible result is worse than a wrong answer. The relation is left `Related::Absent`,
and `Related::get()` panics with:

> `relation was not loaded — add an `.include()` to the query`

The user *did* add the include. The error message actively misdirects them.

**Reproducer:**

```rust
let alice: User = SelectQuery::<User>::new(&pool)
    .filter(USER_ID.eq(1))
    .include(posts())
    .fetch_one()
    .await?;
// is_absent after explicit .include() = true
```

**Fix:** move `fetch_optional`/`fetch_one` to the `I = ()` impl to match the other three,
and add include-aware `exec_optional`/`exec_one` on the `IncludeSet` impl alongside `exec`.
Both halves are needed: the bound alone would remove the ability to fetch a single row with
relations, which is the single most common ORM operation there is.

**Status:** Fixed by FIX-04. `SelectQuery::exec_one` and `exec_optional` now live on the
include-aware impl and load relations with `is_full_table = false`; the non-include
`fetch_one`/`fetch_optional` are only available on `SelectQuery<'_, M, Out, ()>`.

---

### BUG-05

**Divide-by-zero panic when an insert row has an empty column set.**

- **Location:** `crates/runtime/src/query.rs:755` and `crates/runtime/src/query.rs:642`
- **Severity:** High — library panic on user input

```rust
let cols_per_row = self.rows[0].len() as u32;
let chunk_size = (max / cols_per_row).max(1) as usize;
```

`.max(1)` is applied to the *result* of the division, not the divisor. An empty column set
makes `cols_per_row == 0` and the division panics:

```
thread panicked at crates\runtime\src\query.rs:755:26:
attempt to divide by zero
```

The identical pattern appears in the nested-insert path at line 642, reachable through
`InsertQuery::with_related` with an empty child row.

Like BUG-02 this is an arithmetic panic invisible to `cargo xtask harden`.

**Fix:** `let chunk_size = (max / cols_per_row.max(1)).max(1) as usize;` — and reject the
empty-row case up front with a real error, since an insert with no columns is a caller
mistake worth naming rather than silently turning into `INSERT INTO t DEFAULT VALUES`.

**Status:** Fixed by FIX-05. `InsertManyQuery::exec` and `InsertQuery::exec_nested` now
return a clear `Error::Message("insert row has no columns")`; regression tests live in
`crates/runtime/tests/insert_validation.rs`.

---

## Medium

### BUG-06

**`RusqliteTransaction` derives `Clone`, allowing one connection to be returned twice.**

- **Location:** `crates/runtime/src/rusqlite.rs:229-233`

```rust
#[derive(Debug, Clone)]
pub(crate) struct RusqliteTransaction {
    pool: RusqlitePool,
    conn: Arc<std::sync::Mutex<rusqlite::Connection>>,
}
```

`commit` and `rollback` consume `self` and push `conn` back to the pool. With `Clone`, two
handles to the same connection exist, and committing both pushes the **same `Arc` into the
pool twice**. The pool then hands the same physical connection to two callers who each
believe they hold it exclusively — and `begin_transaction`'s `pop()` can hand it to two
concurrent transactions, whose statements interleave on one connection with one `BEGIN`.

Currently `pub(crate)`, so this is latent rather than live, and no internal caller clones
it. It should not be reachable at all.

**Fix:** remove the `Clone` derive. If an internal caller needs it, that caller is the bug.

### BUG-07

**`PoolStats` always reports zeros for the `rusqlite` backend.**

- **Location:** `crates/runtime/src/pool.rs:63-87`

```rust
Pool::SqliteNative(_) => 0,   // in both size() and num_idle()
```

`stats()` derives `in_use` as `size - idle`, so for the native SQLite backend every field is
`0` — permanently. Any readiness dashboard, saturation alert, or capacity graph built on
`PoolStats` shows a flatline for the backend the docs recommend for performance, and it
looks like a healthy idle pool rather than a broken metric.

This directly undercuts a feature the readiness assessment scored as a fix for a blocker
(§5.4, "Untunable pool — RESOLVED").

Note that the same connection accounting BUG-01 corrupts is what these numbers should be
reporting: had `size()` been implemented, the leak would have been visible on a graph.

**Fix:** report `inner.conns.len()` for `num_idle` and the configured capacity for `size`.
Both are already tracked. While here, add `PoolStats` coverage for every `Pool` variant to
`crates/runtime/tests/pool_config.rs` — the existing test only exercises the `Any` path.

### BUG-08

**`IncludeList` drops children when two parents share a join key.**

- **Location:** `crates/runtime/src/include.rs:335-349`

```rust
for (i, parent) in parents.iter().enumerate() {
    parent_index.entry((self.get)(parent)).or_insert(i);
}
```

`or_insert` keeps only the **first** parent index per key. Children are then routed to that
one bucket, and every other parent sharing the key is handed an empty `Related::Loaded(vec![])`
— which reads as "this parent genuinely has no children," not as a bug.

For the common case where the include key is the parent's primary key this cannot trigger.
It triggers whenever the relation is keyed on a non-unique parent column, which the API
permits: `IncludeList::new` takes an arbitrary `get: fn(&M) -> Key`.

Note the sibling `IncludeOne` handles the shared-key case correctly (map lookup plus clone,
lines 522-536) — the asymmetry is what makes this look unintentional rather than a
documented restriction.

**Fix:** build `HashMap<Key, Vec<usize>>` and push the children into every matching bucket,
cloning as `IncludeOne` does. This requires `C: Clone`, matching the bound `IncludeOne`
already carries for exactly this reason.

**Status:** Fixed by FIX-08. `IncludeList::load` now indexes parents by `Vec<usize>`, clones
children only when more than one parent shares a key, and the first matching parent receives
the original child row.

### BUG-09

**`InsertManyQuery` accepts heterogeneous rows and derives the column set from row 0.**

- **Location:** `crates/runtime/src/query.rs:724-772`

`row()` and `rows()` accept any `(&'static str, Value)` sequence, with no check that every
row carries the same columns in the same order. Both the chunk size (line 754) and the
generated column list come from `self.rows[0]`. Rows that disagree are either bound to the
wrong columns or produce a parameter-count mismatch, surfacing as an opaque driver error far
from the call site.

**Fix:** validate on `exec` that every row has the same column set as row 0 and return a
`Error::Message` naming the first offending row index and the differing columns. Cheap
relative to a round trip, and it converts a silent data-corruption path into a clear error.

### BUG-10

**`driver=rusqlite` without the feature enabled produces an opaque sqlx error.**

- **Location:** `crates/runtime/src/pool.rs:360-368`

`strip_driver_param` is called only *inside* the `#[cfg(feature = "sqlite-rusqlite")]`
branch. Without the feature, the URL keeps the parameter and is handed to
`SqliteConnectOptions::from_str`:

```
Err("sqlx error: error with configuration: unknown query parameter `driver` while parsing connection URL")
```

Nothing mentions the feature flag. `docs/BenchmarkResults.md` and the readiness assessment
both tell users to *"add `driver=rusqlite` to the SQLite URL"* to get the fast path, so this
is the error a user hits by following the documentation. The same applies to
`driver=tokio-postgres` on the Postgres branch.

**Fix:** detect the parameter unconditionally and, when the corresponding feature is off,
return a clear error naming the Cargo feature to enable.

---

## Performance

### PERF-01

**The full-table include fast path loads the entire child table into memory, unbounded.**

- **Location:** `crates/runtime/src/include.rs:46-52`
- **Severity:** High — an OOM path reachable from an ordinary query

```rust
if full_table && filter is empty && order.is_empty() && limit.is_none() {
    return SelectQuery::<C>::new(exec).fetch_all().await;
}
```

When the parent query is unconstrained, the loader skips the `IN` list and fetches **every
row of the child table**. The reasoning in the comment is sound — all children belong to
some parent, so the result is correct — and it is a real win on small tables.

But there is no ceiling. `User::query(&db).include(User::posts()).exec()` on a database with
a hundred users and fifty million posts materialises all fifty million rows in a `Vec<Post>`
before grouping. The non-fast path would have chunked at 32,756 keys per query and still
returned the same fifty million rows, so this is a difference of degree — but the fast path
removes the only natural back-pressure, and `child_full_table` propagates the same behaviour
down every nested level.

Note this interacts with the buffered `stream()`: there is currently no way to consume a
large include incrementally.

**Fix:** gate the fast path on a row-count estimate or a configurable ceiling
(`PoolConfig::full_table_include_limit`, defaulting to something like 100k), falling back to
chunked `IN` above it. Document the threshold.

### PERF-02

**`Tx` takes a mutex and re-boxes the dialect on every statement.**

- **Location:** `crates/runtime/src/tx.rs:105-113`, `Tx { inner: Mutex<Option<TxInner>> }`

`Tx::dialect()` returns `Box<dyn DbDialect>`, allocating on every call, and every statement
locks the `Mutex<Option<TxInner>>`. For a transaction issuing many small statements — the
bulk-insert and nested-write paths both do — this is a per-statement allocation plus lock on
a value that is provably single-owner for the transaction's lifetime.

**Fix:** cache the dialect in the `Tx` at `begin()` and return `&dyn DbDialect`. The same
`Box<dyn DbDialect>`-per-call pattern appears on `Executor::dialect()` and is worth
measuring there too — it sits on the hot path of every query compile.

### PERF-03

**`fetch_optional` decodes the full result set, then `remove(0)`.**

- **Location:** `crates/runtime/src/query.rs:190-215`

`limit` is forced to `Some(1)` only when the caller set none. A caller who set
`.limit(1000)` and then called `fetch_optional` decodes all 1000 rows and discards 999, and
`v.remove(0)` shifts the whole vector to return the first. Use `into_iter().next()`, and
consider overriding the limit unconditionally.

### PERF-04

**`dedup` clones every join key; grouping allocates a `Vec` per parent.**

- **Location:** `crates/runtime/src/include.rs:134-139, 340-343`

`dedup` inserts `k.clone()` into the `HashSet` for every key, so every include pays one key
clone per parent row — for `String` keys that is an allocation per parent. Grouping then
allocates `parents.len()` separate `Vec<C>`s with `with_capacity(bucket_hint)`.

Both are modest, but they sit inside the loader that `docs/BenchmarkResults.md` markets as
~2× Sea-ORM and ~7× Prisma, so they are worth the measurement.

### PERF-05

**`is_postgres` acquires and drops a pooled connection to read a constant.**

- **Location:** `crates/migrate/src/runner.rs`

Carried forward from the readiness assessment (finding #11), unchanged. One pooled acquire
per `apply_all` purely to read `backend_name()`, which `Pool::provider()` already knows
without touching the database.

---

## Cross-cutting observations

**One root cause, three critical bugs.** BUG-01, BUG-02, and BUG-03 are all the transaction
lifecycle of the native driver paths. `sqlx::Transaction` implements `Drop` and rolls back;
both hand-written replacements omitted it, and `Tx` does not compensate. The fix is a
`Drop` impl on each native transaction type plus a `Tx`-level test that every backend
survives an abandoned transaction.

**`xtask harden` has an arithmetic blind spot.** BUG-02 and BUG-05 are both panics in
library code reachable from user input, and both pass the panic audit because it greps for
`unwrap()`/`expect()`. The audit's per-crate budget system is good; its definition of
"panic" is too narrow. Extending it is a higher-leverage fix than either individual bug.

**The suite's shape, not its size.** 218 tests, zero of which drop a transaction, exhaust a
pool, call `fetch_one` with an include, or insert an empty row. Every finding above is a
state the tests never construct. This is the concrete case for W4-05 (mutation testing) in
the v1 plan.

**Severity is concentrated in the newest code.** `crates/migrate` — the oldest, most
property-tested, most audited component — produced no findings this pass. `rusqlite.rs` and
`tokio_postgres.rs`, both added in the last two releases to chase benchmark numbers,
produced four of the five worst. The performance work outran the test discipline that
covers the rest of the codebase.

---

*Analysis method: targeted source audit of `crates/runtime` (pool, tx, query, include,
rusqlite, tokio_postgres) at commit `af3ce27`, with executable reproducers for BUG-01,
BUG-02, BUG-04, BUG-05, and BUG-10 run against real SQLite databases via
`cargo test -p ruprizzle --features sqlite-rusqlite`. Reproducer files were removed after
confirmation; the working tree is unchanged by this analysis. BUG-03 is reasoned from source
and the `deadpool-postgres` recycling contract and must be confirmed against a live
Postgres before its fix is validated.*
