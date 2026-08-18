# Query guide

The generated client gives you two ways to build a query. The **model repository** (`db.user()`, `db.post()`) is the Prisma-flavoured starting point for application code. The **raw builders** (`db.select::<User>()`, `db.insert::<User>()`) are the Drizzle-flavoured escape hatch when you want full control.

All snippets below assume a generated `db` module from the `examples/blog/schema.ruprizzle` and are wrapped in `rust,ignore` because they depend on generated code.

## Select and fetch helpers

`find_many()` starts a `SELECT` for a model. Add filters, ordering, pagination, and then call a fetch method.

```rust,ignore
let users: Vec<User> = db.user()
    .find_many()
    .filter(user::EMAIL.eq("alice@example.com"))
    .order_by(user::NAME.asc())
    .limit(10)
    .offset(20)
    .fetch_all()
    .await?;
```

For a single row without `include()`:

```rust,ignore
let user: User = db.user()
    .find_many()
    .filter(user::EMAIL.eq("alice@example.com"))
    .fetch_one()
    .await?;

let maybe: Option<User> = db.user()
    .find_many()
    .filter(user::EMAIL.eq("alice@example.com"))
    .fetch_optional()
    .await?;
```

The generated repo does not expose a separate `find_by_id` or `find_unique`. Use `find_many().filter(user::ID.eq(id))` and one of the fetch helpers.

## Include-aware execution

Once you add `.include(...)`, the fetch methods are disabled and the include-aware methods take over. This is enforced at compile time so you cannot accidentally return rows with unloaded relations.

```rust,ignore
let user = db.user()
    .find_many()
    .filter(user::ID.eq(alice_id))
    .include(user::posts().take(5))
    .exec_one()
    .await?;

for post in user.posts.get() {
    println!("{}", post.title);
}
```

`exec()` returns `Vec<M>`, `exec_one()` returns exactly one row, and `exec_optional()` returns `Option<M>`.

## Filters

Column tokens are typed, so values and other columns are checked at compile time.

```rust,ignore
user::EMAIL.eq("alice@example.com")                       // equality
user::EMAIL.ne("alice@example.com")                       // inequality
user::CREATED_AT.gt(start) | user::CREATED_AT.gte(start)  // ordering
user::CREATED_AT.lt(end) | user::CREATED_AT.lte(end)
user::CREATED_AT.between(start, end)                      // BETWEEN
user::ROLE.in_set(vec![Role::User, Role::Admin])          // IN
user::ROLE.not_in_set(vec![Role::User])                   // NOT IN
user::NAME.is_null()                                      // IS NULL
user::NAME.is_not_null()                                  // IS NOT NULL
user::EMAIL.starts_with("alice@")                         // string matchers
user::EMAIL.ends_with("@example.com")
user::EMAIL.contains("acme")
user::EMAIL.ilike("alice")                                // ILIKE on Postgres, LIKE on SQLite
```

Combine filters with `and`, `or`, `all`, and `any`:

```rust,ignore
use ruprizzle::{all, any};

let f = all([
    user::CREATED_AT.gte(week_ago),
    user::EMAIL.ends_with("@example.com"),
]);

let f = any([
    user::ROLE.eq(Role::Admin),
    user::EMAIL.starts_with("admin@"),
]);
```

Filter parents by a condition on their children with the generated relation filters:

```rust,ignore
user::posts_some(post::PUBLISHED.eq(true))    // at least one child matches
user::posts_every(post::PUBLISHED.eq(true))   // every child matches (vacuously true if none)
user::posts_none(post::PUBLISHED.eq(true))    // no child matches
```

For the low-level correlated `EXISTS` shape, use `Filter::exists` and `Column::correlated_to`:

```rust,ignore
use ruprizzle::Filter;

let sub = db.select::<Post>()
    .columns(post::ID)
    .filter(post::AUTHOR_ID.correlated_to(user::ID));

let authors = db.user()
    .find_many()
    .filter(Filter::exists(sub))
    .fetch_all()
    .await?;
```

## Conditional / dynamic building

