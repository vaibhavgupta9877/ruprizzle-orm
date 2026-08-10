# ImplPlan 06 — Relations & Nested Include (Phase P5)

**Duration:** 4 days · **Owners:** Claude (loader algorithm), Devin (codegen + tests)
**Exit gate G5:** two-level nested include returns correct data in a bounded number
of queries, proven by a query-count assertion test.

---

## Why this phase is the differentiator

Relation loading is where ORMs are actually judged. SeaORM makes you write
`find_with_related` chains by hand. sqlx makes you write the join and the
de-duplication yourself. Prisma's `include` is the feature people actually miss
when they move to Rust. Getting this right is the strongest reason for
ruprizzle-orm to exist — and it is also the phase most likely to overrun, so it
gets its own gate.

---

## P5-01 · Relation IR resolution

**Owner:** Claude · **Est:** 4h

By the end of ImplPlan02's pass 3, each relation is resolved into a canonical form
shared by both sides:

```rust
pub struct ResolvedRelation {
    pub name:        String,          // explicit @relation("author") or derived
    pub kind:        RelationKind,    // OneToOne | OneToMany | ManyToOne
    pub owner:       ModelName,       // the side holding the FK
    pub owner_cols:  Vec<String>,     // FK columns (composite supported)
    pub target:      ModelName,
    pub target_cols: Vec<String>,     // referenced columns, usually the PK
    pub on_delete:   ReferentialAction,  // Cascade|Restrict|SetNull|NoAction
    pub on_update:   ReferentialAction,
    pub optional:    bool,
    /// Field name on each side, for codegen.
    pub owner_field:  FieldName,
    pub target_field: Option<FieldName>,   // None if the back-relation is omitted
}
```

**Canonicalisation rule:** both `User.posts` and `Post.author` point at the *same*
`ResolvedRelation` instance (by index). This guarantees the two sides can never
disagree about the FK columns or the delete behaviour — a class of bug that
plagues hand-maintained mappings.

**Many-to-many in v1:** not implicit. You declare the join model:

```prisma
model PostTag {
  postId Uuid @map("post_id")
  tagId  Uuid @map("tag_id")
  post   Post @relation(fields: [postId], references: [id], onDelete: Cascade)
  tag    Tag  @relation(fields: [tagId],  references: [id], onDelete: Cascade)
  @@id([postId, tagId])
  @@map("post_tags")
}
```

Two `ManyToOne` relations, no magic. Prisma's implicit `_PostToTag` table is
convenient right up until you need a column on the join, at which point it becomes
a migration you cannot express. Explicit join models are the honest default; sugar
for traversing them can arrive in 0.2 without a breaking change.

---

## P5-02 · Include API & codegen

**Owner:** Devin · **Est:** 6h

Generated per relation:

```rust
// in `pub mod user`
pub fn posts() -> IncludeBuilder<super::post::Post> { IncludeBuilder::new("posts") }

// in `pub mod post`
pub fn author() -> IncludeBuilder<super::user::User> { IncludeBuilder::new("author") }
```

Usage, including nesting and per-relation constraints:

```rust
let users = db.user()
    .find_many()
    .filter(user::ROLE.eq(Role::Admin))
    .include(
        user::posts()
            .filter(post::PUBLISHED.eq(true))
            .order_by(post::CREATED_AT.desc())
            .take(5)
            .include(post::comments().take(3))     // depth 2
    )
    .exec().await?;

for u in &users {
    for p in u.posts.get() {                       // Related::get
        println!("{} — {} comments", p.title, p.comments.get().len());
    }
}
```

Per-relation `filter`/`order_by`/`take` is the thing Prisma users reach for
constantly and Drizzle handles awkwardly. It is worth the implementation cost.

**Depth limit: 3 in v1**, enforced at build time with a clear error. Unbounded
nesting invites accidental combinatorial explosions; the limit is a guardrail, not
a technical constraint, and is configurable in `generator` config.

---

## P5-03 · The loading strategy — batched, not joined

**Owner:** Claude · **Est:** 8h

Two viable strategies. We choose **batched sequential loading**, and the reasoning
should survive review:

| | Single JOIN | Batched (chosen) |
|---|---|---|
| Queries | 1 | 1 per level (not per row) |
| Row explosion | parent × children — a parent with 100 posts × 10 comments returns 1000 rows repeating parent columns | none |
| Per-relation `take` | needs window functions; hard on SQLite | trivial `LIMIT` per batch |
| Per-relation `filter` | pushes into `LEFT JOIN ON`, easy to get wrong | trivial `WHERE` |
| De-duplication | required, fiddly, error-prone | none |
| Correctness on `take` at parent level | `LIMIT` interacts badly with joined rows | unaffected |

