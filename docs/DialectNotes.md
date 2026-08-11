# Dialect notes

Postgres and SQLite share the same schema DSL but differ in what they can express
natively.

## Type mapping

| DSL type | Postgres | SQLite | Notes |
|---|---|---|---|
| `String` | `TEXT` | `TEXT` | |
| `Int` | `INTEGER` | `INTEGER` | |
| `BigInt` | `BIGINT` | `INTEGER` | SQLite only has one integer type. |
| `Float` | `DOUBLE PRECISION` | `REAL` | |
| `Decimal` | `NUMERIC` | `TEXT` | Avoid arithmetic on SQLite. |
| `Boolean` | `BOOLEAN` | `INTEGER` | `0`/`1`. |
| `DateTime` | `TIMESTAMPTZ` | `TEXT` | ISO-8601 in SQLite. |
| `Uuid` | `UUID` | `TEXT` | |
| `Json` | `JSONB` | `TEXT` | SQLite stores as text. |
| `Bytes` | `BYTEA` | `BLOB` | |

## Migrations

| Change | Postgres | SQLite |
|---|---|---|
| Add / drop column | `ALTER TABLE` | `ALTER TABLE` (3.35+) or table rebuild. |
| Rename column | `ALTER TABLE ... RENAME COLUMN` | Table rebuild. |
| Change type | `ALTER TABLE ... ALTER COLUMN ... TYPE` | Table rebuild. |
| Add / drop FK | `ALTER TABLE ... ADD/DROP CONSTRAINT` | Table rebuild (cannot add FK after create). |
| Add enum variant | `ALTER TYPE ... ADD VALUE` | Rebuild CHECK constraint on each using column. |

## Capabilities

```rust
let cap = SqliteDialect.capabilities();
assert!(!cap.native_enums);       // enums are emulated
assert!(!cap.native_uuid);        // stored as text
assert!(!cap.alter_column_type);  // type changes require rebuild
assert!(cap.drop_column);         // SQLite 3.35+
```

`ruprizzle validate` prints warnings when you use a construct that the active
provider handles poorly, e.g. `Decimal` on SQLite.
