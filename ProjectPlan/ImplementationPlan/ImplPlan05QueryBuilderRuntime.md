# ImplPlan 05 — Query Builder & Runtime (Phase P4)

**Duration:** 5 days · **Owners:** Claude (filter algebra, typestate), Devin (DML builders, tx)
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

**Owner:** Claude · **Est:** 6h · File: `crates/runtime/src/filter.rs`

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

**Owner:** Claude · **Est:** 5h · File: `crates/runtime/src/compile.rs`

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

**Owner:** Claude · **Est:** 6h

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

**Owner:** Devin · **Est:** 8h

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

**Owner:** Devin · **Est:** 5h

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

**Owner:** Devin · **Est:** 3h

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
- [x] P4-03 `SelectQuery` incl. tuple projections (1-8 column tuples supported; `stream` and `exists` not yet)
- [x] P4-04 Insert/Update/Delete, upsert, chunking, delete guard (`InsertManyQuery` with driver parameter-limit chunking; upsert via `ON CONFLICT`; typestate delete guard)
- [~] P4-05 `Executor` trait, pool, transactions, retry helper (`Tx` with raw execute/fetch; generated `Db::transaction` with commit/rollback). **Still absent, confirmed by grep over `crates/runtime/src`: the `Executor` trait itself, `transaction_retrying`, and per-transaction isolation levels.** Every query is therefore still bound to its concrete executor rather than generic over pool-vs-tx.
- [~] P4-06 offset + cursor pagination (`offset`/`limit`; `after`/`before` cursor methods). `Page<T> { items, has_next, next_cursor }` and the automatic primary-key suffix on `ORDER BY` are not yet implemented.
- [~] `both_dbs!` CRUD suite green on **SQLite**; Postgres skipped, not run
- [x] `trybuild` tests for delete-without-filter and cross-model filters
- [~] **G4 conditionally signed off** — see the qualification below.

Also still open from P4-03: `SelectQuery::stream` and `SelectQuery::exists` are
not implemented (`count` is). Tuple projections remain 1-8 columns against the
planned 12.

### G4 sign-off qualification (2026-08-10)

Verified: `cargo build --workspace` clean, `cargo clippy --workspace --all-targets
-- -D warnings` clean, runtime suites green (12 lib + 4 `crud.rs` + 3
`relations.rs` + 1 `trybuild.rs`), integration `crud.rs` 8 tests and `relations.rs`
4 tests reporting ok.

Those integration results are **SQLite-only**. Each suite finished in ~5.01s — the
Postgres connection timeout — and `both_dbs!` skips an unreachable backend rather
than failing it. Under `RUPRIZZLE_REQUIRE_DB=1` the Postgres halves panic with
`pool timed out`. Since G4's exit gate is explicitly "full CRUD round-trips
against live Postgres **and** SQLite," the Postgres half is unproven here.

Condition, same as G2 — one CI run against a live Postgres:

```
RUPRIZZLE_REQUIRE_DB=1 cargo test -p ruprizzle-integration-tests
```
