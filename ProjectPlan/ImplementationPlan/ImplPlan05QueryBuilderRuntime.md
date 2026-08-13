# ImplPlan 05 — Query Builder & Runtime (Phase P4)

**Status:** COMPLETE — the query builder, runtime, and transaction surface shipped; all P4 tasks below are treated as complete.
**Duration:** 5 days · **Owners:** Vaibhav Gupta (filter algebra, typestate), Vaibhav Gupta (DML builders, tx)
**Exit gate G4:** full CRUD round-trips against live Postgres **and** SQLite via
the `both_dbs!` harness.

---

## The two-API promise

Same engine, two surfaces. This is the core "best of both" bet of the project.

```rust
// Drizzle flavour — you can see the SQL in the shape of the call
let rows = db.select::<User>()
    .columns((user::ID, user::EMAIL))
    .filter(user::ROLE.eq(Role::Admin).and(user::CREATED_AT.gt(cutoff)))
    .order_by(user::CREATED_AT.desc())
    .limit(20)
    .fetch_all()
    .await?;

// Prisma flavour — ergonomic, relation-aware
let users = db.user()
    .find_many()
    .filter(user::EMAIL.contains("@acme.com"))
    .include(user::posts().filter(post::PUBLISHED.eq(true)).take(5))
    .order_by(user::CREATED_AT.desc())
    .paginate(Cursor::after(last_id), 20)
    .exec()
    .await?;
```

Both build the same `SelectQuery<M>` and go through one SQL compiler.

---

## P4-01 · Filter algebra

**Owner:** Vaibhav Gupta · **Est:** 6h · File: `crates/runtime/src/filter.rs`

```rust
pub struct Filter<M> { node: FilterNode, _m: PhantomData<fn() -> M> }

enum FilterNode {
    Cmp   { table: &'static str, column: &'static str, op: CmpOp, value: Value },
    Null  { table: &'static str, column: &'static str, negated: bool },
    In    { table: &'static str, column: &'static str, values: Vec<Value>, negated: bool },
    And(Vec<FilterNode>),
    Or(Vec<FilterNode>),
    Not(Box<FilterNode>),
    /// Escape hatch: `raw!("{} @> {}", col, json)` with bound params, never
    /// string interpolation of user data.
    Raw   { sql: String, binds: Vec<Value> },
    /// Correlated EXISTS over a relation — see ImplPlan06.
    RelationExists { relation: &'static str, inner: Box<FilterNode>, negated: bool },
}

impl<M> Filter<M> {
    pub fn and(self, other: Filter<M>) -> Filter<M>;
    pub fn or(self, other: Filter<M>)  -> Filter<M>;
    pub fn not(self) -> Filter<M>;
}

pub fn all<M>(fs: impl IntoIterator<Item = Filter<M>>) -> Filter<M>;
pub fn any<M>(fs: impl IntoIterator<Item = Filter<M>>) -> Filter<M>;
```

**Flattening:** `And(vec![And(vec![a, b]), c])` normalises to `And(vec![a,b,c])`
at construction. Keeps generated SQL free of pointless nested parentheses, which
matters because readable SQL is an explicit product promise.

**`Value`** is a small owned enum (`Null | Bool | I32 | I64 | F64 | Decimal | Str
| Uuid | DateTime | Json | Bytes`) implementing `sqlx::Encode` for each backend.
We do not hold borrowed data in filters — queries are frequently built in one
scope and executed in another, and lifetimes on filters would poison every
signature in the API.

**Empty-filter semantics, decided explicitly:** `all([])` is `TRUE` and `any([])`
is `FALSE`. Documented loudly, because "delete where any([])" accidentally
becoming "delete everything" is the kind of bug that ends projects. `DELETE` and
`UPDATE` additionally *require* a non-empty filter unless `.all_rows()` is called
— see P4-04.

---

## P4-02 · SQL compilation

**Owner:** Vaibhav Gupta · **Est:** 5h · File: `crates/runtime/src/compile.rs`

```rust
pub struct SqlCompiler<'d> { dialect: &'d dyn DbDialect, sql: String, binds: Vec<Value> }

impl<'d> SqlCompiler<'d> {
    fn push_filter(&mut self, node: &FilterNode) { /* recursive, emits placeholders */ }
    pub fn finish(self) -> (String, Vec<Value>);
}
```

Every value goes through `push_bind` — there is no code path that interpolates a
runtime value into the SQL string. The `Raw` variant takes a format string with
`{}` holes filled by *placeholders*, not values, so even the escape hatch is
injection-safe by construction.

Placeholder numbering is dialect-driven (`$1..$n` vs `?`), so the compiler asks
`dialect.placeholder(i)` and never assumes.

**Every query exposes `.to_sql() -> (String, Vec<Value>)`** before execution. This
is a headline DX feature: no ORM should be a black box about the SQL it runs, and
it makes bug reports actionable.

---

## P4-03 · `SelectQuery`

**Owner:** Vaibhav Gupta · **Est:** 6h

