# ImplPlan 08 — CLI & Developer Experience (Phase P7)

**Duration:** 3 days · **Owners:** Devin (commands), Claude (error UX review)
**Exit gate:** a new user goes from empty directory to a working query in under
five minutes, verified by an actual timed walkthrough.

---

## Why DX gets a dedicated phase

The RealityCheck doc is right that this project cannot win on raw performance —
sqlx does the I/O, so we are within noise of hand-written sqlx by construction.
What we can win on is the thing Rust database tooling is genuinely weakest at:
the first thirty minutes. That is a deliberate positioning choice, and it means
CLI polish is a feature, not a chore.

---

## P7-01 · Command surface

**Owner:** Devin · **Est:** 6h

```
ruprizzle init [--provider postgres|sqlite]
    Scaffold schema.ruprizzle, .env, migrations/, and add deps to Cargo.toml.

ruprizzle generate [--watch]
    Parse + validate + emit Rust into generator.output.
    --watch re-runs on file change (notify-rs), sub-second for typical schemas.

ruprizzle migrate dev [--name <n>] [--accept-data-loss] [--create-only]
    Diff schema against snapshot, write a migration, apply it, regenerate client.
    The single command a developer uses all day.

ruprizzle migrate deploy
    Apply pending migrations only. No diffing, no prompts, no codegen.
    This is the production/CI command.

ruprizzle migrate status | resolve <id> | reset
ruprizzle db push [--accept-data-loss]
    Diff and apply directly with no migration file. Prototyping only; prints a
    warning saying so every single time.

ruprizzle db seed
    Run `seeds/main.rs` against the configured database.

ruprizzle format
    Canonical formatting of schema.ruprizzle (alignment, attribute order).

ruprizzle validate
    Parse + validate + capability-check. Exit non-zero on error. For CI.
```

`migrate dev` vs `migrate deploy` is a hard split. Conflating them is how a
prototyping command ends up dropping a production column; separate binaries-level
behaviour makes the dangerous one impossible to invoke by habit.

**Config resolution order:** CLI flag → `RUPRIZZLE_*` env → `.env` file →
`datasource` block. Print which source won under `--verbose`.

---

## P7-02 · Error output standard

**Owner:** Claude · **Est:** 4h

Every user-facing error answers three questions: what happened, where, and what to
do next. Enforced by review, and by a test that asserts every `SchemaError` variant
has a non-empty `help()`.

```
Error: ruprizzle::relation::ambiguous

  × two relations from `Post` to `User` need explicit names
    ╭─[schema.ruprizzle:31:3]
 30 │   authorId Uuid
 31 │   author   User @relation(fields: [authorId], references: [id])
    ·            ──┬─
    ·              ╰── first relation to `User`
 32 │   editorId Uuid?
 33 │   editor   User? @relation(fields: [editorId], references: [id])
    ·            ──┬──
    ·              ╰── second relation to `User`
    ╰────
  help: name each relation so the back-references are unambiguous:
          author User  @relation("PostAuthor", fields: [authorId], references: [id])
          editor User? @relation("PostEditor", fields: [editorId], references: [id])
        then on `User`:
          authored Post[] @relation("PostAuthor")
          edited   Post[] @relation("PostEditor")
```

Note the help shows *the actual fix for this schema*, with the user's own
identifiers substituted in — not a generic template. That is the standard.

Runtime errors get the same treatment:

```rust
#[error("unique constraint violated on `users.email`")]
#[diagnostic(
    code(ruprizzle::unique_violation),
    help("a row with email = 'a@b.c' already exists; use \
          `db.user().upsert()` if you meant to insert-or-update")
)]
UniqueViolation { table: String, columns: Vec<String>, value: Option<String> },
```

Mapping raw driver errors (Postgres `SQLSTATE 23505`, SQLite
`SQLITE_CONSTRAINT_UNIQUE`) into this typed set is a dialect responsibility, and it
is what turns an opaque database error into an actionable one. Cover: unique
violation, FK violation, not-null violation, check violation, deadlock,
serialization failure, connection failure.

---

## P7-03 · Watch mode

**Owner:** Devin · **Est:** 3h

`ruprizzle generate --watch` is the loop developers live in. Requirements:
debounce 150 ms, incremental (skip emission when the schema hash is unchanged),
clear the previous error block on success, and never leave the output directory in
a broken state after a parse failure — the last good generation stays on disk so
`cargo check` in another terminal keeps working.

Target: under 200 ms end-to-end for a 20-model schema. Parsing and codegen are
pure CPU with no I/O beyond the file write, so this is achievable; measure it.

---

## P7-04 · `init` scaffolding

**Owner:** Devin · **Est:** 3h

`ruprizzle init` produces a working starting point, not an empty file:

```
schema.ruprizzle    # datasource + generator + a commented User model
.env                # DATABASE_URL with a working local default
.gitignore          # appends /src/db (generated) if not present
migrations/         # empty, with a README
```

and prints the next three commands verbatim so they can be pasted:

```
✓ Initialised ruprizzle (provider: postgres)

  Next:
    1. edit schema.ruprizzle
    2. ruprizzle migrate dev --name init
    3. cargo add ruprizzle
```

It also detects an existing `Cargo.toml` and offers to add the `ruprizzle`
dependency, but **never edits it without asking**.

---

## P7-05 · Documentation deliverables

**Owner:** Claude · **Est:** 5h

| Doc | Content | Length |
|---|---|---|
| README | pitch, 20-line example, install, comparison table | 1 page |
| Quickstart | empty dir → first query | 5 min read |
| Schema reference | every type, attribute, and function | complete |
| Query guide | filters, projections, pagination, transactions | with runnable examples |
| Relations guide | include, nested, some/every/none, N+1 explanation | with SQL shown |
| Migrations guide | dev vs deploy, drift, backfills, the 12 change classes | complete |
| Dialect notes | Postgres vs SQLite differences, honestly | table |
| Known limitations | everything in ImplPlan10's deferral list | explicit |
| Migrating from | SeaORM / Diesel / sqlx cheat-sheets | 1 page each |

Every code sample in the docs is compiled in CI via `doc_comment` or an
`examples/` binary. Documentation samples that do not compile are the fastest way
to lose the trust the docs were meant to build.

The "Known limitations" page is a differentiator, not an admission. The
RealityCheck doc's instinct — publish honest constraints — is right, and for an
alpha ORM asking people to trust it with their data, it is table stakes.

---

## P7-06 · Editor support (stretch)

**Owner:** Devin · **Est:** 3h, cut first if the phase overruns

- TextMate grammar for `.ruprizzle` → syntax highlighting in VS Code + JetBrains.
  This is a JSON file and a few hours; the payoff in perceived polish is
  disproportionate.
- A full LSP (completion, go-to-definition, inline diagnostics) is **0.2 scope**.
  The parser and validator already expose everything an LSP needs, so this stays
  cheap to add later — which is the reason spans were built into the IR in P0.

---

## Phase P7 checklist

- [ ] P7-01 all commands implemented, `dev`/`deploy` split enforced
- [ ] P7-02 every error has span + actionable help; driver errors mapped to typed set
- [ ] P7-03 watch mode under 200 ms, safe on parse failure
- [ ] P7-04 `init` scaffolds a working project
- [ ] P7-05 nine docs written, all samples compiled in CI
- [ ] P7-06 syntax highlighting (stretch)
- [ ] Timed walkthrough: empty dir → first query in under 5 minutes
