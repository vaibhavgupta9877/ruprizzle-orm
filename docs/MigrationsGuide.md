# Migrations guide

`ruprizzle` generates migrations by diffing your current `schema.ruprizzle`
against the state of the migration history. You edit the schema, run a command,
and the tool writes, applies, and tracks the SQL for you.

## The two commands

| Command | When to use | What it does |
|---|---|---|
| `ruprizzle migrate dev` | Local development | Diff schema → write migration files → apply → regenerate client. |
| `ruprizzle migrate deploy` | CI / production | Apply pending `migrations/*.sql` files only. |

These are deliberately separate. `deploy` never writes migration files, prompts,
or diffs the schema, so a habit of running `migrate deploy` cannot accidentally
drop a production column.

## Development workflow

```bash
# 1. Edit schema.ruprizzle
# 2. Create, apply, and regenerate the client
ruprizzle migrate dev --name add_profile

# If the migration would lose data and you accept it:
ruprizzle migrate dev --name drop_legacy --accept-data-loss

# Write the migration without applying it:
ruprizzle migrate dev --name draft --create-only
```

`migrate dev` is the only command that diffs, writes, and applies. Every other
command is read-only or apply-only.

## Backfills

Adding a `NOT NULL` column without a default requires a three-step migration:

1. Add the column as nullable.
2. Fill existing rows (`RUPRIZZLE:BACKFILL` block in the generated `up.sql`).
3. Alter the column to `NOT NULL`.

Edit the generated `up.sql` and replace the placeholder expression before
applying. The `RUPRIZZLE:BACKFILL` block is preserved when you re-run
`migrate dev`.

## Drift and `migrate status`

`ruprizzle migrate status` introspects the live database and compares it to the
migration history. If someone hand-edited the database, it reports the
differences.

```bash
ruprizzle migrate status
```

Resolve drift by either:

- writing a new migration that brings the schema back into sync, or
- `ruprizzle migrate reset --force` in development to drop everything and replay.

## Prototyping: `db push`

```bash
ruprizzle db push
```

This diffs and applies directly with no migration file. It is intended for
prototyping only and prints a warning every time. Do not use it in production.

## Introspection: `db pull`

`db pull` reads an existing database and generates a `schema.ruprizzle` from it.
Use it to adopt an existing database or to inspect a schema that was not created
by ruprizzle.

```bash
ruprizzle db pull
```

The command will:

- read the live database from `DATABASE_URL`,
- generate a `schema.ruprizzle` that describes the current tables, columns,
  indexes, and foreign keys,
- back up the previous `schema.ruprizzle` before overwriting.

Review the generated schema before committing it. Names and types may need
manual cleanup, especially for columns that do not map cleanly to ruprizzle
scalar types.

## Seeding: `db seed`

`db seed` loads fixture data from a JSON file and upserts it by primary key.

1. Create `seeds/main.json` next to `schema.ruprizzle`:

```json
{
  "User": [
    { "id": 1, "email": "alice@example.com", "name": "Alice" },
    { "id": 2, "email": "bob@example.com", "name": "Bob" }
  ]
}
```

2. Run:

```bash
ruprizzle db seed
```

Seed rows are upserted by primary key in a single transaction and the client is
regenerated so you can query them immediately.

## `migrate squash`

Squashing collapses the existing migration history into a baseline and archives
the old `up.sql` / `down.sql` files.

```bash
ruprizzle migrate squash --force
```

Requirements:

- the migration history must be fully applied,
- all checksums must be valid,
- the database must be in the exact state described by the final migration.

This rewrites history; pass `--force` to confirm. Old migrations are archived
under `migrations/.archive/` and a new baseline migration is created.

## `migrate resolve`

If a migration was partially applied or failed outside ruprizzle, you can mark
it as applied without re-running it.

```bash
ruprizzle migrate resolve --applied 20260101000000_broken
```

Use this only after manually inspecting the database and confirming it is in the
state the migration would have produced.

## `migrate reset`

Drop the database and replay the full migration history from the beginning.

```bash
ruprizzle migrate reset --force
```

This is destructive and `--force` is required. It is intended for development
prototyping only.

## Running migrations in CI / production

In CI and production, use `migrate deploy`.

```bash
ruprizzle migrate deploy
```

This command:

- reads `migrations/` in order,
- skips migrations that are already recorded in the `_Migration` table,
- applies each pending `up.sql` inside a transaction,
- never diffs or writes new migration files.

A typical container deploy looks like this:

```dockerfile
# Build the application image first
...
# At container start, before binding the port:
CMD ["sh", "-c", "ruprizzle migrate deploy && exec ./my-app"]
```

## Destructive changes and `--accept-data-loss`

`migrate dev` will stop and warn when a change would drop data, such as:

- dropping a column,
- dropping a table,
- changing a `NOT NULL` column to nullable and then back,
- narrowing a `BigInt` to `Int` where values may not fit.

Pass `--accept-data-loss` when the loss is intentional in development.

## The 12 change classes

The migration engine is tested against all common schema changes:

1. Add / drop a model
2. Add nullable column
3. Add `NOT NULL` column with default
4. Add `NOT NULL` column without default
5. Drop column
6. Widen / narrow `Int` ↔ `BigInt`
7. Nullable → `NOT NULL`
8. Add / drop index or unique
9. Add / drop foreign key
10. Add / drop enum variant
11. Rename column / model (with `@renamedFrom` or heuristic detection)
12. Add / remove default expression

## Rename detection

Renames are detected by comparing column/model fingerprints. If ruprizzle is
confident, it suggests a `RENAME` change. If you know a rename happened, use
`@renamedFrom` in the schema:

```prisma
model User {
  email String @renamedFrom("email_address")
}
```

Without `@renamedFrom`, the engine may produce a `DROP` + `ADD` pair, which
loses data. Always review renames.

## Mutual foreign-key cycles

If two or more models reference each other in a closed loop, the migration
planner will:

- add `DEFERRABLE INITIALLY IMMEDIATE` to the generated foreign key constraints;
- wrap the `up.sql` with a backend-specific command that defers enforcement until
  the end of the migration transaction;
- add the matching re-enable command before `COMMIT`.

The generated SQL is:

| Dialect | Preamble | Postamble | `down.sql` behaviour |
|---|---|---|---|
| Postgres | `SET CONSTRAINTS ALL DEFERRED;` | `SET CONSTRAINTS ALL IMMEDIATE;` | `DROP TABLE ... CASCADE` for cycle tables |
| SQLite | `PRAGMA defer_foreign_keys = ON;` | `PRAGMA defer_foreign_keys = OFF;` | `PRAGMA foreign_keys = OFF;` before drops, then `PRAGMA foreign_key_check;` and `PRAGMA foreign_keys = ON;` |
| MySQL | `SET FOREIGN_KEY_CHECKS = 0;` | `SET FOREIGN_KEY_CHECKS = 1;` | `SET FOREIGN_KEY_CHECKS = 0/1` around drops |

This lets you create, populate, and roll back schemas with cyclic foreign keys
without having to split them across multiple migrations by hand.

## SQLite migration notes

SQLite handles some of the change classes (notably type changes and `NOT NULL`
transitions) by rebuilding the table. Postgres uses direct `ALTER` statements
where possible. MySQL uses `ALTER TABLE` and `MODIFY COLUMN`.

## Best practices

- Commit `migrations/` with the same PR that changes `schema.ruprizzle`.
- Run `ruprizzle migrate deploy` in CI before running the application.
- Never run `migrate dev` on a production database.
- Review `up.sql` and `down.sql` before applying destructive changes.
- Use `--create-only` to draft a migration for review in a team PR.
