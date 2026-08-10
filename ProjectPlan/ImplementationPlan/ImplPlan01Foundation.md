# ImplPlan 01 — Foundation (Phase P0)

**Duration:** 2 days · **Owners:** Claude (IR design), Devin (CI, harness)
**Exit gate:** workspace builds, CI green, both databases reachable from tests.

---

## P0-01 · Workspace scaffold

**Owner:** Devin · **Est:** 3h

```
ruprizzle-orm/
├── Cargo.toml                 # workspace root
├── crates/
│   ├── core/                  # ruprizzle-core
│   ├── parser/                # ruprizzle-parser
│   ├── dialect/               # ruprizzle-dialect
│   ├── codegen/               # ruprizzle-codegen
│   ├── migrate/               # ruprizzle-migrate
│   ├── runtime/               # ruprizzle       (the user-facing crate)
│   ├── macros/                # ruprizzle-macros
│   └── cli/                   # ruprizzle-cli   (bin: `ruprizzle`)
├── examples/
│   ├── blog/  saas-tenant/  ecommerce/  minimal/
├── tests/                     # workspace-level integration tests
├── xtask/                     # cargo xtask for release/codegen chores
└── docker-compose.yml         # postgres:17 + adminer
```

Root `Cargo.toml`:

```toml
[workspace]
members = ["crates/*", "xtask"]
resolver = "3"

[workspace.package]
version      = "0.1.0-alpha.1"
edition      = "2024"
rust-version = "1.85"
license      = "MIT OR Apache-2.0"
repository   = "https://github.com/<org>/ruprizzle-orm"

[workspace.dependencies]
pest        = "2.8"
pest_derive = "2.8"
sqlx        = { version = "0.8", default-features = false, features = ["runtime-tokio", "macros", "uuid", "chrono", "json", "rust_decimal"] }
thiserror   = "2"
miette      = { version = "7", features = ["fancy"] }
serde       = { version = "1", features = ["derive"] }
serde_json  = "1"
proc-macro2 = "1"
quote       = "1"
syn         = { version = "2", features = ["full"] }
prettyplease = "0.2"
clap        = { version = "4", features = ["derive", "env"] }
tokio       = { version = "1", features = ["macros", "rt-multi-thread"] }
indexmap    = { version = "2", features = ["serde"] }
```

**Pin discipline:** `sqlx` is the only dependency whose breakage is existential.
Pin it exactly (`=0.8.x`) in the runtime crate and bump deliberately.

**Acceptance:** `cargo build --workspace` and `cargo test --workspace` succeed on a
clean checkout.

---

## P0-02 · Core IR (`ruprizzle-core`)

**Owner:** Claude · **Est:** 5h

This is the contract every other crate speaks. Get it right once; churn here is
the single biggest schedule risk.

```rust
// crates/core/src/ir.rs

/// The fully validated, dialect-agnostic representation of a schema.
/// Everything downstream (codegen, migrations, dialects) consumes only this.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Schema {
    pub datasource: Datasource,
    pub generator:  Generator,
    pub enums:      IndexMap<EnumName, EnumDef>,
    pub models:     IndexMap<ModelName, Model>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Model {
    pub name:        ModelName,      // Rust/PascalCase, e.g. `User`
    pub table:       String,         // physical table, e.g. `users`
    pub fields:      IndexMap<FieldName, Field>,
    pub primary_key: PrimaryKey,     // single or composite
    pub indexes:     Vec<IndexDef>,
    pub uniques:     Vec<UniqueDef>,
    pub docs:        Option<String>, // `///` comments -> rustdoc on the struct
    pub span:        Span,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Field {
    pub name:     FieldName,         // Rust field, snake_case
    pub column:   String,            // physical column
    pub kind:     FieldKind,
    pub optional: bool,              // `?` in DSL -> Option<T>
    pub default:  Option<DefaultValue>,
    pub attrs:    FieldAttrs,        // id, unique, updated_at, db_generated...
    pub docs:     Option<String>,
    pub span:     Span,
}

