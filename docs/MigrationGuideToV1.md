# Migration guide: pre-1.0 to `1.0.0`

This covers every API-shape change between the `0.1.1-beta.1` publish (commit `95ec17b`,
2026-08-13) and current `dev-v0-2` `HEAD` (`0.4.0-beta.2` and later), which is what will
become `1.0.0` once `docs/Stability.md`'s W6 workstream and the `1.0.0-rc.1` feedback
window (W6-04) complete.
Purely additive features (savepoints, array binds, streaming, new query operators, MySQL,
`db pull`, seeding, migration squashing, rename detection, metrics — see the parity table in
`ProjectPlan/v1/PathToStableV1.md`) are not covered here unless they changed an existing
signature; see `CHANGELOG.md`'s `[Unreleased]` section for the full feature list.

If you generated a schema client against `0.1.1-beta.1`, regenerate it
(`ruprizzle generate`) after upgrading — none of the changes below require editing your
`.ruprizzle` schema itself, but generated code should be refreshed to pick up any codegen-side
fixes.

## 1. Queries with `.include(...)` no longer support `.fetch_one()` / `.fetch_optional()`

**Why:** `SelectQuery::fetch_one()` and `fetch_optional()` used to compile even when the query
had `.include(...)` attached, but they never actually loaded the included relations — every
`Related<T>` field silently came back `Related::Absent`. Calling `.get()` on it then panicked
with a message that only mentioned `.include()`, not the terminal method that was the actual
bug. (BUG-04 in `CHANGELOG.md`.)

**What changed:** a query with `.include(...)` is now a different builder state. It only
exposes `.exec_one()` / `.exec_optional()`, which load the requested relations before
returning. `.fetch_one()` / `.fetch_optional()` remain available, but only on queries with no
`.include(...)` calls.

Before:

```rust
let user = User::query()
    .filter(User::id().eq(1))
    .include(User::posts())
    .fetch_one(&pool)   // compiled, but `user.posts` was always Related::Absent
    .await?;

let posts = user.posts.get(); // panicked: "call .include() first"
```

After:

```rust
let user = User::query()
    .filter(User::id().eq(1))
    .include(User::posts())
    .exec_one(&pool)     // relations are actually loaded
    .await?;

let posts = user.posts.get(); // works
```

If you don't need the included relation on a particular call, drop `.include(...)` and keep
`.fetch_one()` / `.fetch_optional()` — no behavior changes for queries that never called
`.include(...)`.

## 2. `Related::get()`'s panic message now points at the right fix

Not an API-shape change, but worth knowing during migration: if you still hit the panic in
`Related::get()`, its text now says to call `.exec()` / `.exec_one()` / `.exec_optional()`
rather than only mentioning `.include()`. If you see this message after upgrading, it means
you're still on a `.fetch_*()` terminal with `.include(...)` attached — apply the fix in
section 1.

## 3. Pool exhaustion is now a typed, matchable error variant

**Why:** pool exhaustion on the native drivers used to surface as a stringly-typed error
message, which meant catching it required matching on `Display` text — fragile across
releases and inconsistent with the rest of `Error`'s variants.

**What changed:** `Error::PoolExhausted { backend: &'static str }` was added, with a stable
`kind() == "pool_exhausted"`.

Before:

```rust
match pool.acquire().await {
    Err(e) if e.to_string().contains("exhausted") => { /* handle it */ }
    Err(e) => return Err(e),
    Ok(conn) => conn,
};
```

After:

```rust
match pool.acquire().await {
    Err(Error::PoolExhausted { backend }) => { /* handle it, backend tells you which driver */ }
    Err(e) => return Err(e),
    Ok(conn) => conn,
};
```

If you match on `Error` exhaustively (`match e { ... }` without a wildcard arm), adding this
variant requires adding an arm for it — this is the one place the change is a compile-time
break rather than a silent behavior difference. If you match with a wildcard (`_ => ...`) or
only match specific variants you care about, no change is required, but you can now catch
pool exhaustion specifically where you previously couldn't.

## 4. `InsertManyQuery` now validates row shape before building SQL

**Why:** batched inserts where a later row had different columns (or the same columns in a
different order) than row 0 either produced silently wrong SQL or an opaque driver error,
depending on the database. (BUG-09.)

**What changed:** `InsertManyQuery::exec` and nested `with_related` child inserts now check
every row against row 0's column set and order before running, and return a descriptive
`Error` naming the offending row and column if they don't match — instead of a bad query or a
generic driver error.

