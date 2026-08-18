# Quickstart

From an empty directory to a working query in under five minutes.

## Prerequisites

- Rust 1.85 or later.
- A running PostgreSQL, MySQL/MariaDB, or SQLite 3 database. SQLite needs no
  server; just a writable file path.

This guide uses PostgreSQL. To use SQLite, replace `--provider postgres` with
`--provider sqlite` and set `DATABASE_URL` to a file path such as
`sqlite://./dev.db`.

## 1. Install the CLI

```bash
cargo install ruprizzle-cli
```

## 2. Scaffold a project

```bash
mkdir my-app && cd my-app
ruprizzle init --provider postgres
```

This creates:

```text
my-app/
  schema.ruprizzle
  .env
  .gitignore
  migrations/
    README.md
  src/
    main.rs
```

Open `.env` and update `DATABASE_URL`:

```bash
DATABASE_URL="postgres://user:password@localhost:5432/my_app_db?sslmode=disable"
```

## 3. Edit the schema

Replace `schema.ruprizzle` with:

```prisma
datasource db {
  provider = "postgres"
  url      = env("DATABASE_URL")
}

generator client {
  output      = "src/db"
  module_name = "db"
}

model User {
  id    Int    @id @default(autoincrement())
  email String @unique
  name  String
}
```

## 4. Create and run the first migration

```bash
ruprizzle migrate dev --name init
```

This diffs the empty database against the schema, writes a migration under
`migrations/`, applies it, and regenerates the client.

## 5. Add dependencies

```bash
cargo add ruprizzle tokio --features tokio/full
```

Or edit `Cargo.toml`:

```toml
[dependencies]
ruprizzle = "1.0.0-rc.1"
tokio = { version = "1", features = ["full"] }
```

## 6. Write the first query

Make `src/main.rs`:

```rust
mod db;

#[tokio::main]
async fn main() -> Result<(), ruprizzle::Error> {
    let db = db::Db::connect(&std::env::var("DATABASE_URL")?).await?;

    let alice = db
        .user()
        .create(db::UserInsert {
            id: None,
            email: "alice@example.com".into(),
            name: Some("Alice".into()),
        })
        .exec()
        .await?;

    let users = db
        .user()
        .find_many()
        .filter(db::user::EMAIL.ends_with("@example.com"))
        .order_by(db::user::NAME.asc())
        .fetch_all()
        .await?;

    println!("created: {:?}", alice);
    println!("users: {:?}", users);
    Ok(())
}
```

Run it:

```bash
cargo run
```

## 7. Iterate

Change `schema.ruprizzle` and run:

```bash
ruprizzle migrate dev --name add_field
ruprizzle generate
```

Or, for live code generation while you edit:

```bash
ruprizzle generate --watch
```

## Common first errors

- `Failed to acquire connection`: `DATABASE_URL` is wrong or the database is not
  running.
- `table users already exists`: the database already has a `users` table from a
  previous prototype. Use `ruprizzle migrate reset --force` in development to
  drop and re-apply, or delete `migrations/` and start fresh.
- `no column named ...`: the generated client is stale. Run `ruprizzle generate`.

## Next steps

- [Schema reference](SchemaReference.md)
- [Query guide](QueryGuide.md)
- [Migrations guide](MigrationsGuide.md)
- [Relations guide](RelationsGuide.md)
