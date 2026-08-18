# Schema reference

Every type, attribute, and function available in `schema.ruprizzle`. See
[Dialect notes](DialectNotes.md) for backend-specific mappings and limitations.

## Top-level blocks

### `datasource`

```prisma
datasource db {
  provider = "postgres" | "mysql" | "sqlite"
  url      = env("DATABASE_URL") | "literal://..."
  strict   = true
}
```

- `provider`: the database backend. Supported: `postgres`, `mysql`, `sqlite`.
- `url`: an environment variable name or a literal URL. `env()` is recommended;
  literal URLs are useful for quick tests.
- `strict`: when `true`, the CLI and runtime reject constructs that the active
  provider cannot express well.

### `generator`

```prisma
generator client {
  output            = "src/db"
  module_name       = "db"
  max_include_depth = 3
}
```

- `output`: directory where the Rust client is emitted.
- `module_name`: the name your crate uses to `mod` the output.
- `max_include_depth`: the deepest level of nested `include` the codegen will
  generate.

## Scalars

| Type | Postgres | MySQL / MariaDB | SQLite | Notes |
|---|---|---|---|---|
| `String` | `TEXT` | `VARCHAR(255)` | `TEXT` | |
| `Int` | `INTEGER` | `INT` | `INTEGER` | 32-bit signed. |
| `BigInt` | `BIGINT` | `BIGINT` | `INTEGER` | SQLite stores in 64-bit `INTEGER`. |
| `Float` | `DOUBLE PRECISION` | `DOUBLE` | `REAL` | |
| `Decimal` | `NUMERIC` | `DECIMAL(65,30)` | `TEXT` | Avoid arithmetic on SQLite. |
| `Boolean` | `BOOLEAN` | `TINYINT(1)` | `INTEGER` | MySQL and SQLite use `0`/`1`. |
| `DateTime` | `TIMESTAMPTZ` | `DATETIME(6)` | `TEXT` | ISO-8601 in SQLite. |
| `Date` | `DATE` | `DATE` | `TEXT` | |
| `Time` | `TIME` | `TIME` | `TEXT` | |
| `Uuid` | `UUID` | `CHAR(36)` | `TEXT` | |
| `Json` | `JSONB` | `JSON` | `TEXT` | SQLite stores as text; JSON1 filters work. |
| `Bytes` | `BYTEA` | `BLOB` | `BLOB` | |

## Native type annotations

Use `@db.<native>` to override the default physical type for the active dialect.

| Annotation | Postgres | MySQL / MariaDB | SQLite |
|---|---|---|---|
| `@db.Uuid` | `UUID` | `CHAR(36)` | `TEXT` |
| `@db.VarChar(n)` | `VARCHAR(n)` | `VARCHAR(n)` | `TEXT` |
| `@db.Text` | `TEXT` | `TEXT` | `TEXT` |
| `@db.Integer` | `INTEGER` | `INT` | `INTEGER` |
| `@db.SmallInt` | `SMALLINT` | `SMALLINT` | `INTEGER` |
| `@db.BigInt` | `BIGINT` | `BIGINT` | `INTEGER` |
| `@db.Serial` | `SERIAL` | `AUTO_INCREMENT` | `INTEGER` |
| `@db.BigSerial` | `BIGSERIAL` | `AUTO_INCREMENT` | `INTEGER` |
| `@db.Real` | `REAL` | `FLOAT` | `REAL` |
| `@db.Double` | `DOUBLE PRECISION` | `DOUBLE` | `REAL` |
| `@db.Decimal(p,s)` | `NUMERIC(p,s)` | `DECIMAL(p,s)` | `TEXT` |
| `@db.Boolean` | `BOOLEAN` | `TINYINT(1)` | `INTEGER` |
| `@db.Timestamp` | `TIMESTAMP` | `DATETIME(6)` | `TEXT` |
| `@db.Timestamptz` | `TIMESTAMPTZ` | `DATETIME(6)` | `TEXT` |
| `@db.Date` | `DATE` | `DATE` | `TEXT` |
| `@db.Time` | `TIME` | `TIME` | `TEXT` |
| `@db.Json` | `JSONB` | `JSON` | `TEXT` |
| `@db.Jsonb` | `JSONB` | `JSON` | `TEXT` |
| `@db.Bytes` | `BYTEA` | `BLOB` | `BLOB` |
| `@db.Generated("...")` | generated column | generated column | stored as text |

## Field attributes

