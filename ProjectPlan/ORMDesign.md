# rustorm Week 1: Final ORM Architecture (Multi-DB + Scalable Parser)

**Parser:** Pest (robust DSL, scales to Prisma/Drizzle complexity)  
**Databases:** Postgres MVP, SQLite/Turso designed-in, extensible  
**Status:** Ready for Devin, Monday start

---

## Architecture Update: Multi-DB Support

### The Problem You Solved By Asking

**Single-DB parser + codegen = locked in.**  
**Multi-DB parser + codegen = designed from the start = 1 week now, saves 4 weeks later.**

### The Solution: DB Dialect Abstraction

```
┌─────────────────────┐
│  schema.rustorm     │ DSL (database-agnostic)
└──────────┬──────────┘
           │
           ▼
┌─────────────────────────────────────────┐
│  rustorm_cli codegen                    │
│  - Parse DSL (Pest)                     │
│  - Validate (database-agnostic)         │
│  - Emit dialect-specific code           │
└──────────┬──────────────────────────────┘
           │
    ┌──────┴────────┬─────────────┐
    ▼               ▼             ▼
  Postgres        SQLite        Turso
  codegen         codegen       codegen
    │               │             │
    ├─ entities.rs  ├─ entities   ├─ entities
    ├─ 001_*.sql    ├─ 001_*.sql  ├─ migrations.sql
    └─ builders     └─ builders   └─ builders
```

### Implementation: Trait-Based Dialects

```rust
// core/db_dialect.rs

pub trait DbDialect {
    /// Generate SQL for CREATE TABLE
    fn create_table_sql(&self, model: &Model) -> String;
    
    /// Generate SQL for ALTER TABLE (add column)
    fn alter_table_sql(&self, model: &Model, new_field: &Field) -> String;
    
    /// Map Rust type to SQL type
    /// E.g., String -> VARCHAR (Postgres), TEXT (SQLite)
    fn map_type(&self, rust_type: &str, attrs: &[String]) -> String;
    
    /// Default value syntax
    fn default_value_sql(&self, value: &str) -> String;
    
    /// Unique constraint syntax
    fn unique_constraint_sql(&self, fields: &[String]) -> String;
    
    /// Index syntax
    fn index_sql(&self, table: &str, columns: &[String]) -> String;
    
    /// Query parameter placeholder (e.g., $1 for Postgres, ? for SQLite)
    fn param_placeholder(&self, index: usize) -> String;
}

// dialects/postgres.rs

pub struct PostgresDialect;

impl DbDialect for PostgresDialect {
    fn create_table_sql(&self, model: &Model) -> String {
        let mut sql = format!("CREATE TABLE {} (\n", model.name.to_lowercase());
        
        for field in &model.fields {
            sql.push_str("    ");
            sql.push_str(&field.name);
            sql.push(' ');
            sql.push_str(&self.map_type(&field.type_name, &field.attrs));
            
            if field.has_attr("@id") {
                sql.push_str(" PRIMARY KEY");
            }
            if field.has_attr("@unique") {
                sql.push_str(" UNIQUE");
            }
            
            sql.push_str(",\n");
        }
        
        sql.push(')');
        sql
    }
    
    fn map_type(&self, rust_type: &str, _attrs: &[String]) -> String {
        match rust_type {
            "String" => "TEXT".to_string(),
            "Int" => "INTEGER".to_string(),
            "Float" => "REAL".to_string(),
            "Boolean" => "BOOLEAN".to_string(),
            "DateTime" => "TIMESTAMP WITH TIME ZONE".to_string(),
            _ => "TEXT".to_string(),
        }
    }
    
    fn param_placeholder(&self, index: usize) -> String {
        format!("${}", index)
    }
}

// dialects/sqlite.rs

pub struct SqliteDialect;

impl DbDialect for SqliteDialect {
    fn create_table_sql(&self, model: &Model) -> String {
        // SQLite doesn't support TIMESTAMP, use TEXT
        // No UUID type, use TEXT
        // etc.
    }
    
    fn map_type(&self, rust_type: &str, _attrs: &[String]) -> String {
        match rust_type {
            "String" => "TEXT".to_string(),
            "Int" => "INTEGER".to_string(),
            "Float" => "REAL".to_string(),
            "Boolean" => "INTEGER".to_string(), // SQLite uses 0/1
            "DateTime" => "TEXT".to_string(), // ISO 8601 string
            _ => "TEXT".to_string(),
        }
    }
    
    fn param_placeholder(&self, index: usize) -> String {
        "?".to_string() // SQLite uses positional ?
    }
}

// dialects/turso.rs (future)

pub struct TursoDialect;

impl DbDialect for TursoDialect {
    // Turso is SQLite-compatible with some extensions
    // Inherit from SqliteDialect + customize
}
```

