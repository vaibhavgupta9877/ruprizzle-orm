# ImplPlan 02 — Schema DSL & Parser (Phase P1)

**Duration:** 4 days · **Owners:** Vaibhav Gupta (grammar + validation rules), Vaibhav Gupta (lowering, tests)
**Exit gate G1:** the four `examples/` schemas parse into correct IR; all validation
errors reported in a single pass with accurate spans.

> **Status: ✅ COMPLETE.** All four tasks landed in `crates/parser/`; 20 parser
> tests plus 4 IR snapshots and 21 rule fixtures pass; `cargo xtask ci` (fmt,
> clippy `-D warnings`, test, docs) is green. The code is now the source of truth —
> the sketches below are kept for intent. Deviations are logged in
> [ImplPlan10AppendixDecisions.md](ImplPlan10AppendixDecisions.md#p1-deviation-log);
> two rules are deliberately not implemented, see [Known gaps](#known-gaps).
>
> Shipped surface: `parse(file_name, source) -> Result<ir::Schema, SchemaErrors>`,
> plus `parse_with_warnings` and `parse_ast`. Everything else — grammar, AST,
> lowering, validation — is private behind it, exactly as the fallback plan
> requires.

---

## The DSL

Prisma's syntax is the most learnable schema DSL in wide use, and thousands of
developers already know it. We adopt it deliberately rather than inventing
notation, with Rust-appropriate deviations.

`examples/blog/schema.ruprizzle`:

```prisma
datasource db {
  provider = "postgres"          // "postgres" | "sqlite"
  url      = env("DATABASE_URL")
}

generator client {
  output      = "src/db"          // where generated code lands
  module_name = "db"              // `mod db;` in the user's crate
}

/// A registered account.
enum Role {
  USER
  ADMIN
}

model User {
  id        Uuid     @id @default(uuid4())
  email     String   @unique
  name      String?
  role      Role     @default(USER)
  posts     Post[]
  createdAt DateTime @default(now())  @map("created_at")
  updatedAt DateTime @updatedAt       @map("updated_at")

  @@index([email])
  @@map("users")
}

model Post {
  id        Uuid    @id @default(uuid4())
  title     String  @db.VarChar(200)
  body      String?
  published Boolean @default(false)

  authorId  Uuid    @map("author_id")
  author    User    @relation(fields: [authorId], references: [id], onDelete: Cascade)

  @@index([authorId, published])
  @@map("posts")
}
```

### Deviations from Prisma, and why

| Prisma | ruprizzle | Reason |
|---|---|---|
| `Int` = i32, `BigInt` = i64 | same, plus explicit `@db.SmallInt` | Rust needs exact widths |
| `Decimal` → `Decimal` | `rust_decimal::Decimal` | no float money |
| implicit m:n join tables | **not in v1** — declare the join model | hidden tables fight the "predictable SQL" principle |
| `@default(cuid())` | `uuid4()`, `uuid7()`, `cuid2()`, `nanoid()` | uuid7 is time-ordered; better index locality |
| no doc → rustdoc | `///` becomes rustdoc on the generated struct/field | Rust users expect this |

`uuid7()` as the recommended default is a real advantage over Prisma's `cuid()`
for Postgres: monotonic keys avoid B-tree page splits on insert-heavy tables.

---

## P1-01 · Pest grammar ✅

**Owner:** Vaibhav Gupta · **Est:** 6h · **Shipped:** `crates/parser/src/schema.pest`,
`crates/parser/src/grammar.rs`, `crates/parser/src/ast.rs`

```pest
schema = { SOI ~ decl* ~ EOI }

decl = _{ datasource | generator | enum_def | model_def }

datasource = { "datasource" ~ ident ~ "{" ~ config_kv* ~ "}" }
generator  = { "generator"  ~ ident ~ "{" ~ config_kv* ~ "}" }
config_kv  = { ident ~ "=" ~ config_value }
config_value = _{ env_call | string | boolean | number }
env_call   = { "env" ~ "(" ~ string ~ ")" }

enum_def   = { doc_comment* ~ "enum" ~ ident ~ "{" ~ enum_variant* ~ "}" }
enum_variant = { doc_comment* ~ ident ~ ("@map" ~ "(" ~ string ~ ")")? }

model_def  = { doc_comment* ~ "model" ~ ident ~ "{" ~ model_member* ~ "}" }
model_member = _{ block_attr | field }

field      = { doc_comment* ~ ident ~ field_type ~ field_attr* }

// Order matters: list before optional before plain.
field_type = { ident ~ (list_marker | opt_marker)? }
list_marker = { "[" ~ "]" }
opt_marker  = { "?" }

field_attr = { "@" ~ attr_path ~ arg_list? }
block_attr = { "@@" ~ attr_path ~ arg_list? }
attr_path  = @{ ident ~ ("." ~ ident)? }        // supports `db.VarChar`

arg_list   = { "(" ~ (arg ~ ("," ~ arg)*)? ~ ")" }
arg        = { named_arg | value }
named_arg  = { ident ~ ":" ~ value }
value      = _{ func_call | array | string | number | boolean | ident }
func_call  = { ident ~ "(" ~ (value ~ ("," ~ value)*)? ~ ")" }
array      = { "[" ~ (value ~ ("," ~ value)*)? ~ "]" }

ident   = @{ (ASCII_ALPHA | "_") ~ (ASCII_ALPHANUMERIC | "_")* }
string  = ${ "\"" ~ inner_str ~ "\"" }
inner_str = @{ (!"\"" ~ ("\\\"" | ANY))* }
number  = @{ "-"? ~ ASCII_DIGIT+ ~ ("." ~ ASCII_DIGIT+)? }
boolean = { "true" | "false" }

doc_comment = ${ "///" ~ " "? ~ doc_text ~ NEWLINE }
doc_text    = @{ (!NEWLINE ~ ANY)* }

WHITESPACE = _{ " " | "\t" | "\r" | "\n" }
COMMENT    = _{ !"///" ~ "//" ~ (!NEWLINE ~ ANY)* }
```

**Two grammar traps to get right, both of which bite naive implementations:**

1. `COMMENT` must not swallow `///`. The negative lookahead `!"///"` before `"//"`
   is what keeps doc comments alive. Without it doc comments silently vanish and
   rustdoc output is empty with no error.
2. `doc_comment` is `${...}` (compound-atomic) so interior whitespace is preserved,
   but `WHITESPACE` still applies *between* doc comment lines.

A third trap surfaced during implementation and is worth the same billing: **a
silent rule still gets implicit whitespace inserted inside it.** The keyword rules
were written `_{ "model" ~ !ident_char }`, which Pest expands to `"model" ~ skip ~
!ident_char` — so the boundary check ran *after* the space and `model modelish`
parsed as a model named `ish`. Keywords are atomic in the shipped grammar (D-101),
as is `field_type` (D-102), which is what makes a missing type report as a field
type rather than as a bare identifier.

**Acceptance met:** grammar compiles; `parses_every_production` exercises every
rule; `doc_comments_survive_the_comment_rule` and
`keywords_do_not_swallow_identifier_prefixes` pin both traps; malformed input
produces a located error.

---

## P1-02 · AST → IR lowering ✅

**Owner:** Vaibhav Gupta · **Est:** 8h · **Shipped:** `crates/parser/src/lower.rs`,
`crates/parser/src/naming.rs`

Two-stage on purpose: parse to a loose AST that mirrors the grammar, then lower to
the strict IR. Do not try to build IR directly in the parse walk — relation
resolution needs the full model set, which does not exist mid-parse.

```
Pest pairs ──▶ ast::Schema ──▶ [pass 1: collect names]
                                      │
                                      ▼
                              [pass 2: lower types]  ── resolves Model/Enum refs
                                      │
                                      ▼
                              [pass 3: resolve relations] ── pairs both sides
                                      │
                                      ▼
                              [pass 4: apply naming] ── @map, conventions
                                      │
                                      ▼
                              [pass 5: validate] ── ImplPlan02 rule table
                                      │
                                      ▼
                                  ir::Schema
```

**Pass 1** builds the name environment (`HashMap<String, DeclKind>`) so pass 2 can
tell `Role` (enum) from `User` (model) from `String` (scalar) — a single-pass
parser cannot, because a type may be referenced before it is declared.

**Pass 4 naming conventions** (applied only where no explicit `@map`):
- Model `User` → table `users` (PascalCase → snake_case, pluralized)
- Field `createdAt` → column `created_at`
- Enum `Role` → Postgres type `role`
- Pluralization uses a small irregular-noun table plus `s`/`es`/`ies` rules. Keep
  it dumb and documented; `@@map` is the escape hatch for anything surprising.

Two resolution rules were settled during implementation and are worth stating
here, because downstream phases depend on them:

- **`references:` defaults to the target's primary key** when omitted (D-104).
- **Referential defaults are `onDelete: Restrict` for a required relation and
  `SetNull` for an optional one, `onUpdate: Cascade`** (D-105). Deleting a row out
  from under a required foreign key must fail; an optional one can be cleared.

**Acceptance met:** `crates/parser/tests/examples.rs` snapshots the full IR of all
four schemas under `examples/` (`examples__{blog,ecommerce,saas,social}.snap`) and
asserts the load-bearing properties directly: naming conventions, canonical
relations reached from both sides, composite keys, named and self relations, and
fingerprint stability.

---

## P1-03 · Validation rules ✅

**Owner:** Vaibhav Gupta · **Est:** 6h · **Shipped:** `crates/parser/src/validate.rs`
(V01, V11, V14-empty, V16, V17) and `crates/parser/src/lower.rs` (V02–V10,
V12–V15). The split is on "what does this rule need to point at": a rule that must
underline `@updatedAt` needs the attribute's span, which the IR deliberately does
not keep (D-103).

Every rule is one `SchemaError` variant. The validator **collects** rather than
short-circuits.

| # | Rule | Error |
|---|---|---|
| V01 | Every model has exactly one `@id` or one `@@id([...])` | `MissingPrimaryKey` / `MultiplePrimaryKeys` |
| V02 | Field types resolve to scalar, enum, or model | `UnknownType` (+ suggestion) |
| V03 | Model and enum names are unique, PascalCase | `DuplicateDecl` / `NamingConvention` |
| V04 | Field names unique within a model | `DuplicateField` |
| V05 | `@relation(fields:)` columns exist and are scalar | `UnknownRelationField` |
| V06 | `@relation(references:)` targets exist and are unique/PK | `InvalidRelationTarget` |
| V07 | Relation FK type matches referenced type | `RelationTypeMismatch` |
| V08 | Every relation has exactly one owning side | `AmbiguousRelation` / `MissingBackRelation` |
| V09 | `@default` value type matches field type | `DefaultTypeMismatch` |
| V10 | `@updatedAt` only on `DateTime` | `InvalidAttributeTarget` |
| V11 | `@@index`/`@@unique` reference existing scalar fields | `UnknownIndexField` |
| V12 | List fields (`T[]`) are relations only — no scalar arrays in v1 | `ScalarListUnsupported` |
| V13 | Optional relation implies nullable FK column | `RelationNullabilityMismatch` |
| V14 | Enum has ≥1 variant; variants unique | `EmptyEnum` / `DuplicateVariant` |
| V15 | `datasource.provider` is a supported dialect | `UnknownProvider` |
| V16 | No table/column name collides after `@map` resolution | `NameCollision` |
| V17 | Reserved Rust keywords in field names get `r#` escaping, warn if unclear | `ReservedKeyword` (warning) |
| V18 | Dialect capability check (see P2) | `UnsupportedByDialect` — deferred to P2 (D-108) |

**V08 deserves detail** because it is where most schema bugs live. A relation is
well-formed when:
- exactly one side carries `@relation(fields: [...], references: [...])` (the
  owning/FK side), and
- the other side declares the inverse field (`User.posts: Post[]` ↔ `Post.author: User`), and
- if a model has two relations to the same model, both carry an explicit relation
  name: `@relation("author", ...)` / `@relation("editor", ...)`.

Without the third clause, `Post.authorId` and `Post.editorId` both pointing at
`User` is ambiguous, and the error must say exactly that plus show the fix.

**Acceptance met:** 21 fixtures under `crates/parser/tests/invalid/`, one per rule
(three for V08, two each for V01 and V14), each asserting its diagnostic code and
snapshotting the rendered span. Two further tests hold the line on quality:
`every_error_points_somewhere_and_says_what_to_do` asserts a label and a `help(...)`
on every diagnostic the parser actually produces, and
`several_mistakes_are_reported_in_one_pass` proves three mistakes yield three
diagnostics from one run.

---

## P1-04 · Parser error UX ✅

**Owner:** Vaibhav Gupta · **Est:** 4h · **Shipped:** `crates/parser/src/errors.rs`

Pest's raw errors are mechanical. Wrap them:

```
Error: ruprizzle::parse::unexpected_token

  × expected a field type after `email`
   ╭─[schema.ruprizzle:12:3]
 11 │ model User {
 12 │   email  @unique
    ·          ┬
    ·          ╰── expected a type here, found attribute `@unique`
 13 │ }
   ╰────
  help: fields are written `name Type @attrs`, e.g. `email String @unique`
```

Map Pest's `positives`/`negatives` token sets onto human phrasing via a lookup
table keyed by `Rule`. Do not surface raw rule names such as `field_type` to users.

Keying on the *set* rather than on a single rule is what makes this work: Pest
reports alternatives, and the combination is what identifies the situation —
`string | number | boolean | env_call` only ever co-occur on the right-hand side of
a configuration entry, and `schema` alone means nothing matched at the top level.

**Acceptance met:** `common_mistakes_get_tailored_messages` runs all five — a
field with no type, an unquoted configuration value, a misspelled declaration
keyword (which gets a "did you mean `model`?"), a missing closing brace, and a
bare `@` — and asserts that no raw grammar rule name appears in any of them.

---

## The example set

Four schemas under `examples/`, chosen so that between them they cover every
shape the parser has to get right:

| Example | Covers |
|---|---|
| `blog/` | the canonical one-to-many: enums, docs, `@map`/`@@map`, `@db.VarChar`, `@updatedAt`, composite `@@index` |
| `ecommerce/` | `Decimal` money, a status enum, an explicit join model with a composite `@@id`, `Restrict` vs `Cascade` |
| `saas/` | targets **SQLite**, so the second dialect cannot rot; `Json`, optional scalars, `@@unique` |
| `social/` | the awkward shapes — two named relations between one pair of models, and a self-relation (`Thread.parent` / `Thread.replies`) |

All four lower warning-free, which is itself asserted.

## Known gaps

- **V03's `PascalCase` check is not implemented** (D-107). There is no
  `NamingConvention` variant in `SchemaError`, and a warning that fires on every
  deliberately-lowercase model name is worse than none. The duplicate-declaration
  half of V03 — the part that catches real bugs — is enforced.
- **V18 is deferred to P2** (D-108). It is defined by the dialect capability
  matrix, which does not exist yet; `SchemaError::DialectDegraded` is waiting for
  it.
- **Sort direction in `@@index`** is parsed as `Asc` unconditionally. The IR
  carries `SortOrder`, so `@@index([createdAt(sort: Desc)])` is a grammar addition
  when P2 needs it, not an IR change.

## Phase P1 checklist

- [x] P1-01 grammar parses all four examples
- [x] P1-02 lowering produces snapshot-verified IR
- [x] P1-03 16 of 18 validation rules implemented with fixtures (V03-naming and
      V18 deliberately deferred — see Known gaps)
- [x] P1-04 friendly parse errors for the top 5 mistakes
- [x] Multi-error reporting confirmed (3 errors → 3 diagnostics, one run)
- [x] **G1 signed off by Vaibhav Gupta** — re-verified 2026-08-10: `cargo build --workspace`
      clean, `cargo clippy --workspace --all-targets -- -D warnings` clean, parser
      suites green (11 lib + 6 `examples.rs` + 3 `invalid.rs` + 1 doctest). G1 needs
      no live database, so this sign-off is complete and unqualified.

## Fallback (per RealityCheck kill criteria) — not needed

Recorded as it stood before implementation. Pest cost about half a day of fights,
all of them whitespace-in-silent-rules (D-101, D-102), and none of them reached the
day-3 trigger. The boundary below held anyway: nothing outside `crates/parser`
knows Pest exists.

If Pest is fighting us at day 3: the grammar is an *implementation detail* behind
`parser::parse(&str) -> Result<ir::Schema>`. Swap to a hand-written recursive
descent parser (est. +2 days) with no impact on P2–P8. Do not let a parser choice
threaten the schedule; the IR boundary exists precisely to make this cheap.