```rust
// This now fails fast with a clear error identifying which row/column is
// inconsistent, instead of either silently reordering values into the wrong
// columns or bubbling up an opaque driver error:
User::insert_many(vec![
    User::new().name("Alice").email("a@example.com"),
    User::new().email("b@example.com").name("Bob"), // same columns, different order
])
.exec(&pool)
.await?;
```

No code changes are required to adopt this — it only affects call sites that were already
relying on (or accidentally triggering) the old inconsistent-row behavior. If your batch
inserts always build rows with `User::new()` and the same builder call order, or all rows come
from the same struct/iterator shape, you will not observe any difference.

## 5. `IncludeList` distributes children to every matching parent

**Why:** `.include(...)` on a one-to-many relation with a shared join key only attached the
loaded children to the *first* parent row that matched, leaving every other parent with the
relation empty. (BUG-08.)

**What changed:** children are now correctly attached to every parent row sharing the join
key. This is a bug fix, not an API signature change, but if your code compensated for the old
behavior (e.g., deduplicating parents before calling `.include(...)`, or only reading the
relation off the first row in a group), that workaround is no longer necessary and may now
produce duplicate data if left in place.

## 6. `RusqliteTransaction` no longer implements `Clone`

**Why:** `Clone` on a transaction handle let one pooled connection be returned to the pool
twice — once per clone dropped — corrupting the pool's accounting. (BUG-06.)

**What changed:** if your code cloned a `RusqliteTransaction` (only reachable with the
`sqlite-rusqlite` feature enabled), that call site no longer compiles. There is no supported
replacement for cloning a transaction handle — restructure the call site to pass `&Transaction`
(a shared reference) to whatever needed the clone, or to scope the work inside a single
transaction borrow instead.

## 7. New `PoolConfig::reset_on_recycle` field (additive, but affects exhaustive struct literals)

**What changed:** `PoolConfig` gained `reset_on_recycle: bool` (default `false`), which
selects `deadpool`'s `Clean` recycling for the native `tokio-postgres` backend — discarding
session state on every checkout, at roughly 2x the per-checkout latency, in exchange for
defence-in-depth against any session-state leak beyond the abandoned-transaction rollback
fix in section 8.

If you construct `PoolConfig` with `..Default::default()` or a builder method, nothing
changes. If you construct it as a full struct literal naming every field, add
`reset_on_recycle: false` (or your preferred value) to keep compiling.

## 8. Abandoned transactions on native drivers now roll back instead of leaking or corrupting state

Not an API-shape change — no signature changed — but a correctness fix worth knowing if you
relied on (or worked around) the old behavior:

- **`rusqlite`:** a transaction dropped without an explicit `.commit()`/`.rollback()` (which
  happens on every early return via `?`) used to permanently lose its connection from the
  pool; enough of these exhausted the pool and required a process restart. It now rolls back
  and returns the connection on drop, without panicking. A related bug made
  `RusqlitePool::acquire` panic with a divide-by-zero once the pool was fully exhausted by
  leaked connections — this is now the typed `Error::PoolExhausted` from section 3.
- **`tokio-postgres`:** an abandoned transaction didn't leak a connection, but recycled it
  back into the pool with `BEGIN` still open — so the next request to receive that connection
  silently ran inside the *previous* request's transaction. `Drop` now spawns a `ROLLBACK`
  before the connection is released.

If your application caught and ignored errors from `begin()` under sustained load (as a
workaround for the old exhaustion panic/leak), you can remove that workaround — pool exhaustion
now surfaces as the typed `Error::PoolExhausted` described in section 3 rather than a panic.

## 9. New escape hatch: the `raw!` macro and `RawFragment`

Purely additive, but worth flagging if you previously worked around missing raw-SQL support by
reaching into `sqlx` directly: `ruprizzle_macros::raw!` (re-exported from `ruprizzle`) now
provides an injection-safe raw SQL fragment with bound parameters, usable inside the query
builder. This does not replace or change any existing method — it is a new capability, not a
signature change.

## Not covered by this guide

Everything else that changed since `0.1.1-beta.1` is additive (new query operators, savepoints,
array binds, streaming, MySQL support, prepared statements, metrics export, migration
squashing, rename detection) and does not require call-site changes to code that already
compiled against `0.1.1-beta.1`. See `CHANGELOG.md`'s `[Unreleased]` section for the complete
list, and `ProjectPlan/v1/PathToStableV1.md` section 5 for how each feature maps to its
workstream.
