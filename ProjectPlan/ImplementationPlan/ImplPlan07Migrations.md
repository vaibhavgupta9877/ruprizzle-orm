# ImplPlan 07 — Migrations: Snapshot, Diff, Apply (Phase P6)

**Duration:** 5 days · **Owners:** Claude (diff engine), Devin (runner, drift, CLI wiring)
**Exit gate G6:** the 12 change classes below each produce correct migrations that
apply cleanly and preserve data, on both dialects.

---

## The model

No Rust ORM has Prisma's migration workflow: Diesel makes you hand-write both
directions, SeaORM makes you write migrations as Rust code, sqlx gives you a bare
file runner. **Automatic diffing from a declarative schema is the headline feature
of this phase.**

```
schema.ruprizzle  ──parse──▶  ir::Schema  (desired state)
                                    │
migrations/.snapshot.json ──────▶ ir::Schema  (last known state)
                                    │
                                    ▼
                          diff(prev, next) -> Vec<Change>
                                    │
                                    ▼
                        plan(changes, dialect) -> Vec<Stmt>   (ordered, safety-annotated)
                                    │
                                    ▼
                  migrations/20260810_143022_add_role/{up.sql, down.sql}
```

The snapshot **is** the serialized IR (P0-02 made `Schema` serde-capable exactly for
this) — one type, no drift between what the parser produces and what the differ
compares.

---

## P6-01 · Migration directory format

**Owner:** Devin · **Est:** 3h

```
migrations/
├── .snapshot.json                     # IR after the last migration
├── 20260810_143022_init/
│   ├── up.sql
│   ├── down.sql
│   └── meta.json                      # checksum, destructive flags, ruprizzle version
└── 20260812_090114_add_user_role/   # same three files
```

Applied migrations are tracked in a table the ORM owns:

```sql
CREATE TABLE _ruprizzle_migrations (
    id             TEXT PRIMARY KEY,      -- directory name
    checksum       TEXT NOT NULL,         -- sha256 of up.sql
    applied_at     TIMESTAMPTZ NOT NULL,
    execution_ms   BIGINT NOT NULL,
    rolled_back_at TIMESTAMPTZ
);
```

**Checksums are enforced.** If a migration file changed after being applied, the
runner refuses to proceed and names the file — editing applied migrations is the
most common way teams corrupt a database.

---

## P6-02 · The diff engine

**Owner:** Claude · **Est:** 10h — *the hardest single task in the project*

```rust
pub enum Change {
    CreateEnum(EnumDef),
    DropEnum(EnumName),
    AddEnumVariant  { enum_: EnumName, variant: String },
    DropEnumVariant { enum_: EnumName, variant: String },   // destructive

    CreateModel(Model),
    DropModel(ModelName),                                    // destructive
    RenameModel { from: ModelName, to: ModelName, new_table: String },

    AddColumn   { model: ModelName, field: Field },
    DropColumn  { model: ModelName, field: FieldName },       // destructive
    AlterColumn { model: ModelName, from: Field, to: Field, aspects: Vec<ColumnAspect> },
    RenameColumn{ model: ModelName, from: FieldName, to: FieldName, new_column: String },

    CreateIndex(ModelName, IndexDef),
    DropIndex(ModelName, String),
    AddUnique(ModelName, UniqueDef),
    DropUnique(ModelName, String),

    AddForeignKey(ModelName, ResolvedRelation),
    DropForeignKey(ModelName, String),
    AlterReferentialAction { /* ... */ },
}

pub enum ColumnAspect { Type, Nullability, Default, Identity }
```

### Ordering — the part that is easy to get wrong

Statements must be emitted in dependency order or they simply fail:

```
1. CREATE / ALTER enums (add variants)
2. CREATE tables            — no FKs yet
3. ADD columns
4. ALTER columns            — widen before narrow
5. Data backfills           — user-editable section (see P6-04)
6. ADD foreign keys         — after all referenced tables exist
7. CREATE indexes / uniques — after data is in place, cheaper
8. DROP foreign keys
9. DROP indexes
10. DROP columns            — destructive
11. DROP tables             — destructive, reverse topological order
12. DROP enums              — last, nothing references them
```

Within *create table* and *drop table*, sort by the FK dependency graph (topological
for creation, reversed for drops). Cyclic FKs (`A.b_id -> B`, `B.a_id -> A`) cannot
be resolved by ordering alone: create both tables without FKs, then add both in step
6. The planner detects cycles with a strongly-connected-components pass.

### Rename detection

