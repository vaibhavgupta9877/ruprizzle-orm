# Migrations guide

`ruprizzle` generates migrations by diffing your current schema against the state
of the migration history.

## The two commands

| Command | When to use | What it does |
|---|---|---|
| `ruprizzle migrate dev` | Local development | Diff → write migration → apply → regenerate client. |
| `ruprizzle migrate deploy` | CI / production | Apply pending `migrations/*.sql` files only. |

These are deliberately separate. `deploy` never writes migration files, prompts,
or diffs the schema, so a habit of running `migrate deploy` cannot accidentally
drop a production column.

## Development workflow

```bash
# 1. Edit schema.ruprizzle
# 2. Create and apply a migration
ruprizzle migrate dev --name add_profile

# 3. If the migration is destructive and you accept the data loss:
ruprizzle migrate dev --name drop_legacy --accept-data-loss

# 4. To write the migration without applying it:
ruprizzle migrate dev --name draft --create-only
```

## Backfills

Adding a `NOT NULL` column without a default requires a three-step migration:

1. Add the column as nullable.
2. Fill existing rows (`RUPRIZZLE:BACKFILL` block in the generated `up.sql`).
3. Alter the column to `NOT NULL`.

Edit the generated `up.sql` and replace the placeholder expression before
applying. The `RUPRIZZLE:BACKFILL` block is preserved when you re-run
`migrate dev`.

## Drift

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

SQLite handles some of these (notably type changes and `NOT NULL` transitions)
by rebuilding the table. Postgres uses direct `ALTER` statements where possible.
