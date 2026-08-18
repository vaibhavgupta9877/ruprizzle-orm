# Relations guide

`ruprizzle` generates one relation loader per relation. The loader is a value-level description of how to fetch the other side, and the actual loading is batched so the query count is bounded by the number of levels, not the number of parent rows.

All snippets below assume a generated `db` module. They are wrapped in `rust,ignore` because they depend on generated code.

## One-to-many and many-to-one

Given the blog schema, `User` has many `Post`s and each `Post` has one `author`:

```rust,ignore
let users = db.user()
    .find_many()
    .include(user::posts().take(5))
    .exec()
    .await?;

for user in &users {
    for post in user.posts.get() {
        println!("{}: {}", user.name, post.title);
    }
}

let posts = db.post()
    .find_many()
    .include(post::author())
    .exec()
    .await?;

for post in &posts {
    if let Some(author) = post.author.get() {
        println!("{} by {}", post.title, author.name);
    }
}
```

`include()` changes the execution method. Use `exec()`, `exec_one()`, or `exec_optional()` instead of `fetch_all()`/`fetch_one()`/`fetch_optional()` so the relation is loaded.

## Accessing loaded relations

Relation fields are `Related<T>`. Access the loaded value with `.get()`:

- `Related<Vec<Post>>` decodes to `&Vec<Post>`.
- `Related<Option<User>>` decodes to `&Option<User>`.

```rust,ignore
let user = db.user()
    .find_many()
    .filter(user::ID.eq(alice_id))
    .include(user::posts())
    .exec_one()
    .await?;

for post in user.posts.get() {
    println!("{}", post.title);
}
```

`try_get()` returns `Option<&T>` and is useful when you are not sure whether a relation was included. `is_absent()` and `is_loaded()` let you check the state without panicking.

## Filtering included children

Add filters, ordering, and a per-parent limit to the include:

```rust,ignore
let users = db.user()
    .find_many()
    .include(
        user::posts()
            .filter(post::PUBLISHED.eq(true))
            .order_by(post::CREATED_AT.desc())
            .take(10),
    )
    .exec()
    .await?;
```

`take(n)` is a per-parent limit, not a global limit. It is compiled to a `ROW_NUMBER() OVER (PARTITION BY ...)` window on dialects that support it.

## Single-row includes

`fetch_one()` and `fetch_optional()` are unavailable once `include()` is present. Use the include-aware equivalents:

```rust,ignore
let user = db.user()
    .find_many()
    .filter(user::ID.eq(alice_id))
    .include(user::posts().take(5))
    .exec_one()
    .await?;

let maybe = db.user()
    .find_many()
    .filter(user::EMAIL.eq("nobody@example.com"))
    .include(user::posts())
    .exec_optional()
    .await?;
```

## Many-to-many with an explicit join model

`ruprizzle` does not hide join tables. A `Post` with many `Tag`s through an explicit `PostTag` model looks like this:

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

The generated code provides an `IncludeMany` loader and a query helper:

```rust,ignore
let posts = db.post()
    .find_many()
    .include(post::tags())
    .exec()
    .await?;

for post in &posts {
    for tag in post.tags.get() {
        println!("{} has tag {}", post.id, tag.id);
    }
}

// Or load tags for a single post directly
let tags = post::tags_query(db.raw_pool(), post_id).fetch_all().await?;
```

## Nested writes on insert

Create a parent and its children in one call. The children must be supplied as an `InsertManyQuery`, and a `NestedSetter` attaches the loaded child rows to the parent.

```rust,ignore
use ruprizzle::{Encodable, InsertManyQuery, NestedSetter, Value};

struct SetPosts;
impl NestedSetter<User> for SetPosts {
    fn set(&self, parent: &mut User, batch: ruprizzle::executor::RowBatch) {
        parent.posts = ruprizzle::Related::Loaded(
            ruprizzle::executor::decode_rows::<Post>(batch).unwrap(),
        );
    }
}

let children = InsertManyQuery::<Post>::new(db.raw_pool())
    .row([("title", Value::Str("Hello".into()))])
    .row([("title", Value::Str("World".into()))]);

let user = db.user()
    .create(UserInsert {
        id: None,
        email: "alice@example.com".into(),
        name: Some("Alice".to_string()),
        role: None,
        created_at: None,
        updated_at: None,
    })
    .with_related(|u| u.id.to_value(), "author_id", children, SetPosts)
    .exec()
    .await?;

// Many-to-many on insert uses the generated *_set helper
let post = db.post()
    .create(PostInsert {
        id: None,
        title: "A post".into(),
        body: None,
        published: Some(true),
        author_id: user_id,
        created_at: None,
    })
    .with_m2m(post::tags_set(vec![rust_tag_id, orm_tag_id]))
    .exec()
    .await?;
```