Pure structural diffing cannot distinguish "renamed `name` to `full_name`" from
"dropped `name`, added `full_name`" — and guessing wrong destroys data. Two
mechanisms:

1. **Explicit, authoritative:** `@renamedFrom("name")` in the schema. Always wins.
2. **Heuristic prompt:** when a drop and an add in the same model have identical
   type, nullability, and default, `ruprizzle migrate dev` *asks*
   interactively — never assumes:

```
? Column `users.name` was removed and `users.full_name` was added,
  and they have the same type. Did you rename it?
  > Yes, rename (preserves data)
    No, drop and create (DATA LOSS on `name`)
```

In non-interactive mode (`--yes` / CI) the heuristic is **disabled** and the change
becomes drop+add, which the destructive guard then blocks — failing loudly in CI is
correct, silently guessing is not.

### Destructive-change guard

Any `Change` marked destructive halts `migrate dev` unless `--accept-data-loss` is
passed, and prints exactly what will be lost, with row counts queried live from the
database when a connection is available:

```
⚠ This migration will cause data loss:
  • DROP COLUMN users.legacy_id        (4,182 non-null rows)
  • DROP TABLE  audit_log_v1           (91,004 rows)
Re-run with --accept-data-loss to proceed.
```

**Acceptance:** a property test generating random schema pairs asserts that
`apply(diff(a, b))` starting from `a` yields a database whose introspected shape
equals `b`. This round-trip property will find ordering bugs no human enumerates.

---

## P6-03 · `down.sql` generation

**Owner:** Devin · **Est:** 4h

Generated by running the diff **in reverse** (`diff(next, prev)`), not by
hand-inverting statements.

Honesty requirement: a down migration cannot restore dropped data, so say so in the
file itself rather than implying reversibility:

```sql
-- ⚠ IRREVERSIBLE: this down-migration restores the *schema* but not the data
-- that was removed by DROP COLUMN users.legacy_id in the corresponding up.sql.
ALTER TABLE "users" ADD COLUMN "legacy_id" TEXT;
```

---

## P6-04 · Backfill hook

**Owner:** Devin · **Est:** 2h

The one thing automatic diffing genuinely cannot do: populate a new `NOT NULL`
column on an existing table. The planner detects this case and emits a marked,
editable section rather than producing a migration that cannot apply:

```sql
-- ▼▼▼ RUPRIZZLE:BACKFILL — edit this block; it is preserved on regeneration ▼▼▼
-- Column users.display_name is NOT NULL with no default, and `users` has rows.
-- Provide a backfill, then the NOT NULL constraint below will succeed.
UPDATE "users" SET "display_name" = "email" WHERE "display_name" IS NULL;
-- ▲▲▲ RUPRIZZLE:BACKFILL ▲▲▲
ALTER TABLE "users" ALTER COLUMN "display_name" SET NOT NULL;
```

Regeneration preserves anything between the markers. The three-step add-nullable →
backfill → set-not-null sequence is generated automatically; the user only fills in
the middle.

---

## P6-05 · The runner

**Owner:** Devin · **Est:** 5h

```rust
pub struct Migrator { /* ... */ }

impl Migrator {
    pub async fn pending(&self)  -> Result<Vec<MigrationId>>;
    pub async fn apply_all(&self) -> Result<Report>;
    pub async fn rollback(&self, n: usize) -> Result<Report>;
    pub async fn status(&self)   -> Result<Status>;
    pub async fn verify_checksums(&self) -> Result<()>;
}
```

- **Advisory lock** before applying: `pg_advisory_lock(<const>)` on Postgres, an
  exclusive lock on SQLite. Two app instances booting simultaneously must not race
  the same migration — this is a real production failure mode for the "run
  migrations on startup" pattern we expect people to use.
- Each migration runs in one transaction where the dialect allows it. Statements
  flagged `transactional: false` (P2-01) run outside, and the file is split
  accordingly, with the split noted in `meta.json`.
- On failure: roll back the current migration, record nothing, report the exact
  failing statement with its line in `up.sql`.
- Embeddable in the user's app: `ruprizzle::migrate::embed!("migrations")` bakes
  the directory into the binary via `include_dir` so deployments ship one artifact.

---

## P6-06 · Drift detection

**Owner:** Devin · **Est:** 4h

Three states must be distinguishable, and conflating them is how people lose data:

| State | Meaning | Command reaction |
|---|---|---|
| **In sync** | DB = snapshot = schema | nothing to do |
| **Pending** | schema ≠ snapshot | generate a migration |
| **Drift** | DB ≠ snapshot | someone changed the DB by hand |