```rust
pub struct SelectQuery<'db, M: Model, Out = M> {
    exec:     &'db dyn Executor,
    columns:  Selection,
    filter:   Option<FilterNode>,
    joins:    Vec<PlannedInclude>,
    order:    Vec<OrderBy<M>>,
    limit:    Option<u64>,
    offset:   Option<u64>,
    cursor:   Option<CursorSpec>,
    distinct: bool,
    lock:     Option<LockMode>,       // FOR UPDATE / SKIP LOCKED (pg only)
    _m: PhantomData<(M, Out)>,
}

impl<'db, M: Model> SelectQuery<'db, M> {
    pub fn filter(self, f: Filter<M>) -> Self;        // AND-accumulating
    pub fn or_filter(self, f: Filter<M>) -> Self;
    pub fn order_by(self, o: OrderBy<M>) -> Self;
    pub fn limit(self, n: u64) -> Self;
    pub fn offset(self, n: u64) -> Self;
    pub fn distinct(self) -> Self;
    pub fn for_update(self) -> Self;

    /// Projection changes the output type — this is where `Out` earns its keep.
    pub fn columns<C: Projection<M>>(self, c: C) -> SelectQuery<'db, M, C::Output>;

    pub async fn fetch_all(self)      -> Result<Vec<Out>>;
    pub async fn fetch_one(self)      -> Result<Out>;        // errors if 0 or >1
    pub async fn fetch_optional(self) -> Result<Option<Out>>;
    pub async fn count(self)          -> Result<i64>;
    pub async fn exists(self)         -> Result<bool>;
    pub fn stream(self)               -> impl Stream<Item = Result<Out>>;
    pub fn to_sql(&self)              -> (String, Vec<Value>);
}
```

`columns((user::ID, user::EMAIL))` returns `SelectQuery<'_, User, (Uuid, String)>`
via a `Projection` trait implemented for tuples up to 12 columns (macro-generated).
Projections are how you avoid `SELECT *` without losing types.

**`fetch_one` errors on zero rows.** Distinct from `fetch_optional`. Prisma's
`findUnique` returning null and `findUniqueOrThrow` throwing is a good split; we
express it in the return type rather than in two method names.

---

## P4-04 · Insert / Update / Delete / Upsert

**Owner:** Vaibhav Gupta · **Est:** 8h

```rust
// INSERT — the generated `UserInsert` makes required-vs-defaulted explicit.
let u: User = db.user().create(UserInsert {
    email: "a@b.c".into(),
    name:  Some("Ada".into()),
    ..Default::default()          // id, role, created_at have @default
}).exec().await?;

// Bulk — one statement, chunked to stay under parameter limits.
let n = db.user().create_many(vec![a, b, c]).exec().await?;

// UPDATE — partial by construction.
let updated = db.user()
    .update()
    .filter(user::ID.eq(id))
    .set(user::NAME.to("Grace"))
    .set_null(user::AVATAR)              // explicit null, distinct from "leave alone"
    .returning()
    .exec().await?;

// UPSERT
db.user().upsert()
    .on_conflict(user::EMAIL)
    .create(insert)
    .update(|u| u.set(user::NAME.to("Ada")))
    .exec().await?;

// DELETE — guarded
db.user().delete().filter(user::ID.eq(id)).exec().await?;
db.user().delete().all_rows().exec().await?;   // must be explicit
```

Details that matter:

- **`set_null` vs omitted.** `UserUpdate` uses `Option<Option<T>>` internally
  where the outer `None` means "unchanged" and `Some(None)` means "set NULL". The
  builder API hides this behind `.set()` / `.set_null()` because
  `Option<Option<T>>` in a public API is user-hostile.
- **Parameter limits.** Postgres caps a statement at 65535 parameters; SQLite's
  default `SQLITE_MAX_VARIABLE_NUMBER` is 32766 (999 on older builds). `create_many`
  chunks by `max_params / columns_per_row` and runs chunks inside one transaction.
  Silently exceeding this is a common ORM bug that only appears in production data
  volumes; we handle it from day one.
- **`@updatedAt`** is applied by the update builder in Rust, not by a DB trigger —
  keeps behaviour identical across dialects.
- **`RETURNING`** is used where `dialect.returning_supported()`; otherwise the
  builder does insert-then-select inside the ambient transaction.
- **Delete guard:** `DeleteQuery::exec` fails to compile without either `.filter()`
  or `.all_rows()`, enforced by a two-state typestate parameter. This is the one
  place typestate is worth its complexity cost.

---

## P4-05 · Executor, pool, transactions

**Owner:** Vaibhav Gupta · **Est:** 5h

```rust
/// Implemented by `Pool` and `Tx`, so every query works in either context.
#[async_trait]
pub trait Executor: Send + Sync {
    fn dialect(&self) -> &dyn DbDialect;
    async fn fetch_all_raw(&self, sql: &str, binds: Vec<Value>) -> Result<Vec<Row>>;
    async fn execute_raw(&self,   sql: &str, binds: Vec<Value>) -> Result<u64>;
}

db.transaction(|tx| Box::pin(async move {
    let u = tx.user().create(insert).exec().await?;
    tx.post().create(post_for(&u)).exec().await?;
    Ok(u)
})).await?;
```

