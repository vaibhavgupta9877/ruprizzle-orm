# Query guide

The generated client gives you two query styles: a type-safe builder that mirrors
SQL, and a model convenience wrapper. The model wrapper is the default starting
point for application code.

## Select

```rust
use my_app::db;

let users = db.user()
    .find_many()
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
    .find_many()
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
    .find_many()
    .columns(user::NAME)
    .fetch_all()
    .await?;
```

## Insert

Use the generated `UserInsert` shape to create one row:

```rust
let user = db.user()
    .create(db::UserInsert {
        id: None,
        email: "alice@example.com".into(),
        name: Some("Alice".into()),
    })
    .exec()
    .await?;
```

For a lower-level insert, `db.insert::<User>()` gives the same `InsertQuery` and
lets you call `.set(...)` / `.set_optional(...)` directly:

```rust
let user = db.insert::<User>()
    .set(user::EMAIL, "alice@example.com")
    .set_optional(user::NAME, Some("Alice"))
    .exec()
    .await?;
```

`create_many` is supported for bulk inserts:

```rust
let users = db.user()
    .create_many(vec![
        db::UserInsert { id: None, email: "a@example.com".into(), name: None },
        db::UserInsert { id: None, email: "b@example.com".into(), name: None },
    ])
    .exec()
    .await?;
```

## Update

```rust
let updated = db.user()
    .update()
    .set(user::NAME, "Alicia")
    .filter(user::EMAIL.eq("alice@example.com"))
    .exec()
    .await?;
```

## Delete

```rust
let deleted = db.user()
    .delete()
    .filter(user::EMAIL.eq("alice@example.com"))
    .exec()
    .await?;
```

## Pagination

`page(size)` fetches a `Page<Out>` with a cursor. `has_next` is exact because one
extra row is fetched and discarded.

```rust
let first_page = db.user()
    .find_many()
    .order_by(user::ID.asc())
    .page(20)
    .await?;

for user in &first_page.items {
    println!("{}", user.email);
}

if first_page.has_next {
    let next_cursor = first_page.next_cursor.unwrap();
    let next_page = db.user()
        .find_many()
        .order_by(user::ID.asc())
        .after(user::ID, next_cursor, 20)
        .await?;
}
```

## Transactions

```rust
use ruprizzle::prelude::*;

let mut tx = db.raw_pool().begin().await?;

// `&tx` implements `Executor`, so the raw builders work unchanged.
let user = InsertQuery::new(&tx)
    .set(user::EMAIL, "a@b.c")
    .exec()
    .await?;

if should_commit {
    tx.commit().await?;
} else {
    tx.rollback().await?;
}
```

## Raw SQL

If the builder cannot express a query, drop down to the executor:

```rust
use ruprizzle::prelude::*;

let rows = db
    .raw_pool()
    .fetch_all_raw(
        "SELECT * FROM users WHERE email LIKE ?".into(),
        vec![Value::Str("%alice%".into())],
    )
    .await?;
```