Drift is detected by introspecting the live database into a partial IR and diffing
against the snapshot. `ruprizzle migrate status` reports it:

```
Drift detected: your database does not match the migration history.
  • table `users` has column `hotfix_flag` not present in any migration
  • index `idx_users_email` is missing from the database
Fix by: `ruprizzle migrate resolve` (record as applied)
     or `ruprizzle migrate reset` (DROP EVERYTHING and replay — dev only)
```

The introspector needed here is a reduced version of the full `db pull` feature
that ImplPlan10 defers — it only needs enough fidelity to compare, not to generate
a schema file.

---

## The 12 change classes (acceptance matrix)

Each row is a dedicated integration test on **both** dialects, asserting the
migration applies and pre-existing rows survive.

| # | Change | Postgres | SQLite | Destructive |
|---|---|---|---|---|
| 1 | add model | direct | direct | no |
| 2 | drop model | direct | direct | **yes** |
| 3 | add nullable column | direct | direct | no |
| 4 | add NOT NULL column w/ default | direct | direct | no |
| 5 | add NOT NULL column w/o default | 3-step backfill | 3-step backfill | no |
| 6 | drop column | direct | direct (3.35+) | **yes** |
| 7 | widen type (Int→BigInt) | `ALTER TYPE` | table rebuild | no |
| 8 | narrow type (BigInt→Int) | `ALTER TYPE USING` | table rebuild | **yes** |
| 9 | nullable → NOT NULL | backfill + `SET NOT NULL` | table rebuild | **yes** |
| 10 | add/drop index or unique | direct | direct | no |
| 11 | add/drop FK, change onDelete | direct | table rebuild | no |
| 12 | add/drop enum variant | `ALTER TYPE ADD VALUE` / rebuild | CHECK rebuild | drop=**yes** |

---

## Phase P6 checklist

- [x] P6-01 directory format, `_ruprizzle_migrations`, checksum enforcement
  - `Migrator` reads `migrations/<id>/{up.sql,down.sql,meta.json}`, computes and
    verifies SHA-256 checksums, and creates the `_ruprizzle_migrations` tracking
    table on first use.
- [~] P6-02 diff engine, dependency ordering, cycle handling, rename policy
  - Core `diff`/`plan` implemented for: create/drop model, add/drop column,
    alter column, create/drop index, create/drop unique, create/drop enum,
    add/drop FK, and authoritative `@renamedFrom` rename hints.
  - Dependency ordering for `CREATE`/`DROP` and `ALTER` statements is in place.
  - Mutual-FK cycles and heuristic rename prompting are **not** implemented.
- [ ] P6-02 round-trip property test passing
- [x] P6-03 reverse-diff `down.sql` with honest irreversibility notes
  - `ruprizzle_migrate::down_sql` diffs `next` back to `prev` and prefaces the
    output with notes about data that cannot be restored.  Each destructive
    statement is also annotated inline.
- [x] P6-04 backfill hook preserved across regeneration
  - `plan` emits a marked, editable `RUPRIZZLE:BACKFILL` block when adding a
    NOT NULL column that has no default, splitting the operation into nullable
    add -> user backfill -> NOT NULL alter.
  - `Migrator::apply_all` rejects migrations that still contain the placeholder
    commented `UPDATE`, so the hook is exercised before destructive failures.
- [x] P6-05 runner with advisory lock + embeddable migrations
  - `Migrator::{pending,apply_all,status,verify_checksums,rollback}` work and
    run each migration in a transaction with per-statement error reporting.
  - `apply_all` acquires a Postgres `pg_advisory_xact_lock` per migration;
    SQLite locking is left to the engine's default transaction locking.
  - `embed!` compile-time embedding is **not** implemented.
- [x] P6-06 drift detection via lightweight introspection
  - `ruprizzle_migrate::detect` introspects the live database (SQLite via
    `sqlite_master`/`PRAGMA table_info`, Postgres via `information_schema`)
    and reports table/column/nullability drift as human-readable strings.
- [ ] All 12 change classes green on both dialects
  - Currently verified: add model, add nullable/NOT-NULL column with default,
    add index/unique, add FK (Postgres), create enum (Postgres).
  - SQLite FK additions after `CREATE TABLE` and destructive changes that need
    table rebuilds are limited.
- [x] CLI wiring for `migrate deploy` and `migrate status`
- [ ] **G6 signed off by Claude**