- `@id` — primary key. Auto-detected for `Int`/`BigInt`/`Uuid` with `@default`.
- `@default(<expr>)` — default value. See the expressions below.
- `@unique` — single-column unique constraint.
- `@map("column_name")` — override the physical column name.
- `@relation(...)` — define the owner side of a relation.
- `@ignore` — omit from the generated client.
- `@db.<native>` — override the native type.
- `@updatedAt` — automatically set to `now()` on every update (Postgres/MySQL).

## Default expressions

| Expression | Returns | Example |
|---|---|---|
| `autoincrement()` | integer | `id Int @id @default(autoincrement())` |
| `uuid7()` | `Uuid` | `id Uuid @id @default(uuid7())` |
| `now()` | `DateTime` | `createdAt DateTime @default(now())` |
| `dbgenerated("...")` | dialect-specific | `token String @default(dbgenerated("gen_random_uuid()"))` |
| literal value | the literal | `role Role @default(USER)` |

## Model-level attributes

- `@@map("table_name")` — override the physical table name.
- `@@unique([a, b, ...])` — composite unique constraint.
- `@@index([a, b, ...])` — index on the listed fields.
- `@@id([a, b, ...])` — composite primary key.
- `@@ignore` — omit the model from the generated client.

## Index attributes

### Basic index

```prisma
@@index([email])
```

### Named index

```prisma
@@index([email, createdAt], name: "user_email_created_idx")
```

### Index type

```prisma
@@index([email], type: "Hash")   // Postgres only
```

### Expression index

```prisma
@@index([lower(email)])
```

Expression indexes are supported on Postgres and SQLite 3.31+ where the
expression can be emitted directly.

### Partial index

```prisma
@@index([email], where: "verified = true")
```

Partial indexes are supported on Postgres and SQLite.

## Generated columns

A generated column is computed from other columns or expressions:

```prisma
model User {
  id       Int    @id @default(autoincrement())
  first    String
  last     String
  fullName String @db.Generated("first | ' ' | last")
}
```

On SQLite, generated columns are stored as text or as the underlying native
representation, depending on the dialect.

## Enums

```prisma
enum Role {
  USER
  ADMIN
}
```

Postgres uses a native `CREATE TYPE`. MySQL and SQLite emulate enums with a
`CHECK` constraint on `TEXT`.

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

### `@relation` arguments

- `fields` — the foreign-key column(s) on this model.
- `references` — the target column(s) on the related model.
- `onDelete` / `onUpdate` — referential action.
- `name` — optional relation name, used for disambiguation.
- `map` — override the physical foreign-key constraint name.

### Referential actions

Supported: `Cascade`, `Restrict`, `SetNull`, `SetDefault`, `NoAction`.

### Many-to-many

Many-to-many relations are explicit join models:

```prisma
model Tag {
  id   Uuid    @id @default(uuid7())
  name String  @unique
  posts PostTag[]
}

model PostTag {
  id      Uuid  @id @default(uuid7())
  postId  Uuid  @map("post_id")
  tagId   Uuid  @map("tag_id")
  post    Post  @relation(fields: [postId], references: [id], onDelete: Cascade)
  tag     Tag   @relation(fields: [tagId], references: [id], onDelete: Cascade)

  @@unique([postId, tagId])
}
```

## PostgreSQL extensions

```prisma
datasource db {
  provider   = "postgres"
  url        = env("DATABASE_URL")
  extensions = ["uuid-ossp", "pgcrypto"]
}
```

`uuid-ossp` is required for `uuid7()`-style defaults unless another extension
provides equivalent functionality.

## Naming

- Model and field identifiers use `camelCase` by convention.
- `@@map` and `@map` are used to produce `snake_case` physical names.
- Physical names are limited by the target database (Postgres: 63 bytes; MySQL:
  64 characters; SQLite: no hard limit).

## Example schema

```prisma
datasource db {
  provider = "postgres"
  url      = env("DATABASE_URL")
  extensions = ["uuid-ossp"]
}

generator client {
  output      = "src/db"
  module_name = "db"
  max_include_depth = 3
}

model User {
  id        Uuid     @id @default(uuid7())
  email     String   @unique @db.VarChar(255)
  name      String?
  role      Role     @default(USER)
  metadata  Json?    @db.Jsonb
  createdAt DateTime @default(now()) @map("created_at") @db.Timestamptz
  posts     Post[]

  @@index([email, createdAt])
  @@map("users")
}

model Post {
  id        Uuid     @id @default(uuid7())
  title     String
  published Boolean  @default(false)
  authorId  Uuid     @map("author_id")
  author    User     @relation(fields: [authorId], references: [id], onDelete: Cascade)

  @@index([authorId], where: "published = true")
  @@map("posts")
}

enum Role {
  USER
  ADMIN
}
```