Every builder has `*_if` variants that skip the call when the value is `None`, so you can assemble queries from optional inputs without changing the builder type.

```rust,ignore
let mut q = db.user().find_many();

if let Some(email) = maybe_email {
    q = q.filter(user::EMAIL.eq(email));
}

let users = q
    .filter_if(maybe_after.map(|t| user::CREATED_AT.gte(t)))
    .order_by_if(maybe_sort.map(|_| user::NAME.asc()))
    .limit_if(maybe_limit)
    .offset_if(maybe_offset)
    .fetch_all()
    .await?;
```

Inserts, updates, and deletes have the same conditional helpers:

```rust,ignore
let user = db.user()
    .create(UserInsert {
        id: None,
        email: "alice@example.com".into(),
        name: None,
        role: None,
        created_at: None,
        updated_at: None,
    })
    .set_if(user::NAME, Some("Alice".to_string()))
    .exec()
    .await?;

let affected = db.user()
    .update()
    .set_if(user::NAME, Some("Alicia".to_string()))
    .set_if(user::ROLE, None::<Role>)        // skipped
    .filter_if(Some(user::EMAIL.eq("alice@example.com")))
    .exec()
    .await?;

let deleted = db.user()
    .delete()
    .filter_if(Some(user::EMAIL.eq("alice@example.com")))
    .exec()
    .await?;
```

For `delete`, passing `None` to `filter_if` makes the query unfiltered. You can also call `.all_rows()` explicitly when you mean to touch every row.

## Projections

`columns(...)` restricts the `SELECT` list and changes the output type. A single column decodes to a one-tuple; multiple columns decode to a tuple.

```rust,ignore
let names: Vec<(String,)> = db.user()
    .find_many()
    .columns(user::NAME)
    .fetch_all()
    .await?;

let rows: Vec<(String, String)> = db.user()
    .find_many()
    .columns((user::NAME, user::EMAIL))
    .fetch_all()
    .await?;
```

Other projection helpers are available on any `SelectQuery`:

```rust,ignore
let count: i64 = db.user().find_many().filter(user::ROLE.eq(Role::Admin)).count().await?;
let any: bool = db.user().find_many().filter(user::EMAIL.starts_with("admin@")).exists().await?;

let emails: Vec<(String,)> = db.user()
    .find_many()
    .columns(user::EMAIL)
    .distinct()
    .fetch_all()
    .await?;
```

`count()` and `exists()` ignore `ORDER BY`, `LIMIT`, and `OFFSET`.

## Aggregates and grouping

Aggregates are built from column tokens and carry their return type at the type level.

```rust,ignore
let rows: Vec<(Option<f64>, i64)> = db.select::<Employee>()
    .group_by(employee::ROLE)
    .aggregate((employee::SALARY.sum(), employee::ID.count()))
    .having(employee::ROLE.eq("Manager"))
    .order_by(employee::ROLE.asc())
    .fetch_all()
    .await?;
```

Available aggregate methods are `sum`, `avg`, `min`, `max`, `count`, and `count_distinct`. `group_by` can take a single column or a tuple. `having` accepts the same `Filter<M>` as `filter`.

The code generator also emits a model aggregate struct and input. For `User`, the generated types are `UserAggregate` and `UserAggregateInput`:

```rust,ignore
let rows: Vec<UserAggregate> = db.user()
    .find_many()
    .group_by(user::ROLE)
    .aggregate(UserAggregateInput {
        count_id: Some(user::ID.count()),
        count_distinct_email: Some(user::EMAIL.count_distinct()),
        ..Default::default()
    })
    .fetch_all()
    .await?;
```

## Ordering, pagination, and cursors

```rust,ignore
let first = db.user()
    .find_many()
    .order_by(user::CREATED_AT.desc())
    .page(20)
    .await?;

for user in &first.items {
    println!("{}", user.email);
}

if first.has_next {
    let last_id = first.items.last().unwrap().id;
    let next = db.user()
        .find_many()
        .order_by(user::ID.asc())
        .after(user::ID, last_id, 20)
        .await?;
}
```