## Nested writes on update

For one-to-many relations, use `connect`, `disconnect`, and `set_related`. The update must match exactly one parent row.

```rust,ignore
use ruprizzle::Encodable;

// Attach existing posts to a user
db.user()
    .update()
    .filter(user::ID.eq(user_id))
    .connect::<Post, _, _>(|u| u.id.to_value(), "author_id", "id", vec![post_id_1, post_id_2])
    .exec()
    .await?;

// Detach specific posts
db.user()
    .update()
    .filter(user::ID.eq(user_id))
    .disconnect::<Post, _, _>(|u| u.id.to_value(), "author_id", "id", vec![post_id_1])
    .exec()
    .await?;

// Replace the user's posts with a new set
db.user()
    .update()
    .filter(user::ID.eq(user_id))
    .set_related::<Post, _, _>(|u| u.id.to_value(), "author_id", "id", vec![post_id_3])
    .exec()
    .await?;
```

For many-to-many relations, the generated `*_attach`, `*_set`, and `*_detach` helpers return an `M2mWrite`:

```rust,ignore
db.post()
    .update()
    .set(post::TITLE, "Updated title")
    .filter(post::ID.eq(post_id))
    .with_m2m(post::tags_attach(vec![tag_id_1, tag_id_2]))
    .exec()
    .await?;

// Replace all tags for a post
db.post()
    .update()
    .filter(post::ID.eq(post_id))
    .with_m2m(post::tags_set(vec![tag_id_1]))
    .exec()
    .await?;

// Detach a tag
db.post()
    .update()
    .filter(post::ID.eq(post_id))
    .with_m2m(post::tags_detach(vec![tag_id_1]))
    .exec()
    .await?;
```

## Cascading deletes

`DeleteQuery::cascade` runs the referential action in a transaction before deleting the parent rows. The action must match the `onDelete` declared on the relation.

```rust,ignore
use ruprizzle::DeleteAction;

let deleted = db.user()
    .delete()
    .filter(user::ID.eq(user_id))
    .cascade::<Post>("author_id", DeleteAction::Cascade)
    .exec()
    .await?;
```

`DeleteAction` has the variants `Cascade`, `Restrict`, `SetNull`, `SetDefault`, and `NoAction`.

## Self-referential relations

A model that references itself, such as an `Employee` with a `manager_id`, can be loaded with aliased includes:

```rust,ignore
// Each employee has one manager
let employees = db.select::<Employee>()
    .include(employee::manager())
    .exec()
    .await?;

// Each manager has many reports
let managers = db.select::<Employee>()
    .include(employee::reports().take(10))
    .exec()
    .await?;
```

The generated module provides `manager()` and `reports()` relation loaders for self-referential schemas. Include depth is limited by the schema's `max_include_depth` generator setting.

## `some`, `every`, and `none` relation filters

For each relation, the generator emits `*_some`, `*_every`, and `*_none` functions that return a `Filter` on the parent model.

```rust,ignore
// Users with at least one published post
let authors = db.user()
    .find_many()
    .filter(user::posts_some(post::PUBLISHED.eq(true)))
    .fetch_all()
    .await?;

// Users whose every post is published
let all_published = db.user()
    .find_many()
    .filter(user::posts_every(post::PUBLISHED.eq(true)))
    .fetch_all()
    .await?;

// Users with no published posts
let no_posts = db.user()
    .find_many()
    .filter(user::posts_none(post::PUBLISHED.eq(true)))
    .fetch_all()
    .await?;
```

For a many-to-one relation, the generated helpers are named from the child side:

```rust,ignore
let posts = db.post()
    .find_many()
    .filter(post::author_some(user::NAME.eq("Alice")))
    .fetch_all()
    .await?;
```

## Why this avoids N+1

A naive loop would issue one query per parent to fetch children. `include` works in two batched stages:

1. Run the parent query.
2. Collect the parent keys, run one `IN (...)` or full-table query per child level, and distribute the child rows back into the parent structs in Rust.

The per-parent limit in `take(n)` is implemented with a `ROW_NUMBER() OVER (PARTITION BY ...)` window on dialects that support it. If the dialect does not support window functions, it falls back to one query per parent, which is the only case where the query count is linear in the parent count.

## Full-table fast path and its bound

When the parent query is an unfiltered full-table fetch and the include has no extra filter, order, or per-parent limit, `ruprizzle` can load the whole child table in one query instead of building a large `IN (...)` list. Before taking that path, it `COUNT(*)`s the child table and only uses the full-table load when the count is below `PoolConfig::full_table_include_limit` (default `100_000`). If the child table is larger, the loader falls back to a chunked `IN (...)` list, which preserves the bounded-query guarantee without materialising millions of rows. Set the field to `None` to disable the fast path entirely.