Prisma made the same call for the same reasons. The "N+1" objection does not apply:
this is **1 query per relation level**, independent of row count.

Algorithm:

```
load(level_0):
    rows_0 = SELECT ... FROM parent WHERE <filter> ORDER BY ... LIMIT ...
    for each requested include at this level:
        keys = distinct(rows_0.map(fk_or_pk))          # parent-side join keys
        if keys.is_empty(): mark Related::Loaded(empty); continue
        children = SELECT ... FROM child
                   WHERE child.<target_col> IN (:keys)  # chunked to param limit
                     AND <relation filter>
                   ORDER BY <relation order>
        if take.is_some():
            # per-parent LIMIT requires a window function
            children = ROW_NUMBER() OVER (PARTITION BY fk ORDER BY ...) <= take
        index = group_by(children, fk)
        attach index back onto rows_0 via HashMap<Key, Vec<Child>>
        recurse into this relation's own includes with `children` as the new parent set
```

Implementation notes:

- **`take` needs a window function.** Both Postgres and modern SQLite (3.25+)
  support `ROW_NUMBER() OVER (PARTITION BY ...)`. The dialect exposes
  `capabilities().window_functions`; if absent, fall back to per-parent queries and
  emit a runtime warning once. Add `window_functions` to `Capabilities` in P2.
- **Key chunking** reuses the parameter-limit logic from P4-04. A parent set of
  50 000 rows must not produce a 50 000-parameter `IN`.
- **Attachment is O(n)** via `HashMap<Key, Vec<Child>>`, built once per level. Do
  not do a nested scan — that is the accidental O(n²) that makes ORMs look slow.
- **Order preservation:** children keep the relation's `ORDER BY`; parents keep the
  outer one. The `HashMap` grouping must push in result order, so use a
  `HashMap<Key, Vec<_>>` fed by an ordered iteration, not a sort afterwards.
- Composite keys are supported by making the join key a tuple; the `IN` becomes a
  row-value comparison on Postgres and an `OR` chain on SQLite.

---

## P5-04 · Relation filters on the parent

**Owner:** Claude · **Est:** 4h

Filtering parents *by* their children — Prisma's `some`/`every`/`none`:

```rust
db.user().find_many()
    .filter(user::posts_some(post::PUBLISHED.eq(true)))
    .filter(user::posts_none(post::FLAGGED.eq(true)))
    .exec().await?;
```

Compiles to correlated subqueries, which is the only formulation that stays correct
under all three quantifiers:

```sql
-- some
EXISTS     (SELECT 1 FROM posts p WHERE p.author_id = users.id AND p.published)
-- none
NOT EXISTS (SELECT 1 FROM posts p WHERE p.author_id = users.id AND p.flagged)
-- every  (note the double negation — the subtle one)
NOT EXISTS (SELECT 1 FROM posts p WHERE p.author_id = users.id AND NOT (p.published))
```

`every` returning `true` for parents with **no** children is the standard vacuous-truth
semantics and matches Prisma. Document it explicitly; it surprises people.

Generated helper names: `user::posts_some(f)`, `posts_every(f)`, `posts_none(f)`,
each returning `Filter<User>` so they compose with everything in P4-01.

---

## P5-05 · Nested writes

**Owner:** Devin · **Est:** 5h

Scoped tightly for v1 — nested create only, one level:

```rust
db.user().create(UserInsert { email: "a@b.c".into(), ..Default::default() })
    .with_posts(vec![PostInsert { title: "Hello".into(), ..Default::default() }])
    .exec().await?;
```

Runs in an implicit transaction: insert parent, take generated PK, insert children
with the FK populated, return the parent with `Related::Loaded(children)`.

Nested update, nested upsert, `connect`/`disconnect`, and `set` are **deferred to
0.2**. They are a large surface with subtle semantics, and shipping half of them is
worse than shipping none. Recorded in ImplPlan10.

---

## P5-06 · Query-count regression tests

**Owner:** Devin · **Est:** 3h

The guarantee only holds if it is tested. A counting executor wraps the real one:

