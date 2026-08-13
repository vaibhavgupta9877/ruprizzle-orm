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
    .fetch_all()
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
    .fetch_all()
    .await?;
```

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