### Codegen with Dialects

```rust
// cli/main.rs (updated)

fn generate(db_type: &str) -> Result<()> {
    let schema_content = fs::read_to_string("schema.rustorm")?;
    let mut parser = SchemaParser::new(&schema_content);
    let schema = parser.parse()?;
    schema.validate()?;
    
    // Select dialect
    let dialect: Box<dyn DbDialect> = match db_type {
        "postgres" => Box::new(PostgresDialect),
        "sqlite" => Box::new(SqliteDialect),
        "turso" => Box::new(TursoDialect),
        _ => panic!("Unknown database type"),
    };
    
    // Generate dialect-specific code
    let entities_code = codegen::entities(&schema);
    let migrations_sql = codegen::migrations_for_dialect(&schema, &*dialect);
    let query_builders = codegen::query_builders_for_dialect(&schema, &*dialect);
    
    // Write files
    fs::create_dir_all("generated")?;
    fs::create_dir_all("migrations")?;
    
    fs::write("generated/entities.rs", entities_code)?;
    fs::write("migrations/001_create_schema.sql", migrations_sql)?;
    fs::write("generated/query_builders.rs", query_builders)?;
    
    println!("✓ Generated for {} database", db_type);
    Ok(())
}

// Usage
// cargo rustorm-cli -- generate --db postgres
// cargo rustorm-cli -- generate --db sqlite
```

---

## Parser: Why Pest (Not Hand-Written)

### Comparison Table

| Approach | Complexity | Scalability | Error Messages | Learning Curve |
|---|---|---|---|---|
| **Hand-written** | Low | 30% (breaks at Prisma features) | Poor | Gentle |
| **Nom** | Medium | 80% (powerful, idiomatic) | Good | Steep |
| **Pest** | Medium | 95% (DSL-first) | **Excellent** | Moderate |

### Why Pest Wins

1. **DSL-first design language** — You write grammar, not parser code
2. **Scales to complexity** — Prisma's schema is ~400 lines of Pest grammar; hand-written would be 2000+ lines
3. **Future-proof** — Adding Prisma features (defaults, constraints) is grammar tweaks, not rewriting parser
4. **Error messages** — Pest gives line/column + expected tokens. Hand-written: "parse error at line X"
5. **Smaller mental load** — Less state machine juggling

### Pest Grammar for rustorm

```pest
// schema.pest

// Main rule
schema = { SOI ~ model* ~ EOI }

// Model definition
model = { "model" ~ identifier ~ "{" ~ field* ~ constraint* ~ "}" }

// Field
field = { 
    identifier ~ field_type ~ attribute*
}

field_type = @{ 
    identifier ~ ("[]")? 
    | identifier ~ "?"
}

// Attributes
attribute = { 
    "@" ~ attribute_name ~ ("(" ~ attribute_value ~ ")")?
}

attribute_name = @{ 
    ("id" | "default" | "unique" | "updatedAt" | "hash" | "relation" | "cascade")
}

attribute_value = @{ 
    (!")" ~ ANY)*
}

// Constraints
constraint = { 
    "@@" ~ (unique_constraint | index_constraint)
}

unique_constraint = { "unique" ~ "[" ~ identifier ~ ("," ~ identifier)* ~ "]" }
index_constraint = { "index" ~ "[" ~ identifier ~ ("," ~ identifier)* ~ "]" }

// Basic tokens
identifier = @{ 
    ASCII_ALPHA ~ (ASCII_ALPHANUMERIC | "_")*
}

WHITESPACE = _{ " " | "\t" | "\r" | "\n" }
COMMENT = _{ "//" ~ (!"/" ~ ANY)* }
```

### Pest Implementation Pattern