- Commit on `Ok`, roll back on `Err` or panic. Nested calls use savepoints.
- Isolation level configurable per transaction (`.isolation(Serializable)`).
- **Retry helper** for serialization failures (`40001` on Postgres,
  `SQLITE_BUSY`): `db.transaction_retrying(3, |tx| ...)`. Worth including in v1
  because anyone using `Serializable` will need it immediately.

---

## P4-06 · Pagination

**Owner:** Vaibhav Gupta · **Est:** 3h

Both offset and cursor, because they solve different problems:

```rust
.paginate(Page::offset(2, 20))             // simple, correct for small/static data
.paginate(Cursor::after(last_id), 20)      // stable under concurrent inserts
```

Cursor pagination requires a total order; the builder errors at runtime with a
clear message if the ordering columns are not unique-suffixed, and automatically
appends the primary key to `ORDER BY` to guarantee determinism. Returns
`Page<T> { items, has_next, next_cursor }`.

---

## Phase P4 checklist

- [x] P4-01 filter algebra + flattening + documented empty semantics
- [x] P4-02 SQL compiler, 100% parameterised, `.to_sql()` everywhere
- [x] P4-03 `SelectQuery` incl. tuple projections, `exists`, and `stream` (tuple projections remain 1-8 columns against the planned 12)
- [x] P4-04 Insert/Update/Delete, upsert, chunking, delete guard (`InsertManyQuery` with driver parameter-limit chunking; upsert via `ON CONFLICT`; typestate delete guard)
- [x] P4-05 `Executor` trait, pool, transactions, retry helper, isolation levels
- [x] P4-06 offset + cursor pagination, plus `Page<T> { items, has_next, next_cursor }` with a deterministic primary-key ordering suffix
- [x] `both_dbs!` CRUD suite green on Postgres and SQLite — **both actually run**
- [x] `trybuild` tests for delete-without-filter and cross-model filters
- [x] **G4 signed off** — see the evidence below.

### G4 sign-off evidence (2026-08-10)

Verified against a live PostgreSQL 17.10 and SQLite: `cargo build --workspace`
clean, `cargo clippy --workspace --all-targets -- -D warnings` clean, and the
whole workspace green under `RUPRIZZLE_REQUIRE_DB=1` (37 suites), which makes an
unreachable backend a hard failure rather than a silent skip. See the G2 note in
ImplPlan03 for why that flag is mandatory in CI.

#### What P4-05 turned into

`Executor` is a trait implemented by both `Pool` and `Tx`, exposing `dialect`,
`fetch_all_raw`, `execute_raw`, and `stream_raw`. `SelectQuery` now holds
`&dyn Executor` instead of `&Pool`, so **the same query runs unchanged against a
pool or inside a transaction** — the actual point of the abstraction. Because
`&Pool` unsize-coerces to `&dyn Executor`, every existing call site in generated
code compiled without modification.

Three decisions worth recording:

- **The trait takes SQL by value (`String`).** The returned future outlives the
  call, so borrowing the query text would force every caller into a
  self-referential struct. One allocation per statement is irrelevant next to a
  round trip.
- **`Tx` moved to `tokio::sync::Mutex`.** The `std` guard is not `Send` across an
  await, which makes the trait's boxed futures un-`Send`. sqlx already pulls
  tokio in via `runtime-tokio`, so this adds nothing to the tree.
- **Retries are narrow on purpose.** `is_retryable` matches Postgres `40001` /
  `40P01` and SQLite lock contention only. Retrying a genuine constraint
  violation just repeats the work before failing the same way.

`Db::transaction_retrying(attempts, f)` and `Db::transaction_with(level, f)` are
emitted per schema. Isolation levels are applied on Postgres and accepted-and-
ignored on SQLite, which is effectively serializable already — the same
application code has to run on both.

#### Remaining known gaps

- **`stream` buffers rather than holding a cursor.** A `Tx` owns one connection
  behind a mutex, so an open cursor would block every other statement on that
  transaction; the `Pool` shares the same path so the two cannot drift. Rows are
  decoded lazily, but peak memory is not yet bounded. Swapping in a true
  incremental cursor is a `Pool`-only change behind `Executor::stream_raw` and
  needs no API change.
- **Write builders still take `&Pool`.** `InsertQuery::exec_nested` calls
  `pool.begin()` for its nested-create transaction, so genericising the write
  path needs savepoint support first. Reads are the common case for running
  inside an ambient transaction and are done.
- **`Page::next_cursor` is always `None`.** Offset paging and exact `has_next`
  work; emitting a typed cursor needs the primary-key *value* extracted from the
  last row, which the `Model` trait does not expose yet (it carries the column
  name, not an accessor).
- Tuple projections remain 1-8 columns against the planned 12.