`page(size)` fetches `size + 1` rows and discards the extra one, so `has_next` is exact. `after(column, value, size)` and `before(column, value, size)` are convenience helpers that add `> value` or `< value`, order by the column, and limit the result.

For offset-based pagination, use `limit` and `offset` directly:

```rust,ignore
let rows = db.user()
    .find_many()
    .order_by(user::NAME.asc())
    .limit(10)
    .offset(40)
    .fetch_all()
    .await?;
```

## Insert and upsert

The generated `create` helper accepts the model's `Insert` struct:

```rust,ignore
let user = db.user()
    .create(UserInsert {
        id: None,
        email: "alice@example.com".into(),
        name: Some("Alice".to_string()),
        role: Some(Role::User),
        created_at: None,
        updated_at: None,
    })
    .exec()
    .await?;
```

For multi-row inserts, use `create_many`:

```rust,ignore
let users = db.user()
    .create_many(vec![
        UserInsert { id: None, email: "a@example.com".into(), name: None, role: None, created_at: None, updated_at: None },
        UserInsert { id: None, email: "b@example.com".into(), name: None, role: None, created_at: None, updated_at: None },
    ])
    .exec()
    .await?;
```

The Drizzle-style entry point is `db.insert::<M>()`:

```rust,ignore
let user = db.insert::<User>()
    .set(user::EMAIL, "alice@example.com")
    .set_optional(user::NAME, Some("Alice".to_string()))
    .set_optional(user::ROLE, Some(Role::User))
    .exec()
    .await?;
```

`set_optional` (and its alias `set_if`) sets a column only when the value is `Some`.

Upserts are `on_conflict` plus `do_update`:

```rust,ignore
db.insert::<User>()
    .set(user::EMAIL, "alice@example.com")
    .set(user::NAME, "Alice")
    .on_conflict(["email"])
    .do_update(["name"])
    .exec()
    .await?;
```

Nested writes on insert use `InsertQuery::with_related` for one-to-many and `with_m2m` for many-to-many:

```rust,ignore
use ruprizzle::{Encodable, InsertManyQuery, NestedSetter, Value};

struct SetPosts;
impl NestedSetter<User> for SetPosts {
    fn set(&self, parent: &mut User, batch: ruprizzle::executor::RowBatch) {
        parent.posts = ruprizzle::Related::Loaded(ruprizzle::executor::decode_rows::<Post>(batch).unwrap());
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
```

For a many-to-many relation, the generated helpers return an `M2mWrite` that can be passed to `with_m2m`:

```rust,ignore
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

## Update and delete

```rust,ignore
let affected = db.user()
    .update()
    .set(user::NAME, "Alicia")
    .set_null(user::NAME)                        // set a nullable column to NULL
    .filter(user::EMAIL.eq("alice@example.com"))
    .exec()
    .await?;
```

Delete requires a filter or an explicit `.all_rows()` call:

```rust,ignore
let deleted = db.user()
    .delete()
    .filter(user::EMAIL.eq("alice@example.com"))
    .exec()
    .await?;

// Delete every row
let deleted = db.user().delete().all_rows().exec().await?;
```

Nested one-to-many writes on update use `connect`, `disconnect`, and `set_related`:

```rust,ignore
use ruprizzle::Encodable;

// Connect existing posts to this user
let affected = db.user()
    .update()
    .filter(user::ID.eq(user_id))
    .connect::<Post, _, _>(|u| u.id.to_value(), "author_id", "id", vec![post_id])
    .exec()
    .await?;

// Disconnect specific posts
let affected = db.user()
    .update()
    .filter(user::ID.eq(user_id))
    .disconnect::<Post, _, _>(|u| u.id.to_value(), "author_id", "id", vec![post_id])
    .exec()
    .await?;

// Replace the user's posts with one post
let affected = db.user()
    .update()
    .filter(user::ID.eq(user_id))
    .set_related::<Post, _, _>(|u| u.id.to_value(), "author_id", "id", vec![post_id])
    .exec()
    .await?;
