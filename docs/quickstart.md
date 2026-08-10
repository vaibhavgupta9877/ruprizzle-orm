# Quickstart

From an empty directory to a working query in under five minutes.

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

```
my-app/
  schema.ruprizzle
  .env
  .gitignore
  migrations/
    README.md
```

Open `.env` and update `DATABASE_URL` if your local Postgres is not on the
default host:

```bash
DATABASE_URL="postgres://localhost:5432/postgres?sslmode=disable"
```

## 3. Edit the schema

`schema.ruprizzle` is the single source of truth. A starter `User` model is
already there, commented out. Replace it with:

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

This diffs the (empty) database against the schema, writes a migration under
`migrations/`, applies it, and regenerates the client.

## 5. Add the generated module

`ruprizzle generate` writes to `src/db/`. Add it to `src/main.rs`:

```rust
mod db;

use ruprizzle::prelude::*;
```

## 6. Run your first query

```rust
mod db;

#[tokio::main]
async fn main() -> Result<(), ruprizzle::Error> {
    let db = db::Db::connect(&std::env::var("DATABASE_URL")?).await?;

    db.user()
        .create(db::UserInsert {
            id: None,
            email: "alice@example.com".into(),
            name: "Alice".into(),
        })
        .exec()
        .await?;

    let users = db
        .user()
        .find_many()
        .filter(db::user::EMAIL.ends_with("@example.com"))
        .fetch_all()
        .await?;

    println!("{users:?}");
    Ok(())
}
```

## 7. Iterate

Change `schema.ruprizzle` and run:

```bash
ruprizzle migrate dev --name add_field
```

Or, for live code generation while you edit:

```bash
ruprizzle generate --watch
```

## Next steps

- [Schema reference](schema-reference.md)
- [Query guide](query-guide.md)
- [Migrations guide](migrations-guide.md)
