# Schema reference

Every type, attribute, and function available in `schema.ruprizzle`.

## Blocks

### `datasource`

```prisma
datasource db {
  provider = "postgres" | "sqlite"
  url      = env("DATABASE_URL") | "literal://..."
  strict   = true
}
```

- `provider`: the database backend.
- `url`: either an environment variable name or a literal URL. Literal URLs are
  supported for quick tests but `env()` is recommended.
- `strict`: if `true`, the CLI and runtime reject unsupported constructs for the
  active provider (planned for 0.2).

### `generator`

```prisma
generator client {
  output        = "src/db"
  module_name   = "db"
  max_include_depth = 3
}
```

- `output`: directory where the Rust client is emitted.
- `module_name`: the name your crate uses to `mod` the output.
- `max_include_depth`: how many levels of nested `include` are allowed.

## Scalars

| Type | Postgres | SQLite | Notes |
|---|---|---|---|
| `String` | `TEXT` | `TEXT` | |
| `Int` | `INTEGER` | `INTEGER` | 32-bit signed. |
| `BigInt` | `BIGINT` | `INTEGER` | SQLite stores in 64-bit `INTEGER`. |
| `Float` | `DOUBLE PRECISION` | `REAL` | |
| `Decimal` | `NUMERIC` | `TEXT` | SQLite stores as text; avoid arithmetic. |
| `Boolean` | `BOOLEAN` | `INTEGER` | SQLite uses `0`/`1`. |
| `DateTime` | `TIMESTAMPTZ` | `TEXT` | ISO-8601 in SQLite. |
| `Date` | `DATE` | `TEXT` | |
| `Time` | `TIME` | `TEXT` | |
| `Uuid` | `UUID` | `TEXT` | SQLite stores as text. |
| `Json` | `JSONB` | `TEXT` | SQLite stores as text; query limitations apply. |
| `Bytes` | `BYTEA` | `BLOB` | |

## Field attributes

- `@id` — primary key. Auto-detected for `Int`/`BigInt`/`Uuid` with `@default`.
- `@default(<expr>)` — default value. Supports `autoincrement()`, `uuid7()`,
  `now()`, and literal values.
- `@unique` — single-column unique constraint.
- `@map("column_name")` — override the physical column name.
- `@relation(...)` — define the owner side of a relation.
- `@ignore` — omit from the generated client.

## Model-level attributes

- `@@map("table_name")` — override the physical table name.
- `@@unique([a, b])` — composite unique constraint.
- `@@index([a, b])` — index on the listed fields.
- `@@id([a, b])` — composite primary key.

## Enums

```prisma
enum Role {
  USER
  ADMIN
}
```

Postgres uses a native `CREATE TYPE`. SQLite emulates enums with a `CHECK`
constraint on `TEXT`.

## Relations

Only the owner side of a relation carries `@relation`:

```prisma
model Post {
  authorId Uuid @map("author_id")
  author   User @relation(fields: [authorId], references: [id], onDelete: Cascade)
}

model User {
  posts Post[]
}
```

Supported referential actions: `Cascade`, `Restrict`, `SetNull`, `SetDefault`,
`NoAction`.
