# ImplPlan 02 — Schema DSL & Parser (Phase P1)

**Duration:** 4 days · **Owners:** Claude (grammar + validation rules), Devin (lowering, tests)
**Exit gate G1:** the four `examples/` schemas parse into correct IR; all validation
errors reported in a single pass with accurate spans.

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

## P1-01 · Pest grammar

**Owner:** Claude · **Est:** 6h · File: `crates/parser/src/schema.pest`

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

**Acceptance:** grammar compiles; a fixture exercising every production parses;
malformed inputs produce a Pest error with line/column.

---

## P1-02 · AST → IR lowering

**Owner:** Devin · **Est:** 8h · File: `crates/parser/src/lower.rs`

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

**Acceptance:** all four example schemas lower to IR matching hand-written
expected values (`insta` snapshots of the IR).

---

## P1-03 · Validation rules

**Owner:** Claude · **Est:** 6h · File: `crates/parser/src/validate.rs`

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
| V18 | Dialect capability check (see P2) | `UnsupportedByDialect` |

**V08 deserves detail** because it is where most schema bugs live. A relation is
well-formed when:
- exactly one side carries `@relation(fields: [...], references: [...])` (the
  owning/FK side), and
- the other side declares the inverse field (`User.posts: Post[]` ↔ `Post.author: User`), and
- if a model has two relations to the same model, both carry an explicit relation
  name: `@relation("author", ...)` / `@relation("editor", ...)`.

Without the third clause, `Post.authorId` and `Post.editorId` both pointing at
`User` is ambiguous, and the error must say exactly that plus show the fix.

**Acceptance:** one fixture per rule under `crates/parser/tests/invalid/`, each
asserting the specific error code and span via `insta`.

---

## P1-04 · Parser error UX

**Owner:** Devin · **Est:** 4h

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

**Acceptance:** five common syntax mistakes each produce a tailored message.

---

## Phase P1 checklist

- [ ] P1-01 grammar parses all four examples
- [ ] P1-02 lowering produces snapshot-verified IR
- [ ] P1-03 all 18 validation rules implemented with fixtures
- [ ] P1-04 friendly parse errors for the top 5 mistakes
- [ ] Multi-error reporting confirmed (3 errors → 3 diagnostics, one run)
- [ ] **G1 signed off by Claude**

## Fallback (per RealityCheck kill criteria)

If Pest is fighting us at day 3: the grammar is an *implementation detail* behind
`parser::parse(&str) -> Result<ir::Schema>`. Swap to a hand-written recursive
descent parser (est. +2 days) with no impact on P2–P8. Do not let a parser choice
threaten the schedule; the IR boundary exists precisely to make this cheap.