/// A field is *either* scalar data *or* a relation. Never both.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum FieldKind {
    Scalar(ScalarType),
    Enum(EnumName),
    /// A navigation property. The FK column lives on whichever side declares
    /// `fields:`; see ImplPlan06.
    Relation(RelationRef),
    /// `Post[]` — the "many" side, has no column of its own.
    List(Box<FieldKind>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ScalarType {
    String, Int, BigInt, Float, Decimal, Boolean,
    DateTime, Date, Time, Uuid, Json, Bytes,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum DefaultValue {
    Literal(Literal),
    /// Function defaults resolved per-dialect: uuid4(), now(), cuid(), autoincrement()
    Function(DefaultFn),
    /// `dbgenerated("...")` — verbatim, dialect's problem
    DbGenerated(String),
}
```

**Design notes worth defending:**

- `IndexMap` not `HashMap` everywhere: **declaration order must be stable** or
  generated code and migration diffs churn on every run. This is not a
  micro-optimisation, it is a correctness requirement for the diff engine.
- `Span` is carried on every node so P1 diagnostics and P6 migration warnings can
  point at the exact source line. Adding spans later is a painful refactor.
- `Schema` is `Serialize`/`Deserialize` because the migration snapshot format
  (P6) *is* the serialized IR. One type, two uses.
- `Model.table` and `Field.column` are resolved at lowering time (applying
  `@@map`/`@map` plus the naming convention), so no downstream crate ever needs
  to know the naming rules.

**Acceptance:** IR types compile; round-trip `serde_json` test passes on a
hand-built `Schema`; rustdoc on every public item.

---

## P0-03 · Diagnostics (`ruprizzle-core::diagnostic`)

**Owner:** Claude · **Est:** 3h

Prisma's best non-technical feature is its error messages. Budget for it up front
rather than retrofitting.

```rust
#[derive(Debug, thiserror::Error, miette::Diagnostic)]
pub enum SchemaError {
    #[error("unknown type `{found}`")]
    #[diagnostic(
        code(ruprizzle::unknown_type),
        help("did you mean `{suggestion}`? Built-in scalars: String, Int, BigInt, \
              Float, Decimal, Boolean, DateTime, Date, Time, Uuid, Json, Bytes")
    )]
    UnknownType {
        found: String,
        suggestion: String,       // Levenshtein-nearest known type
        #[source_code] src: NamedSource<String>,
        #[label("not a known scalar, enum, or model")] span: SourceSpan,
    },

    #[error("model `{model}` has no primary key")]
    #[diagnostic(
        code(ruprizzle::missing_primary_key),
        help("add `@id` to a field, or `@@id([a, b])` for a composite key")
    )]
    MissingPrimaryKey { /* ... */ },
    // ... one variant per validation rule in ImplPlan02
}
```

Rules:
- Every error carries a `#[label]` on the offending span and a `help()` that says
  what to *do*, not just what is wrong.
- Suggestions use Levenshtein distance over the in-scope identifier set.
- Errors accumulate — report **all** schema errors in one pass, never bail on the
  first. Implement as `Vec<SchemaError>` collected by the validator.

**Acceptance:** a schema with 3 distinct mistakes reports all 3, each with a
correct span, in one `ruprizzle generate` run.

---

## P0-04 · Test harness

**Owner:** Devin · **Est:** 5h

Three test tiers, each with a different speed/fidelity trade-off:

| Tier | What | Runs on | Speed |
|---|---|---|---|
| Unit | parser, IR lowering, diff algebra | every `cargo test` | ms |
| Snapshot | generated Rust + generated SQL | every `cargo test` | ms |
| Integration | real Postgres + real SQLite | `--features it` / CI | seconds |

**Snapshot testing** uses `insta`. This is the highest-leverage decision in P0:
codegen output is large and reviewing it by eye does not scale, but `cargo insta
review` makes every codegen change a visible diff.

```rust
// crates/codegen/tests/snapshots.rs
#[test]
fn blog_schema_postgres() {
    let schema = parse_fixture("blog.ruprizzle");
    let out = codegen::emit(&schema, &PostgresDialect);
    insta::assert_snapshot!(out.entities);
    insta::assert_snapshot!(out.query_builders);
}
```

**Integration harness** — each test gets an isolated database, so tests can run
concurrently without interference:

```rust
/// Creates a fresh Postgres schema (or SQLite temp file), applies migrations,
/// hands back a pool, and drops everything on Drop.
pub struct TestDb { /* ... */ }

impl TestDb {
    pub async fn postgres(schema_src: &str) -> Self { /* CREATE SCHEMA rz_<uuid> */ }
    pub async fn sqlite(schema_src: &str)   -> Self { /* file in tempdir */ }
}
```

**The dual-database rule:** every integration test is written once, over a
`TestDb`, and run against both backends via a macro:

```rust
both_dbs!(async fn insert_then_select(db: TestDb) { /* ... */ });
```

This is what keeps SQLite from silently rotting while Postgres gets all the
attention — the RealityCheck doc flagged exactly this failure mode.

`docker-compose.yml` provides `postgres:17`. SQLite needs no service.

**Acceptance:** `both_dbs!` macro works; one trivial test passes on both engines;
`cargo insta test` wired into CI.

---

## P0-05 · CI pipeline

**Owner:** Devin · **Est:** 3h

`.github/workflows/ci.yml`:

| Job | Command | Blocking |
|---|---|---|
| fmt | `cargo fmt --all --check` | yes |
| clippy | `cargo clippy --workspace --all-targets -- -D warnings` | yes |
| test | `cargo test --workspace` | yes |
| integration | `cargo test --workspace --features it` (postgres service) | yes |
| generated-code-lint | generate examples, then clippy **the generated crate** | yes |
| msrv | build on pinned `rust-version` | yes |
| docs | `cargo doc --no-deps -D warnings` | yes |

The `generated-code-lint` job is unusual and important: our output is other
people's source code. If it emits a warning, that is our bug, and it must fail our
build, not theirs.

**Acceptance:** all jobs green on an empty-but-valid workspace.

---

## Phase P0 checklist

- [ ] P0-01 workspace builds
- [ ] P0-02 IR defined, serde round-trips, fully documented
- [ ] P0-03 diagnostics render with spans via `miette`
- [ ] P0-04 `both_dbs!` + `insta` harness working
- [ ] P0-05 seven CI jobs green
- [ ] Decision log started in ImplPlan10 for any deviation from this file
