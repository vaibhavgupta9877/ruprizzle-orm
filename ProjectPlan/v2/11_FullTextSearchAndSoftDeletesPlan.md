# Plan 11: Full-Text Search, Soft Deletes & Audit Trails

**Date:** 2026-08-22  
**Author:** Vaibhav Gupta <vaibhavgupta9877@gmail.com>  
**Status:** Ready for Execution  
**Milestone:** v1.1.0 (Additive, Minor Release)  
**Primary Crates:** `crates/core`, `crates/parser`, `crates/dialect`, `crates/runtime`, `crates/migrate`

---

## 1. Context, Objectives & Scope

In `docs/KnownLimitations.md`, Full-Text Search (FTS) and Soft Deletes were listed as deferred. Adding them in **v1.1** provides immediate developer leverage for typical web application requirements (search bars, trash recovery, audit logging) with 100% backwards compatibility.

### Key Capabilities
1. **Full-Text Search (FTS):**
   - **PostgreSQL:** Native `tsvector` and GIN index generation (`@@index([title, content], type: Gin)`), with `.matches(query)` generating `to_tsvector(...) @@ plainto_tsquery(...)` with ranking `.with_rank()`.
   - **SQLite:** Seamless FTS5 virtual table synchronization or `MATCH` expression compilation.
   - **MySQL:** `FULLTEXT` index generation and `MATCH(...) AGAINST(...)` query operators.
2. **Declarative Soft Deletes (`@deletedAt`):**
   - Directives `@deletedAt` on `DateTime?` fields.
   - Automatically injects `WHERE deleted_at IS NULL` on all `find_many()`, `find_unique()`, and relation joins.
   - Bypass escape hatch: `.with_deleted()` or `.only_deleted()`.
   - Method `.soft_delete()` sets `deleted_at = now()`.
3. **Automatic Audit Timestamps (`@createdAt`, `@updatedAt`):**
   - Automatically populates current UTC timestamp on insert and update.

---

## 2. Technical Architecture & DSL Specification

### 2.1 Schema DSL Extensions

```ruprizzle
model Article {
  id        String    @id @default(uuid())
  title     String
  body      String
  published Boolean   @default(false)
  
  // Audit timestamps
  createdAt DateTime  @default(now())
  updatedAt DateTime  @updatedAt
  deletedAt DateTime? @deletedAt

  // Full-text search index definition:
  @@index([title, body], type: FullText)
}
```

---

### 2.2 Query Builder API (`crates/runtime`)

```rust
// 1. Full-Text Search with Relevance Ranking
let articles: Vec<Article> = Article::find_many()
    .where(Article::title.matches("rust performance"))
    .order_by(Article::search_rank_desc("rust performance"))
    .limit(10)
    .all(&pool)
    .await?;

// 2. Soft Delete Execution
Article::update()
    .where(Article::id.eq("art_123"))
    .soft_delete(&pool)
    .await?;

// 3. Querying with Soft Deletes (default excludes deleted records)
let active_articles = Article::find_many().all(&pool).await?; // WHERE deleted_at IS NULL

// 4. Including Soft Deleted Records
let all_articles = Article::find_many().with_deleted().all(&pool).await?;

// 5. Querying Only Soft Deleted Records (Trash Bin)
let trashed_articles = Article::find_many().only_deleted().all(&pool).await?;
```

---

## 3. Step-by-Step Implementation Tasks

### Task 1: Parser Grammar & Core IR
- [ ] In `crates/parser/src/schema.pest`:
  - Add `@updatedAt` and `@deletedAt` attribute parsing.
  - Add `type: FullText` to index attributes.
- [ ] In `crates/core/src/ir.rs`:
  - Add `FieldAttrs::is_updated_at` and `FieldAttrs::is_deleted_at`.

### Task 2: Dialect DDL & Migration Engine
- [ ] In `crates/dialect/src/postgres.rs`:
  - Emit GIN indexes on `to_tsvector('english', ...)` for FullText indexes.
- [ ] In `crates/dialect/src/mysql.rs`:
  - Emit `FULLTEXT INDEX` DDL.
- [ ] In `crates/dialect/src/sqlite.rs`:
  - Support SQLite FTS5 table generation or LIKE search fallback.

### Task 3: Runtime Query Compilation & Filter Scoping
- [ ] In `crates/runtime/src/compile.rs`:
  - Implement automatic `deleted_at IS NULL` predicate injection on models with `@deletedAt`.
  - Add `.with_deleted()` and `.only_deleted()` state flags to `SelectQuery`.
  - Implement `.matches(text)` compilation to dialect-specific FTS SQL.

### Task 4: Integration & Dialect Conformance Tests
- [ ] Add `crates/runtime/tests/fts_soft_delete_test.rs`:
  - Test FTS querying on PostgreSQL, SQLite, and MySQL.
  - Test soft-delete scoping, `.with_deleted()`, and `.only_deleted()`.

---

## 4. Verification & Testing Strategy

```powershell
# 1. Run FTS & Soft Delete tests
cargo test -p ruprizzle --test fts_soft_delete_test

# 2. Migration engine index diff tests
cargo test -p ruprizzle-migrate --test fts_migration_test

# 3. Mechanical gates
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
```

---

## 5. Definition of Done

1. Models with `@deletedAt` automatically filter soft-deleted records unless explicitly overridden with `.with_deleted()`.
2. Full-Text Search `.matches()` generates native high-performance search queries across PostgreSQL, MySQL, and SQLite.
3. 100% green tests across all three supported dialects.
