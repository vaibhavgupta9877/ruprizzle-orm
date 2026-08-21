# Plan 13: Query Caching, Plan Cache & PostGIS Geospatial

**Date:** 2026-08-22  
**Author:** Vaibhav Gupta <vaibhavgupta9877@gmail.com>  
**Status:** Ready for Execution  
**Milestone:** v1.4.0 (Additive, Minor Release)  
**Primary Crates:** `crates/core`, `crates/parser`, `crates/dialect`, `crates/runtime`, `crates/migrate`

---

## 1. Context, Objectives & Scope

High-throughput production applications and location-based services demand sub-millisecond response times and native spatial querying. Prisma charges premium SaaS pricing for edge caching (Prisma Accelerate); `ruprizzle` provides native, zero-cost in-process and distributed caching out of the box in **v1.4**.

### Key Capabilities in v1.4
1. **Query Result Caching:**
   - In-memory LRU cache and Redis distributed cache adapters.
   - Declarative `.cache(Duration)` and `.cache_key(...)` query builder modifiers.
   - Smart model-level cache invalidation (mutations to `Post` automatically purge affected query cache tags).
2. **Prepared Query Plan Caching:**
   - Bypasses SQL generation and AST compilation on hot-path queries, caching compiled parameterized SQL strings for instant binding.
3. **PostGIS & Geospatial Extensions:**
   - Native spatial types: `Point`, `Polygon`, `MultiPolygon`, `LineString` with optional SRID (default WGS 84 / `4326`).
   - Spatial indexing: `@@index([location], type: Gist)`.
   - Spatial distance & containment filters: `.distance_to()`, `.within_radius()`, `.intersects()`, `.contains()`.

---

## 2. Technical Architecture & DSL Specification

### 2.1 Schema DSL for Geospatial Types

```ruprizzle
datasource db {
  provider   = "postgres"
  url        = env("DATABASE_URL")
  extensions = ["postgis"]
}

model StoreLocation {
  id        String   @id @default(uuid())
  name      String
  location  Point    // Stored as GEOMETRY(Point, 4326)
  boundary  Polygon? // Stored as GEOMETRY(Polygon, 4326)

  @@index([location], type: Gist)
}
```

---

### 2.2 Query Caching & Spatial Query API

```rust
// 1. Query Result Caching with Tagged Invalidation
let top_stores = StoreLocation::find_many()
    .cache(std::time::Duration::from_secs(300)) // Cache for 5 minutes
    .all(&pool)
    .await?;

// 2. Geospatial Radius Search (Finding stores within 5 km of user coordinates)
let user_coords = Point::new(37.7749, -122.4194); // San Francisco (lat, lng)

let nearby_stores = StoreLocation::find_many()
    .where(StoreLocation::location.within_radius(&user_coords, 5000.0)) // 5,000 meters
    .order_by(StoreLocation::location.distance_asc(&user_coords))
    .limit(10)
    .all(&pool)
    .await?;

// 3. Spatial Containment Check
let stores_in_region = StoreLocation::find_many()
    .where(StoreLocation::location.intersects(&city_polygon))
    .all(&pool)
    .await?;
```

---

## 3. Step-by-Step Implementation Tasks

### Task 1: Result Cache Layer & Tagged Invalidation
- [ ] In `crates/runtime/src/cache.rs`:
  - Implement `QueryCache` trait with `MokaCache` (in-memory LRU) and optional `RedisCache`.
  - Implement automatic tag invalidation triggered by `InsertQuery`, `UpdateQuery`, and `DeleteQuery`.

### Task 2: AST Plan Cache Optimization
- [ ] In `crates/runtime/src/compile.rs`:
  - Implement query hash cache storing pre-compiled SQL format strings to skip AST construction.

### Task 3: Spatial Types, IR & Pest Grammar
- [ ] In `crates/parser/src/schema.pest` & `crates/core/src/ir.rs`:
  - Add `Point`, `Polygon`, `LineString` scalar types.
  - Add `IndexType::Gist`.
- [ ] In `crates/dialect/src/postgres.rs`:
  - Emit PostGIS DDL and spatial distance operators (`ST_DWithin`, `ST_Distance`, `ST_Intersects`).

### Task 4: Integration & Geospatial Conformance Tests
- [ ] Add `crates/runtime/tests/geospatial_test.rs`:
  - Test PostGIS point creation, spatial indexing, and distance queries.
- [ ] Add `crates/runtime/tests/query_caching_test.rs`:
  - Test cache hits, expiration, and invalidation upon row updates.

---

## 4. Verification & Testing Strategy

```powershell
# 1. Run caching tests
cargo test -p ruprizzle --test query_caching_test

# 2. Run PostGIS spatial tests (requires PostgreSQL with PostGIS extension)
cargo test -p ruprizzle --features "postgres" --test geospatial_test

# 3. Mechanical gates
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
```

---

## 5. Definition of Done

1. Queries can be cached in-memory or in Redis with automatic model-level cache invalidation.
2. PostGIS spatial types (`Point`, `Polygon`) round-trip with native GiST index generation and radius distance queries.
3. 100% test coverage with zero memory leaks.