```rust
#[test] async fn include_is_bounded() {
    let db = TestDb::postgres(BLOG).await;
    seed_users(&db, 100).await;              // each with 20 posts, each 5 comments
    let counter = db.counting();

    let users = counter.user().find_many()
        .include(user::posts().include(post::comments()))
        .exec().await?;

    assert_eq!(counter.query_count(), 3);    // users, posts, comments — NOT 1 + 100 + 2000
    assert_eq!(users.len(), 100);
    assert_eq!(users[0].posts.get().len(), 20);
}
```

This test is the single most valuable regression guard in the project. Any future
refactor that reintroduces N+1 fails here loudly.

---

## Phase P5 checklist

- [x] P5-01 `ResolvedRelation` canonicalised, both sides share one instance
- [x] P5-02 `include` codegen, nesting to depth 3, per-relation filter/order/take
- [x] P5-03 batched loader, chunked keys, O(n) attachment, window-function `take`
- [x] P5-04 `some`/`every`/`none` with correct vacuous-truth semantics
- [x] P5-05 one-level nested create in a transaction (runtime `with_related` + `NestedSetter`; generated `.with_posts()` helpers not emitted — see gaps)
- [x] P5-06 query-count assertions green on both dialects
- [ ] Composite-key relations covered by tests — **not implemented**, see gaps
- [x] **G5 signed off by Claude** — see the evidence below.

### G5 sign-off evidence (2026-08-10)

Verified against a live PostgreSQL 17.10 and SQLite. `cargo build --workspace`
clean, `cargo clippy --workspace --all-targets` clean, and the whole workspace
green under `RUPRIZZLE_REQUIRE_DB=1`, which turns an unreachable backend into a
hard failure rather than a silent skip.

The gate asks for *"two-level nested include returns correct data in a bounded
number of queries, proven by a query-count assertion test."* That test now
exists, on both dialects:
`tests/integration/tests/relations.rs::include_is_bounded` seeds 10 users × 5
posts × 3 comments, loads `users → posts → comments`, and asserts
`counter.count() == 3` — one query per *level*, against the 61 an N+1 loader
would issue. A second assertion in the same test loads 50 posts with their
author in **2** queries, which is the many-to-one direction.

The counter is `CountingExecutor` in the runtime crate (not the test harness):
it wraps any `&dyn Executor` and counts statements. Keeping it in the library
means an application profiling its own hot path can use the same tool.

#### Three defects the sign-off work uncovered

Writing the gate test found bugs that the previous "verified by inspection"
status had missed. Each is fixed, and each is now covered:

- **`take` was a batch `LIMIT`, not a per-parent one.** `posts().take(5)` over
  100 users returned 5 posts *total*. It now compiles to
  `ROW_NUMBER() OVER (PARTITION BY <fk> ORDER BY ...)` filtered to `<= n`, via
  `compile::select_partitioned`, so it stays one query per level.
  `per_relation_take_is_per_parent` asserts each parent gets exactly its `take`,
  in the relation's own order.
- **Many-to-one includes attached the child to one parent only.** The grouping
  map used `remove`, so the second post sharing an author got `None`. The
  many-to-one path now keeps one child per key and clones per parent, which is
  what a many-to-one relation means.
- **Parent keys were not de-duplicated.** Every many-to-one include sent one
  bind per parent row instead of one per distinct key, burning the parameter
  budget and forcing needless chunking.

`Capabilities::window_functions` was added as P5-03 specified. Both shipped
dialects set it true (SQLite has had windows since 3.25); the loader keeps an
honest per-parent fallback for a dialect that does not, and that fallback is the
only path that is not bounded.

#### Remaining known gaps

- **Composite-key relations are not supported end to end.** `Encodable` has no
  tuple implementation and the codegen takes `owner_cols.first()`, so a relation
  over more than one column silently uses only its first. The row-value `IN` on
  Postgres and `OR` chain on SQLite that P5-01 describes are unwritten. Nothing
  in `examples/` uses one, so no generated code is currently wrong — but the
  checklist item is not done and is not signed off.
- **Generated `.with_posts()` nested-create helpers are not emitted.** The
  runtime mechanism (`with_related` + `NestedSetter`) works and is tested; the
  codegen sugar over it is missing, so nested create is currently a hand-written
  call.
- **`stream`/`page` + `include` do not compose.** Both are implemented only for
  `SelectQuery<_, _, ()>`, so the type system rejects the combination rather
  than loading it wrongly. Batching needs the whole parent set up front, which
  is inherently at odds with streaming; `page` could support includes by loading
  them after truncation, and is the one worth revisiting.
