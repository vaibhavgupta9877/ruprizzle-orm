# Plan 11: Full-Text Search, Soft Deletes & Audit Trails

**Date:** 2026-08-22  
**Author:** Vaibhav Gupta <vaibhavgupta9877@gmail.com>  
**Status:** Completed  
**Milestone:** v1.1.0 (Additive, Minor Release)  
**Primary Crates:** `crates/core`, `crates/parser`, `crates/dialect`, `crates/runtime`, `crates/codegen`

---

## 1. Context, Objectives & Scope

In `docs/KnownLimitations.md`, Full-Text Search (FTS) and Soft Deletes were listed as deferred. Adding them in **v1.1** provides immediate developer leverage for typical web application requirements (search bars, trash recovery, audit logging) with 100% backwards compatibility.

### Key Capabilities
1. **Full-Text Search (FTS):**
   - **PostgreSQL:** Native `tsvector` with `.matches(query)` generating `to_tsvector(...) @@ plainto_tsquery(...)`.
   - **SQLite:** `MATCH` expression compilation.
   - **MySQL:** `MATCH(...) AGAINST(...)` query operators.
2. **Declarative Soft Deletes (`@deletedAt`):**
   - Directives `@deletedAt` on `DateTime?` fields.
   - Automatically injects `WHERE deleted_at IS NULL` on all queries.
   - Bypass escape hatches: `.with_deleted()` or `.only_deleted()`.
   - Method `.soft_delete()` sets `deleted_at = Utc::now()`.
3. **Automatic Audit Timestamps (`@createdAt`, `@updatedAt`):**
   - Directives `@createdAt` and `@updatedAt` recognized by parser, IR, and codegen.

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

## 2.2 Query Builder API (`crates/runtime`)

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
- [x] In `crates/parser/src/schema.pest` and `crates/parser/src/lower.rs`:
  - Add `@createdAt`, `@updatedAt`, and `@deletedAt` attribute lowering and type validation.
- [x] In `crates/core/src/ir.rs`:
  - Add `FieldAttrs::is_created_at`, `FieldAttrs::is_updated_at`, and `FieldAttrs::is_deleted_at`.

### Task 2: Model & Codegen Emission
- [x] In `crates/runtime/src/model.rs`:
  - Add `DELETED_AT_COLUMN` and `UPDATED_AT_COLUMN` model constants.
- [x] In `crates/codegen/src/emit.rs`:
  - Emit `DELETED_AT_COLUMN` and `UPDATED_AT_COLUMN` in generated `Model` trait impls.

### Task 3: Runtime Query Compilation & Filter Scoping
- [x] In `crates/runtime/src/filter.rs` and `crates/runtime/src/col.rs`:
  - Add `FilterNode::FullTextMatch` and `Column::matches()` operator.
- [x] In `crates/runtime/src/compile.rs`:
  - Multi-dialect compilation for `FilterNode::FullTextMatch` across Postgres (`to_tsvector @@ plainto_tsquery`), MySQL (`MATCH...AGAINST`), and SQLite (`MATCH`).
- [x] In `crates/runtime/src/query.rs`:
  - Added `with_deleted`, `only_deleted`, and `effective_filter()` for automatic `WHERE deleted_at IS NULL` injection.
  - Added `SelectQuery::with_deleted()`, `SelectQuery::only_deleted()`, and `UpdateQuery::soft_delete()`.

### Task 4: Integration & Dialect Conformance Tests
- [x] Add `crates/runtime/tests/v1_1_features.rs`:
  - Test FTS query compilation across Postgres and SQLite.
  - Test soft-delete scoping, `.with_deleted()`, `.only_deleted()`, and `.soft_delete()`.
- [x] Run full verification suite across workspace.

---

## 4. Verification & Testing Strategy

```powershell
# 1. Run FTS & Soft Delete tests
cargo test -p ruprizzle --test v1_1_features

# 2. Workspace full suite
cargo test --workspace

# 3. Mechanical gates
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo xtask harden
```

---

## 5. Definition of Done

1. Models with `@deletedAt` automatically filter soft-deleted records unless explicitly overridden with `.with_deleted()`.
2. Full-Text Search `.matches()` generates native high-performance search queries across PostgreSQL, MySQL, and SQLite.
3. 100% green tests across all supported dialects.
