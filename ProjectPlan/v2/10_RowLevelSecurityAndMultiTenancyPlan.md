# Plan 10: Row-Level Security (RLS) & Multi-Tenant Primitives

**Date:** 2026-08-22  
**Author:** Vaibhav Gupta <vaibhavgupta9877@gmail.com>  
**Status:** Ready for Execution  
**Milestone:** v2.2.0-rc.1  
**Primary Crates:** `crates/core`, `crates/parser`, `crates/dialect`, `crates/migrate`, `crates/runtime`

---

## 1. Context, Objectives & Scope

Building multi-tenant B2B SaaS applications requires ironclad tenant data isolation. Forcing developers to manually append `.where(tenant_id.eq(current_tenant))` to every single application query is error-prone and a leading cause of data leakage vulnerabilities.

In v2, `ruprizzle` delivers **declarative Multi-Tenancy & Row-Level Security (RLS)**:
1. **Schema DSL Directives:** Declarative `@@tenant(field)` and `@@policy(op, expr)` attributes in `schema.ruprizzle`.
2. **Native Postgres RLS DDL:** Automatic generation of `ENABLE ROW LEVEL SECURITY` and `CREATE POLICY` statements in the migration engine.
3. **Transparent Query Scoping on SQLite & MySQL:** Automatically injects tenant predicates into query ASTs on engines lacking native RLS.
4. **Ergonomic Tenant Context API:** `pool.with_tenant("org_123")` handles session variables and query scoping automatically.

---

## 2. Technical Architecture & Specification

### 2.1 Schema DSL Definition

```ruprizzle
model Document {
  id        String   @id @default(uuid())
  tenantId  String
  title     String
  content   String
  createdAt DateTime @default(now())

  // Declares tenantId as the tenant isolation key
  @@tenant(tenantId)

  // Declarative security policies for Postgres RLS:
  @@policy(read, "tenant_id = current_setting('app.current_tenant', true)")
  @@policy(write, "tenant_id = current_setting('app.current_tenant', true)")
}
```

---

### 2.2 Core IR & AST Extensions (`crates/core`, `crates/parser`)

```rust
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Model {
    // Existing fields...
    pub tenant_field: Option<FieldName>,
    pub policies: Vec<SecurityPolicy>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SecurityPolicy {
    pub name: String,
    pub operation: PolicyOperation,
    pub expression: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PolicyOperation {
    All,
    Select,
    Insert,
    Update,
    Delete,
}
```

---

### 2.3 Dialect DDL & Query Translation

#### PostgreSQL (Native RLS):
```sql
ALTER TABLE "Document" ENABLE ROW LEVEL SECURITY;
ALTER TABLE "Document" FORCE ROW LEVEL SECURITY;

CREATE POLICY "document_read_policy" ON "Document"
    FOR SELECT
    USING (tenant_id = current_setting('app.current_tenant', true));

CREATE POLICY "document_write_policy" ON "Document"
    FOR ALL
    USING (tenant_id = current_setting('app.current_tenant', true));
```

#### SQLite & MySQL (Transparent AST Query Transformation):
On dialects without native RLS, `ruprizzle` compiler automatically inspects `Model::tenant_field` and injects `AND tenant_id = ?` into the WHERE clause of all generated `SELECT`, `UPDATE`, and `DELETE` queries, and populates `tenant_id` on `INSERT`.

---

### 2.4 Runtime Tenant Context API (`crates/runtime`)

```rust
// Create tenant-scoped handle:
let tenant_db = pool.with_tenant("org_corp_987");

// 1. SELECT automatically isolated to 'org_corp_987'
let docs = Document::find_many().all(&tenant_db).await?;

// 2. INSERT automatically sets tenantId = 'org_corp_987'
let new_doc = Document::create()
    .title("Q4 Report")
    .content("Revenue numbers...")
    .save(&tenant_db)
    .await?;
```

---

## 3. Step-by-Step Implementation Tasks

### Task 1: Grammar & AST Lowering
- [ ] In `crates/parser/src/schema.pest`:
  - Add grammar for `@@tenant(ident)` and `@@policy(op, string)`.
- [ ] In `crates/core/src/ir.rs` & `crates/parser/src/lower.rs`:
  - Lower tenant field and security policies into `Model`.
  - Validate that `tenant_field` references an existing scalar field on the model.

### Task 2: Dialect DDL & Migration Plan
- [ ] In `crates/dialect/src/postgres.rs`:
  - Emit `ENABLE ROW LEVEL SECURITY` and `CREATE POLICY` statements.
- [ ] In `crates/migrate/src/diff.rs`:
  - Detect changes to security policies and generate migration steps.

### Task 3: Runtime Context & Query Rewriter
- [ ] In `crates/runtime/src/pool.rs`:
  - Implement `TenantScopedPool` wrapper carrying `tenant_id: String`.
- [ ] In `crates/runtime/src/compile.rs`:
  - On PostgreSQL: execute `SET LOCAL app.current_tenant = $1` at the start of connection checkout.
  - On SQLite/MySQL: automatically append tenant filter condition to query AST.

### Task 4: Integration & Security Leak Testing
- [ ] Add `crates/runtime/tests/tenant_isolation_test.rs`:
  - Test that tenant A cannot read or modify tenant B records under any query pattern.
  - Verify PostgreSQL RLS enforcement and SQLite AST rewriting.

---

## 4. Verification & Testing Strategy

```powershell
# 1. Run multi-tenancy unit and integration tests
cargo test -p ruprizzle --test tenant_isolation_test

# 2. Test migration DDL generation for policies
cargo test -p ruprizzle-migrate --test policy_migration_test

# 3. Mechanical gates
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
```

---

## 5. Definition of Done

1. `schema.ruprizzle` supports declarative `@@tenant` and `@@policy` attributes.
2. Migration engine enables Postgres RLS and generates SQL policies.
3. `pool.with_tenant(...)` provides seamless, zero-leak tenant isolation across Postgres, SQLite, and MySQL.
4. Comprehensive multi-tenant security test suite passes with 0 failures.
