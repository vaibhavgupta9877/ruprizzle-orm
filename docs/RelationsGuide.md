# Relations guide

`ruprizzle` generates a `find_many` helper per model that supports nested
`include` loading. The goal is one query per level, never N+1.

## One-to-many

```prisma
model User {
  id    Int    @id @default(autoincrement())
  posts Post[]
}

model Post {
  id       Int    @id @default(autoincrement())
  authorId Int    @map("author_id")
  author   User   @relation(fields: [authorId], references: [id])
}
```

```rust
let users = db.user()
    .find_many()
    .include(user::posts().take(5))
    .exec()
    .await?;

for user in &users {
    for post in &user.posts {
        println!("{}: {}", user.name, post.title);
    }
}
```

The generated SQL is a single `SELECT` with a `LEFT JOIN` for the first level.
Deeper levels produce additional queries, one per distinct parent set.

## Filtering included children

```rust
db.user()
    .find_many()
    .include(
        user::posts()
            .filter(post::PUBLISHED.eq(true))
            .order_by(post::CREATED_AT.desc())
            .take(10)
    )
    .exec()
    .await?;
```

## Single-row includes

`fetch_one()` and `fetch_optional()` are only available on plain selects. Once
you add `.include(...)`, use the include-aware single-row methods instead:

```rust
let user = db.user()
    .find_many()
    .filter(user::ID.eq(1))
    .include(user::posts().take(5))
    .exec_one()
    .await?;

for post in user.posts.get() {
    println!("{}", post.title);
}
```

`exec_optional()` returns `Option<M>` and also loads the requested includes.
`exec()` remains the choice when you expect many rows.

## Many-to-many with an explicit join model

`ruprizzle` does not hide join tables. Model them explicitly for full control:

```prisma
model Post {
  id    Int    @id @default(autoincrement())
  tags  Tag[]  @relation("PostTag")
}

model Tag {
  id    Int    @id @default(autoincrement())
  posts Post[] @relation("PostTag")
}

model PostTag {
  postId Int @map("post_id")
  tagId  Int @map("tag_id")
  post   Post @relation(fields: [postId], references: [id])
  tag    Tag  @relation(fields: [tagId], references: [id])

  @@id([postId, tagId])
}
```

## Nested some / every / none

Filter parents by a condition on their children:

```rust
db.user()
    .find_many()
    .filter(user::posts().some(post::PUBLISHED.eq(true)))
    .fetch_all()
    .await?;
```

`some`, `every`, and `none` are translated to `EXISTS` / `NOT EXISTS` subqueries.

## Why this avoids N+1

A naive loop would issue one query per parent to fetch children. `include`
batches children by parent key and issues one query per *level*, then maps the
rows back into the parent structs in Rust.

## Full-table fast path and its bound

When the parent query is an unfiltered full-table fetch and the include has no
extra filter, order, or per-parent limit, `ruprizzle` can load the whole child
table in one query instead of building a large `IN (...)` list. Before taking
that path, it `COUNT(*)`s the child table and only uses the full-table load when
the count is below `PoolConfig::full_table_include_limit` (default `100_000`).
If the child table is larger, the loader falls back to a chunked `IN (...)` list,
which preserves the bounded-query guarantee without materialising millions of
rows. Set the field to `None` to disable the fast path entirely.