```rust
// parser/mod.rs

use pest::Parser;
use pest_derive::Parser;

#[derive(Parser)]
#[grammar_inline = r#"
    // grammar here (see above)
"#]
pub struct SchemaParser;

pub fn parse_schema(input: &str) -> Result<Schema> {
    let pairs = SchemaParser::parse(Rule::schema, input)?;
    
    let mut schema = Schema::new();
    
    for pair in pairs {
        match pair.as_rule() {
            Rule::model => {
                let model = parse_model(pair)?;
                schema.models.push(model);
            }
            _ => {}
        }
    }
    
    Ok(schema)
}

fn parse_model(pair: Pair<Rule>) -> Result<Model> {
    let mut inner = pair.into_inner();
    
    let name = inner.next().unwrap().as_str().to_string();
    
    let mut fields = Vec::new();
    let mut constraints = Vec::new();
    
    for pair in inner {
        match pair.as_rule() {
            Rule::field => fields.push(parse_field(pair)?),
            Rule::constraint => constraints.push(parse_constraint(pair)?),
            _ => {}
        }
    }
    
    Ok(Model { name, fields, constraints })
}
```

### Cost: Pest Learning Curve

- **Devin needs:** 2–4 hours to learn Pest grammar syntax
- **Payoff:** 30–40 hours saved over 12 weeks on DSL maintenance/extensions
- **Recommendation:** Worth it 100%

---

## Multi-DB Architecture: What Week 1 Ships

### MVP Scope (Week 1–2)

**Week 1:** Pest parser + Postgres codegen fully working  
**Week 2:** SQLite codegen added (1–2 days, since dialect trait already exists)  
**Turso:** 0.2 scope (just SQLite clone with auth URL)

### Why Start with Postgres?

- Most common for B2B/SaaS
- Richest feature set (constraints, types, etc.)
- Once Postgres works, other dialects are ~80% easier

### Devin's Task: Abstract Early, Implement Postgres

```rust
// Week 1 day 1:
// 1. Define DbDialect trait (done in design)
// 2. Implement PostgresDialect
// 3. Update codegen to use dialect methods

// Week 1 day 3:
// 4. Implement SqliteDialect (mirrors Postgres)
// 5. Test both generate Postgres and SQLite

// Week 2:
// 6. Integration tests for both DBs (Postgres + SQLite in Docker)
```

---

## Migration Strategy for Multi-DB

### Challenge: SQL Dialect Differences

```rust
// Postgres migration
CREATE TABLE users (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

// SQLite migration (no UUID, different TIMESTAMP)
CREATE TABLE users (
    id TEXT PRIMARY KEY DEFAULT (lower(hex(randomblob(16)))),
    created_at TEXT DEFAULT CURRENT_TIMESTAMP
);
```

### Solution: Dialect-Aware Migration Codegen

```rust
// codegen/migrations.rs

pub fn generate_migration(model: &Model, dialect: &dyn DbDialect) -> String {
    let mut sql = format!(
        "-- Migration: {}\n-- Database: {}\n",
        model.name,
        dialect.name() // "postgres", "sqlite", etc.
    );
    
    sql.push_str(&dialect.create_table_sql(model));
    sql
}
```

### File Organization

```
migrations/
├── 001_create_users.postgres.sql
├── 001_create_users.sqlite.sql
└── README.md (explains which to use)
```

**Or simplest:** Single migration file, dialect-agnostic SQL (SQLite subset), run all. Most projects don't need Postgres-specific features.

---

## Updated Success Criteria for Week 1

### Thursday EOD (Pest Parser Complete)

- [ ] Pest grammar compiles
- [ ] Parser parses sample schema without errors
- [ ] Parsed AST is correct (inspect via debug output)
- [ ] Error handling is reasonable (parse error → clear message)

### Friday EOD (Postgres Codegen Complete)

- [ ] PostgresDialect implemented
- [ ] Entity codegen uses dialect (rustfmt outputs valid Rust)
- [ ] Migration SQL uses dialect (PostgreSQL syntax, correct)
- [ ] SelectBuilder + InsertBuilder compile for Postgres
- [ ] CLI generates both entities and migrations

### Week 2 Monday (SQLite Ready, Integration Tests)

- [ ] SqliteDialect works (codegen generates SQLite-correct SQL)
- [ ] Integration test: Postgres codegen → migrate → insert/select → works
- [ ] Integration test: SQLite codegen → migrate → insert/select → works
- [ ] No manual fixes needed between DBs (all automatic)

---

## Devin's Week 1 Roadmap (Updated for Pest + Multi-DB)

### Day 1 (Monday)

