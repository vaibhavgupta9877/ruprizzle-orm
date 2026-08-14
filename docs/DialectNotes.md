# Dialect notes

Postgres, MySQL/MariaDB, and SQLite share the same schema DSL but differ in
what they can express natively.

## Type mapping

| DSL type | Postgres | MySQL / MariaDB | SQLite | Notes |
|---|---|---|---|---|
| `String` | `TEXT` | `VARCHAR(255)` | `TEXT` | |
| `Int` | `INTEGER` | `INT` | `INTEGER` | |
| `BigInt` | `BIGINT` | `BIGINT` | `INTEGER` | SQLite only has one integer type. |
| `Float` | `DOUBLE PRECISION` | `DOUBLE` | `REAL` | |
| `Decimal` | `NUMERIC` | `DECIMAL(65,30)` | `TEXT` | Avoid arithmetic on SQLite. |
| `Boolean` | `BOOLEAN` | `TINYINT(1)` | `INTEGER` | MySQL and SQLite use `0`/`1`. |
| `DateTime` | `TIMESTAMPTZ` | `DATETIME(6)` | `TEXT` | ISO-8601 in SQLite. |
| `Date` | `DATE` | `DATE` | `TEXT` | |
| `Time` | `TIME` | `TIME` | `TEXT` | |
| `Uuid` | `UUID` | `CHAR(36)` | `TEXT` | |
| `Json` | `JSONB` | `JSON` | `TEXT` | SQLite stores as text. |
| `Bytes` | `BYTEA` | `BLOB` | `BLOB` | |

## Migrations

| Change | Postgres | MySQL / MariaDB | SQLite |
|---|---|---|---|
| Add / drop column | `ALTER TABLE` | `ALTER TABLE` | `ALTER TABLE` (3.35+) or table rebuild. |
| Rename column | `ALTER TABLE ... RENAME COLUMN` | `ALTER TABLE ... CHANGE COLUMN` | Table rebuild. |
| Change type | `ALTER TABLE ... ALTER COLUMN ... TYPE` | `ALTER TABLE ... MODIFY COLUMN` | Table rebuild. |
| Add / drop FK | `ALTER TABLE ... ADD/DROP CONSTRAINT` | `ALTER TABLE ... ADD/DROP FOREIGN KEY` | Table rebuild (cannot add FK after create). |
| Add enum variant | `ALTER TYPE ... ADD VALUE` | Rebuild the column check constraint. | Rebuild CHECK constraint on each using column. |

MySQL has no DML `RETURNING` clause. The runtime executes the insert and then
selects the inserted row by its primary key; auto-increment keys use
`LAST_INSERT_ID()`. Upserts use `ON DUPLICATE KEY UPDATE`. MariaDB-specific
`RETURNING` syntax is intentionally not emitted so the same schema remains
valid on both MySQL and MariaDB.

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
