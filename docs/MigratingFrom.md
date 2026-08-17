# Migrating from other Rust ORMs

## From Diesel

| Diesel | ruprizzle |
|---|---|
| `table!` macro | `schema.ruprizzle` + `ruprizzle generate` |
| `Selectable`, `Queryable` | generated `Model` structs |
| `QueryDsl` | `select()`, `filter()`, `order_by()`, `limit()` builders |
| Migrations by hand | `ruprizzle migrate dev` from schema diff |

### Quick translation

```rust
// Diesel
users::table.filter(users::email.eq("alice@example.com")).first::<User>(conn)?

// ruprizzle
db.user()
    .find_many()
    .filter(user::EMAIL.eq("alice@example.com"))
    .fetch_one()
    .await?
```

## From SeaORM

| SeaORM | ruprizzle |
|---|---|
| `ActiveModel` | generated `Set` builders |
| `Entity` | `schema.ruprizzle` model |
| `Relation` trait | `include()` with generated relation helpers |

### Quick translation

```rust
// SeaORM
User::find_by_id(1).one(db).await?

// ruprizzle
db.user()
    .find_many()
    .filter(user::ID.eq(1))
    .fetch_one()
    .await?
```

## From sqlx

| sqlx | ruprizzle |
|---|---|
| Raw SQL with macros | `db.raw_pool().fetch_all_raw` for escape hatches, generated builders for common cases |
| Manual `FromRow` | generated `FromRow` impls per model |
| No migrations | `ruprizzle migrate` |

### Quick translation

```rust
// sqlx
sqlx::query_as::<_, User>("SELECT * FROM users WHERE email = $1")
    .bind("alice@example.com")
    .fetch_one(&pool).await?

// ruprizzle
db.user()
    .find_many()
    .filter(user::EMAIL.eq("alice@example.com"))
    .fetch_one()
    .await?
```