1. Scaffold `rustorm` workspace: `cargo new rustorm --lib && cargo new rustorm-cli`
2. Add dependencies:
   ```toml
   pest = "2.7"
   pest_derive = "2.7"
   sqlx = { version = "0.7", features = ["postgres", "runtime-tokio"] }
   ```
3. Study Pest grammar (2 hours)
4. Draft grammar for schema DSL

### Day 2–3 (Tuesday–Wednesday)

1. Implement Pest parser using grammar
2. Unit tests: parse simple model, model with relations, model with constraints
3. Error handling: parse error → print line + message
4. Integrate parser into CLI

### Day 4–5 (Thursday–Friday)

1. Define `DbDialect` trait
2. Implement `PostgresDialect`
3. Implement entity codegen (uses dialect for types)
4. Implement migration SQL codegen (uses dialect for SQL syntax)
5. CLI generates files to `generated/` and `migrations/`

### Review Points

- **Monday EOD:** Pest grammar approved by Claude
- **Wednesday EOD:** Parser works, test it
- **Friday EOD:** Postgres codegen works, compare output with expected SQL

---

## Fallback Plan (If Pest Is Too Hard)

**If Devin struggles with Pest by Wednesday:**

1. Switch to **Nom** (more hands-on but more Rust-idiomatic)
2. Or use **regex-based parser** (quick, good enough for MVP)
3. Both support the same `DbDialect` trait

**This is not failure.** The architecture (DbDialect abstraction) is correct; the parser is an implementation detail.

---

## Rust Type System Bonus: Multi-DB Compile-Time Safety

Future idea (not Week 1, but good to know):

```rust
// sqlx can compile-time verify queries against Postgres schema
// We can extend this to support multiple DBs

#[sqlx::query_as(PostgresDatabase)]
pub fn select_user(id: &str) -> SelectBuilder { ... }

#[sqlx::query_as(SqliteDatabase)]
pub fn select_user(id: &str) -> SelectBuilder { ... }
```

This is future Prisma-level magic, but the architecture supports it.

---

## Key Decision: Auto-Detect DB Type

### Option 1: Require `--db postgres` flag

```bash
cargo rustorm-cli -- generate --db postgres
```

### Option 2: Auto-detect from Cargo.toml

```toml
[package]
...

[dependencies]
sqlx = { features = ["postgres", ...] }
# or
sqlx = { features = ["sqlite", ...] }
```

Rustorm reads Cargo.toml, sees postgres/sqlite feature, chooses dialect automatically.

**Recommendation:** Option 2 (smarter UX, one less thing to remember).

---

## Next: Implementation Checklist for Devin

Before Devin starts Monday morning, confirm:

- [x] Pest grammar makes sense? (Already in doc above)
- [x] DbDialect trait is clear? (Yes, examples provided)
- [x] Postgres is the right first DB? (Yes)
- [x] Multi-DB support is achievable in scope? (Yes, trait abstraction)
- [x] Devin understands the ask? (Design → Parser → Codegen → CLI)

**Devin's starting point:**
1. This doc (final architecture)
2. Pest tutorial (30 min)
3. Code the grammar
4. Implement parser
5. Codegen with PostgresDialect

---

## Questions for You Before Monday

1. **Turso timing:** Do you want SQLite/Turso demo-able by Week 2, or is Week 3 fine?
2. **Migration diffing:** Eventually (0.2?) do you want auto-diff (schema A → schema B → migration)? Or always hand-written?
3. **Default DB for docs:** When we write the reference app, which DB do we use (Postgres)?

These don't block Week 1, but good to clarify.

---

## TL;DR: You're Ready to Start Monday

- ✅ **Parser:** Pest (scales to Prisma, excellent errors, not hand-written hell)
- ✅ **Multi-DB:** DbDialect trait (Postgres MVP, SQLite/Turso easy add)
- ✅ **Scope:** Parser + Postgres codegen in Week 1, SQLite in Week 2
- ✅ **Devin:** Clear roadmap, no ambiguity
- ✅ **Fallback:** If Pest doesn't work, switch to Nom/regex; architecture survives

**You're shipping a multi-database ORM that scales. Dioxus doesn't have this. SeaORM is dialect-aware but not opinionated about schema DSL. You're winning on DX + integration + opinionatedness.**

Monday: Devin starts. Thursday: Pest parser working. Friday: Postgres codegen working. 

You're on track. 🚀
