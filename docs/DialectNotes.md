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

## Query builder

The runtime query builder compiles to SQL per dialect. If a construct cannot be
expressed on the target database, `to_sql()` returns `Error::Message(...)`
instead of emitting unsupported SQL:

| Construct | Postgres | MySQL / MariaDB | SQLite |
|---|---|---|---|
| `INNER JOIN` | yes | yes | yes |
| `LEFT JOIN` | yes | yes | yes |
| `RIGHT JOIN` | yes | yes | no |
| `FULL OUTER JOIN` | yes | no | no |
| `UNION` / `UNION ALL` | yes | yes | yes |
| `INTERSECT` | yes | no | yes |
| `EXCEPT` | yes | no | yes |

MySQL does not support `INTERSECT` or `EXCEPT`; MariaDB does not support
`INTERSECT` or `EXCEPT` either. SQLite supports both but does not support
`RIGHT JOIN` or `FULL OUTER JOIN`.

## v1.1–v1.5 features per dialect

The feature line shipped in `1.5.1` lands differently on each backend:

| Feature | Postgres | MySQL / MariaDB | SQLite |
|---|---|---|---|
| Array columns + filter operators | Native `T[]` | `JSON` + `JSON_CONTAINS` / `JSON_OVERLAPS` | `TEXT` (JSON) |
| Full-text search | Native `tsvector` / `tsquery`, `@@fulltext` + generated column | `MATCH ... AGAINST` | FTS5 virtual table |
| Soft deletes | `@softDelete` field + transparent `WHERE` filter | same | same |
| PostGIS / geospatial | `geo-types` mapping, spatial indexes | no | no |
| Implicit many-to-many | join table emitted by `migrate dev` | same | same |
| Read-replica routing / query cache / OTel | runtime-level, dialect-agnostic | same | same |
| Serverless adapters | — | — | `ruprizzle-turso` (Hrana over HTTP), `ruprizzle-d1` (Cloudflare REST) |

Turso and D1 are SQLite-family dialects reached over HTTP: one request per
statement, no interactive transactions — see `docs/KnownLimitations.md` for the
full caveat list.

## MySQL-specific notes

- **No `RETURNING` clause.** Inserts and upserts rely on a primary-key follow-up
  query or `LAST_INSERT_ID()` for auto-increment keys.
- **No native `ENUM` types.** Enumerations are enforced with `CHECK` constraints.
- **No `FULL OUTER JOIN` or `RIGHT JOIN` on older versions.** Prefer `LEFT JOIN`
  or emulate with `UNION` where necessary.
- **`String[]` is stored as `JSON`**. Array containment and overlap are implemented
  with `JSON_CONTAINS` and `JSON_OVERLAPS`.
- **`Uuid` is stored as `CHAR(36)`**; `Decimal` uses `DECIMAL(65,30)` (or a
  narrower `DECIMAL(19,4)` when declared with `@db.Decimal(p,s)`).
- **Upserts use `ON DUPLICATE KEY UPDATE`**, keyed on the primary key and unique
  constraints.
