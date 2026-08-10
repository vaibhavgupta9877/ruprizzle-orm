# Query guide

The generated client gives you two query styles: a type-safe builder that mirrors
SQL, and a model convenience wrapper.

## Select

```rust
use my_app::db;

let users = db.user()
    .select()
    .filter(user::EMAIL.eq("alice@example.com"))
    .order_by(user::NAME.asc())
    .limit(10)
    .offset(20)
    .fetch_all()
    .await?;
```

Call `.to_sql()` on any builder to see the generated SQL before running it:

```rust
let sql = db.user()
    .select()
    .filter(user::EMAIL.eq("alice@example.com"))
    .to_sql();
println!("{sql}");
```

## Filters

- Equality: `user::EMAIL.eq(...)`
- Inequality: `user::EMAIL.not_eq(...)`
- Ordering: `user::AGE.gt(18)`, `.gte(18)`, `.lt(18)`, `.lte(18)`
- String: `user::EMAIL.starts_with("alice@")`, `.ends_with("@example.com")`, `.contains("acme")`
- Null: `user::PHONE.is_null()`, `.is_not_null()`
- Combinators: `.and(filter)`, `.or(filter)`, `all([...])`, `any([...])`

## Projections

Select only the columns you need:

```rust
let names = db.user()
    .select()
    .project(user::NAME)
    .fetch_all()
    .await?;
```

## Insert

```rust
db.user()
    .insert()
    .set(user::EMAIL, "alice@example.com")
    .set(user::NAME, "Alice")
    .exec()
    .await?;
```

`insert_many` is supported for bulk inserts.

## Update

```rust
db.user()
    .update()
    .set(user::NAME, "Alicia")
    .filter(user::EMAIL.eq("alice@example.com"))
    .exec()
    .await?;
```

## Delete

```rust
db.user()
    .delete()
    .filter(user::EMAIL.eq("alice@example.com"))
    .exec()
    .await?;
```

## Pagination

```rust
use ruprizzle::Page;

let page = db.user()
    .select()
    .paginate(Page::new(1, 20))   // page 1, 20 per page
    .fetch()
    .await?;

println!("page {} of {}, total {}", page.number, page.total, page.total_rows);
```

## Transactions

```rust
let mut tx = db.begin().await?;

// tx implements Executor, so all builders work unchanged.
let id = tx.user().insert().set(user::EMAIL, "a@b.c").exec().await?;

if should_commit {
    tx.commit().await?;
} else {
    tx.rollback().await?;
}
```

## Raw SQL

If the builder cannot express a query, drop down to the executor:

```rust
let rows = db.fetch_all_raw(
    "SELECT * FROM users WHERE email LIKE $1".to_owned(),
    vec![Value::from("%@example.com")],
).await?;
```

## Observability

Add `tracing-subscriber` with its `fmt` and `env-filter` features, then install a
subscriber in the application to see database activity:

```toml
tracing-subscriber = { version = "0.3", features = ["env-filter", "fmt"] }
```

```rust
tracing_subscriber::fmt()
    .with_env_filter("ruprizzle::query=debug,ruprizzle::migrate=info")
    .init();
```

`ruprizzle::query` reports SQL text, bind count, result counts, elapsed
milliseconds, and a non-sensitive error category on failure. Bind values and
database error detail are not logged. `ruprizzle::migrate` reports migration
start and completion events with the migration ID and elapsed time. Avoid embedding
sensitive literals in raw SQL because raw SQL text is intentionally observable.