```

`connect` and `set_related` on `UpdateQuery` require the query to match exactly one parent row.

Many-to-many updates use the generated `*_attach`, `*_set`, and `*_detach` helpers:

```rust,ignore
let post = db.post()
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
```

Cascading deletes use `DeleteQuery::cascade` and a `DeleteAction` that matches the schema's `onDelete`:

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

## Relations and `include`

Relations are loaded with a single query per level. Basic one-to-many and many-to-one includes look like this:

```rust,ignore
let users = db.user()
    .find_many()
    .include(user::posts().take(5))
    .exec()
    .await?;

let posts = db.post()
    .find_many()
    .include(post::author())
    .exec()
    .await?;
```

The returned relations are `Related<Vec<Post>>` and `Related<Option<User>>`. Access them with `.get()` after using the include-aware `exec`/`exec_one`/`exec_optional` methods.

Filter, order, and limit the included children:

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

Nested includes are supported:

```rust,ignore
let users = db.user()
    .find_many()
    .include(
        user::posts()
            .take(5)
            .include(post::author())
    )
    .exec()
    .await?;
```

## Explicit joins

Explicit joins produce `Join2` or `LeftJoin2` result tuples.

```rust,ignore
let rows: Vec<Join2<User, Post>> = db.select::<User>()
    .inner_join::<Post>(user::ID.on(post::AUTHOR_ID))
    .fetch_all()
    .await?;

let rows: Vec<LeftJoin2<User, Post>> = db.select::<User>()
    .left_join::<Post>(user::ID.on(post::AUTHOR_ID))
    .fetch_all()
    .await?;
```

`LeftJoin2` wraps the right side in `Maybe<T>`, which dereferences to `Option<T>`. Right and full joins are also available, but SQLite does not support them.

Self-joins use an alias and `Column::aliased`:

```rust,ignore
let rows: Vec<Join2<Employee, Employee>> = db.select::<Employee>()
    .inner_join_aliased::<Employee>("mgr", employee::MANAGER_ID.on(employee::ID.aliased("mgr")))
    .fetch_all()
    .await?;
```

## Subqueries and CTEs

`in_subquery` and `not_in_subquery` accept a `SelectQuery` whose projection matches the column type:

```rust,ignore
let sub = db.select::<Post>()
    .columns(post::AUTHOR_ID)
    .filter(post::PUBLISHED.eq(true))
    .distinct();

let authors = db.user()
    .find_many()
    .filter(user::ID.in_subquery(sub))
    .fetch_all()
    .await?;
```

Correlated subqueries use `correlated_to` and `Filter::exists`/`Filter::not_exists`:

```rust,ignore
use ruprizzle::Filter;

let sub = db.select::<Post>()
    .columns(post::ID)
    .filter(post::AUTHOR_ID.correlated_to(user::ID));

let authors = db.user()
    .find_many()
    .filter(Filter::exists(sub))
    .fetch_all()
    .await?;
```

Non-recursive CTEs are added with `with`:

```rust,ignore
use ruprizzle::Filter;

let managers = db.select::<Employee>()
    .filter(employee::ROLE.eq("manager"))
    .columns((employee::ID, employee::NAME));

let rows = db.select::<Employee>()
    .with("managers", managers)
    .filter(Filter::exists(
        db.select::<Manager>()
            .filter(manager::ID.correlated_to(employee::ID)),
    ))
    .fetch_all()
    .await?;
```

Recursive CTEs use `with_recursive`:

```rust,ignore
let anchor = db.select::<Employee>().filter(employee::ID.eq(2));

let recursive = db.select::<Employee>()
    .filter(Filter::exists(
        db.select::<Reports>()
            .filter(reports::MANAGER_ID.correlated_to(employee::ID)),
    ));

let rows = db.select::<Reports>()
    .with_recursive("reports", anchor, recursive)
    .fetch_all()
    .await?;
