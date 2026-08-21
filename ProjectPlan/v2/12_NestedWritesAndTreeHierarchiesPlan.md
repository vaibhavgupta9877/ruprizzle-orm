# Plan 12: Advanced Relations, Tree Hierarchies & Nested Writes

**Date:** 2026-08-22  
**Author:** Vaibhav Gupta <vaibhavgupta9877@gmail.com>  
**Status:** Ready for Execution  
**Milestone:** v1.3.0 (Additive, Minor Release)  
**Primary Crates:** `crates/core`, `crates/parser`, `crates/codegen`, `crates/runtime`, `crates/migrate`

---

## 1. Context, Objectives & Scope

Relational ergonomics distinguish modern ORMs. Developers migrating from Prisma and Drizzle expect intuitive nested mutations (`create`, `connect`, `connectOrCreate`) and simplified many-to-many relationships without manual join boilerplate. Furthermore, hierarchical tree structures (organizational charts, category taxonomies, threaded comments) require first-class recursive query support.

### Key Capabilities in v1.3
1. **Implicit Many-to-Many Join Tables:** Support `tags Tag[]` without manually declaring an intermediate join model in `schema.ruprizzle`, while seamlessly preserving full compatibility with explicit join models (ADR-006).
2. **Nested Relational Mutations 2.0:** Atomic, transaction-wrapped mutations for nested graphs:
   - `create`: Insert parent and nested children in a single builder invocation.
   - `connect`: Link existing records by primary key or unique fields.
   - `connect_or_create`: Link existing or insert new record atomically.
   - `disconnect` / `delete`: Unlink or delete related entities.
   - `set`: Replace entire relation collection in one call.
3. **Tree & Hierarchy Query Helpers:** Declarative recursive CTE operations on self-referential relations (`.ancestors()`, `.descendants()`, `.tree()`, cycle detection).
4. **Polymorphic Relations & Single Table Inheritance (STI):** Support for discriminator-column inheritance patterns.

---

## 2. Technical Architecture & DSL Specification

### 2.1 Implicit Many-to-Many Syntax

```ruprizzle
model Post {
  id    String @id @default(uuid())
  title String
  tags  Tag[]  // Implicit join table `_PostToTag` generated automatically
}

model Tag {
  id    String @id @default(uuid())
  name  String @unique
  posts Post[]
}
```

---

### 2.2 Nested Writes Query Builder API

```rust
// Create User with nested Profile and initial Posts in a single atomic transaction:
let new_user = User::create()
    .email("alex@example.com")
    .name("Alex")
    .profile(Profile::nested_create().bio("Rust Developer"))
    .posts(vec![
        Post::nested_create().title("Getting Started with Ruprizzle"),
        Post::nested_connect_or_create(
            Post::id.eq("post_existing"),
            Post::nested_create().title("Rust Web Development"),
        ),
    ])
    .save(&pool)
    .await?;

// Update User: attach existing tags and disconnect old ones
User::update()
    .where(User::id.eq("usr_123"))
    .tags_set(vec!["tag_rust", "tag_database"])
    .save(&pool)
    .await?;
```

---

### 2.3 Tree Hierarchy Query Helpers

```ruprizzle
model Category {
  id       String     @id @default(uuid())
  name     String
  parentId String?
  parent   Category?  @relation("CategoryHierarchy", fields: [parentId], references: [id])
  children Category[] @relation("CategoryHierarchy")
}
```

```rust
// 1. Fetch all ancestor categories up to the root
let breadcrumbs: Vec<Category> = Category::ancestors("cat_laptops")
    .order_by_depth_asc()
    .all(&pool)
    .await?;

// 2. Fetch full subtree of descendant categories
let subtree: Vec<Category> = Category::descendants("cat_electronics")
    .max_depth(5)
    .all(&pool)
    .await?;

// 3. Load entire nested hierarchy tree structure
let tree: HierarchyNode<Category> = Category::tree_from_root("cat_root", &pool).await?;
```

---

## 3. Step-by-Step Implementation Tasks

### Task 1: Implicit M2M IR Lowering & Migration Engine
- [ ] In `crates/parser/src/lower.rs`:
  - Detect dual list-relation fields between models without an explicit `through` attribute.
  - Automatically synthesize an internal join model (`_ModelAToModelB`) with compound primary key and cascading foreign keys.
- [ ] In `crates/migrate/src/diff.rs`:
  - Generate DDL for implicit join tables.

### Task 2: Codegen for Nested Write Builders
- [ ] In `crates/codegen/src/emit.rs`:
  - Generate `nested_create`, `nested_connect`, `nested_connect_or_create`, and `nested_disconnect` builders for relation fields.
  - Generate transaction coordinator ensuring all nested operations commit or rollback atomically.

### Task 3: Recursive CTE Hierarchy Runtime Helpers
- [ ] In `crates/runtime/src/query.rs` & `compile.rs`:
  - Implement `.ancestors()` and `.descendants()` recursive CTE generators with depth counters and loop prevention.
  - Add `HierarchyNode<M>` struct with in-memory tree reconstruction.

### Task 4: Integration & Property Tests
- [ ] Add `crates/runtime/tests/nested_writes_test.rs`:
  - Test nested insert, connect, and disconnect operations on PostgreSQL, SQLite, and MySQL.
  - Test rollback behavior on nested validation errors.
- [ ] Add `crates/runtime/tests/tree_hierarchy_test.rs`:
  - Test ancestor and descendant retrieval across deep category trees.

---

## 4. Verification & Testing Strategy

```powershell
# 1. Run nested write integration tests
cargo test -p ruprizzle --test nested_writes_test

# 2. Run tree hierarchy tests
cargo test -p ruprizzle --test tree_hierarchy_test

# 3. Mechanical gates
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
```

---

## 5. Definition of Done

1. Implicit Many-to-Many join tables work out of the box with automatic migration DDL.
2. Nested relational writes execute atomically within a single transaction.
3. Tree query helpers retrieve ancestor/descendant hierarchies with zero recursion bugs.
