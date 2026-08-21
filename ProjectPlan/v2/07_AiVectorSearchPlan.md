# Plan 07: AI & Vector Search First-Class Integration (`pgvector` & `sqlite-vec`)

**Date:** 2026-08-22  
**Author:** Vaibhav Gupta <vaibhavgupta9877@gmail.com>  
**Status:** Ready for Execution  
**Milestone:** v2.2.0-alpha.1  
**Primary Crates:** `crates/core`, `crates/parser`, `crates/dialect`, `crates/migrate`, `crates/runtime`

---

## 1. Context, Objectives & Scope

Rust is rapidly becoming the language of choice for high-throughput AI agents, Retrieval-Augmented Generation (RAG) pipelines, and real-time embedding indexing. Previously, developers using ORMs had to drop down to raw SQL strings to perform vector similarity searches and manage vector index migrations.

In v2, `ruprizzle` delivers **first-class AI Vector Search**:
1. **Schema DSL Vector Type:** Declarative `Vector(dimension)` column definition with compile-time dimension validation.
2. **Migration Engine Vector Indexes:** Automated DDL generation for `pgvector` (`HNSW`, `IVFFlat`) and `sqlite-vec` extensions.
3. **Type-Safe Nearest-Neighbor Query API:** Ergonomic query builder operations (`.nearest_neighbors()`, `.with_distance()`) supporting Cosine, Euclidean (L2), and Dot Product distance metrics.

---

## 2. Technical Architecture & DSL Specification

### 2.1 Schema DSL Definition

```ruprizzle
datasource db {
  provider   = "postgres"
  url        = env("DATABASE_URL")
  extensions = ["vector"]
}

model DocumentChunk {
  id         String       @id @default(uuid())
  documentId String
  content    String
  embedding  Vector(1536)
  createdAt  DateTime     @default(now())

  @@index([embedding], type: Hnsw, distance: Cosine)
}
```

---

### 2.2 Core IR & AST Extensions (`crates/core`, `crates/parser`)

#### `crates/core/src/ir.rs`:
```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ScalarType {
    // Existing variants...
    Vector(u32), // Dimension count, e.g. 1536
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum IndexType {
    BTree,
    Gin,
    Gist,
    Hnsw,
    Ivfflat,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DistanceMetric {
    Cosine,
    L2,
    InnerProduct,
}
```

---

### 2.3 Dialect DDL Generation (`crates/dialect`, `crates/migrate`)

#### PostgreSQL (`pgvector`):
- Column DDL: `embedding vector(1536) NOT NULL`
- Index DDL:
  - Cosine: `CREATE INDEX idx_document_embedding ON "DocumentChunk" USING hnsw (embedding vector_cosine_ops);`
  - L2: `CREATE INDEX idx_document_embedding ON "DocumentChunk" USING hnsw (embedding vector_l2_ops);`
  - Inner Product: `CREATE INDEX idx_document_embedding ON "DocumentChunk" USING hnsw (embedding vector_ip_ops);`

#### SQLite (`sqlite-vec`):
- Initializes `vec0` virtual table or vector blob encoding:
  `CREATE VIRTUAL TABLE vec_items USING vec0(id TEXT PRIMARY KEY, embedding float[1536]);`

---

### 2.4 Query Builder Vector Operations (`crates/runtime`)

```rust
let query_embedding: Vec<f32> = get_openai_embedding("Rust memory safety").await?;

// 1. K-Nearest Neighbors lookup
let results: Vec<DocumentChunk> = DocumentChunk::find_many()
    .nearest_neighbors(DocumentChunk::embedding, &query_embedding, 10)
    .where(DocumentChunk::documentId.eq("doc_456"))
    .all(&pool)
    .await?;

// 2. Query with computed similarity distance score
let scored_results = DocumentChunk::find_many()
    .with_distance(DocumentChunk::embedding, &query_embedding, DistanceMetric::Cosine)
    .where(DocumentChunk::embedding.cosine_distance(&query_embedding).lt(0.25))
    .order_by(DocumentChunk::embedding.distance_asc(&query_embedding))
    .limit(5)
    .all(&pool)
    .await?;
```

#### SQL Operator Translation:
- **Cosine Distance:** `column <=> $1`
- **Euclidean (L2) Distance:** `column <-> $1`
- **Negative Inner Product:** `column <#> $1`

---

## 3. Step-by-Step Implementation Tasks

### Task 1: Parser Grammar & AST Lowering
- [ ] In `crates/parser/src/schema.pest`:
  - Add grammar rule for `Vector(dim)` type syntax.
  - Add grammar for index attributes `type: Hnsw | Ivfflat` and `distance: Cosine | L2 | InnerProduct`.
- [ ] In `crates/core/src/ir.rs` & `crates/parser/src/lower.rs`:
  - Lower `Vector` scalar type and validate dimension > 0.

### Task 2: Dialect DDL & Migration Engine
- [ ] In `crates/dialect/src/postgres.rs`:
  - Add vector column type rendering and HNSW/IVFFlat index DDL.
- [ ] In `crates/migrate/src/diff.rs` & `plan.rs`:
  - Detect changes in vector dimensions and generate safe migration steps.

### Task 3: Value Serialization & Vector Operators in Runtime
- [ ] In `crates/runtime/src/value.rs`:
  - Add `Value::Vector(Arc<[f32]>)` with pgvector binary and text encoding.
- [ ] In `crates/runtime/src/col.rs` & `filter.rs`:
  - Add `.nearest_neighbors()`, `.with_distance()`, `.cosine_distance()`, `.l2_distance()`.

### Task 4: Integration & Benchmark Testing
- [ ] Add `crates/runtime/tests/vector_search_test.rs`:
  - Test pgvector index creation, row insertion, and nearest neighbor search.
  - Benchmark vector query overhead vs raw SQL.

---

## 4. Verification & Testing Strategy

```powershell
# 1. Run vector unit and integration tests
cargo test -p ruprizzle --features "postgres" --test vector_search_test

# 2. Migration engine vector diff tests
cargo test -p ruprizzle-migrate --test vector_migration_test

# 3. Mechanical gates
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
```

---

## 5. Definition of Done

1. `schema.ruprizzle` supports `Vector(dim)` columns and `@@index(..., type: Hnsw)` definitions.
2. Migration engine accurately generates pgvector extensions and index DDL.
3. Query builder `.nearest_neighbors()` retrieves top-K results ordered by vector distance.
4. Passes full verification suite against live PostgreSQL with `pgvector` enabled.