```

## Set operations

`SelectQuery` supports `union`, `union_all`, `intersect`, and `except`. Both sides must have the same output type.

```rust,ignore
let admins = db.user()
    .find_many()
    .filter(user::ROLE.eq(Role::Admin))
    .columns(user::EMAIL);

let users = db.user()
    .find_many()
    .filter(user::AGE.gte(18))
    .columns(user::EMAIL);

let emails: Vec<(String,)> = admins.union(users).fetch_all().await?;
```

`union_all` preserves duplicates. `intersect` and `except` are not supported by MySQL.

## JSON operators

JSON columns expose path extraction and containment filters. The path can be chained with `.get(...)`, `.get_text(...)`, and `.at(...)`.

```rust,ignore
// Requires a model with a serde_json::Value column, e.g. Item::META
let rows = db.select::<Item>()
    .filter(item::META.get("status").eq("active"))
    .filter(item::META.get("nested").get("score").gt(10))
    .filter(item::META.at(0).get_text("name").eq("first"))
    .filter(item::META.has_key("tags"))
    .filter(item::META.contains(serde_json::json!({ "flag": true })))
    .order_by(item::META.get("priority").desc())
    .fetch_all()
    .await?;
```

In updates, `json_set` and `jsonb_set` modify a single key:

```rust,ignore
UpdateQuery::<Item>::new(db.raw_pool())
    .set(item::TITLE, "New title")
    .jsonb_set(item::META, "status", serde_json::json!("archived"))
    .filter(item::ID.eq(item_id))
    .exec()
    .await?;
```

## Array operators

Array columns (or JSON-array columns on MySQL/SQLite) support `contains`, `contained_by`, and `overlaps`.

```rust,ignore
// Requires a model with a Vec<String> column, e.g. Article::TAGS
let rows = db.select::<Article>()
    .filter(article::TAGS.contains(["rust"]))
    .filter(article::TAGS.contained_by(["rust", "orm", "sql"]))
    .filter(article::TAGS.overlaps(["orm", "sql"]))
    .fetch_all()
    .await?;
```

## Prepared statements

Prepare a query once and rebind the placeholders for each execution. Bind positions are zero-based.

```rust,ignore
let mut prep = db.user()
    .find_many()
    .filter(user::EMAIL.eq(""))
    .prepare()?;

prep.bind(0, "alice@example.com".to_string());
let alice = prep.fetch_one().await?;

prep.bind(0, "bob@example.com".to_string());
let bob = prep.fetch_one().await?;
```

`bind_many` replaces all binds at once with a `Vec<Value>`.

## Raw SQL escape hatch

The `raw!` macro builds an injection-safe `RawFragment`. Values are bound as parameters; they are never interpolated into the SQL string.

```rust,ignore
use ruprizzle::{raw, Filter};

let fragment = raw!("email LIKE {}", "%@example.com");
let rows = db.user()
    .find_many()
    .filter(Filter::raw(fragment))
    .fetch_all()
    .await?;
```

For a statement that does not fit the typed builders, run it directly through the pool:

```rust,ignore
use ruprizzle::raw;

let fragment = raw!("UPDATE posts SET published = {} WHERE author_id = {}", true, author_id);
db.raw_pool()
    .execute_raw(fragment.sql().into(), fragment.binds().to_vec())
    .await?;
```

You can also embed a raw fragment inside a typed `Filter`:

```rust,ignore
use ruprizzle::Filter;

let rows = db.user()
    .find_many()
    .filter(Filter::raw(raw!("created_at > {}", since)))
    .fetch_all()
    .await?;
```

## SQL transparency

Call `.to_sql()` on any builder to inspect the generated SQL and bind values before execution.

```rust,ignore
let sql = db.user()
    .find_many()
    .filter(user::EMAIL.eq("alice@example.com"))
    .to_sql()?;

println!("{}", sql.sql);
```

`to_sql()` returns a `CompiledSql` with `sql` and `binds` fields. `SelectQuery`, `UpdateQuery`, `DeleteQuery`, and `SetOpQuery` return `Result<CompiledSql, _>`; `InsertQuery` and `AggregateQuery` return `CompiledSql` directly.
