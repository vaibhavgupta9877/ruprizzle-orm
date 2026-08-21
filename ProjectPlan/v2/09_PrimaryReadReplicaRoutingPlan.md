# Plan 09: Primary / Read-Replica Connection Routing

**Date:** 2026-08-22  
**Author:** Vaibhav Gupta <vaibhavgupta9877@gmail.com>  
**Status:** Ready for Execution  
**Milestone:** v1.4.0 (Additive, Minor Release)  
**Primary Crates:** `crates/runtime`

---

## 1. Context, Objectives & Scope

Production architectures scale query throughput by distributing read traffic across multiple database read replicas while routing write operations and transactions exclusively to the primary writer instance. Prisma charges for this under its paid Prisma Accelerate service; `ruprizzle` provides it natively in **v1.4**.

In **v1.4**, `ruprizzle-runtime` introduces **intelligent connection routing**:
1. **Multi-Pool Connection Manager (`RoutedPool`):** Manages a single primary pool alongside multiple read replica pools with active health checking and load balancing (Round Robin, Least Connections, Random).
2. **Automatic Query Classification:** `SELECT` queries automatically route to replicas; `INSERT`, `UPDATE`, `DELETE`, and active transactions (`Tx`) route directly to the primary.
3. **Explicit Consistency Overrides:** Simple `.use_primary()` and `.use_replica()` modifiers on any query builder.
4. **Replication Lag & Failover Guardrails:** Automatic failover to primary if replicas become unhealthy; optional write-after-read session stickiness to prevent read skew.

---

## 2. Technical Architecture & Routing Engine

```mermaid
graph TD
    Query["Incoming Ruprizzle Query"] --> Router{"Query Router"}
    
    Router -->|SELECT (Default)| LB["Load Balancer<br/>(Round-Robin / Least Conn)"]
    Router -->|INSERT / UPDATE / DELETE| Primary["Primary / Writer DB Pool"]
    Router -->|Active Transaction (Tx)| Primary
    Router -->|.use_primary() Override| Primary
    Router -->|Replicas Unhealthy Fallback| Primary

    LB --> R1["Read Replica 1 Pool"]
    LB --> R2["Read Replica 2 Pool"]
    LB --> RN["Read Replica N Pool"]
```

---

### 2.1 API Design & Configuration

```rust
use std::time::Duration;
use ruprizzle_runtime::pool::{RoutedPool, LoadBalancing};

let pool = RoutedPool::builder()
    .primary("postgres://writer.db.internal:5432/production")
    .replica("postgres://reader-1.db.internal:5432/production")
    .replica("postgres://reader-2.db.internal:5432/production")
    .load_balancing(LoadBalancing::LeastConnections)
    .health_check_interval(Duration::from_secs(5))
    .fallback_to_primary_on_error(true)
    .build()
    .await?;

// 1. Automatic Read Routing (Hits reader-1 or reader-2)
let users = User::find_many().all(&pool).await?;

// 2. Automatic Write Routing (Hits primary writer)
User::create().email("test@example.com").save(&pool).await?;

// 3. Explicit Override: Force read from primary for absolute consistency
let fresh_user = User::find_unique()
    .where(User::id.eq("usr_123"))
    .use_primary()
    .one(&pool)
    .await?;

// 4. Interactive Transaction (Automatically binds to primary connection)
pool.transaction(|tx| async move {
    let user = User::find_unique().where(User::id.eq("usr_123")).one(&tx).await?;
    User::update().where(User::id.eq("usr_123")).set_role(Role::ADMIN).save(&tx).await?;
    Ok(())
}).await?;
```

---

## 3. Step-by-Step Implementation Tasks

### Task 1: Design `RoutedPool` Data Structure
- [ ] In `crates/runtime/src/pool.rs`:
  - Implement `RoutedPool` containing one `primary: Arc<Pool>` and `replicas: Vec<Arc<Pool>>`.
  - Implement `LoadBalancing` algorithms (Round Robin with atomic counter, Least Connections using active connection gauge).

### Task 2: Implement Background Health Checker
- [ ] In `crates/runtime/src/pool.rs`:
  - Spawn periodic background task sending lightweight ping queries (`SELECT 1`) to each replica.
  - Dynamically mark unhealthy replicas as disabled and trigger fallback to primary.

### Task 3: Automatic Query Routing Dispatch
- [ ] In `crates/runtime/src/query.rs` & `executor.rs`:
  - Add query intent flag to AST compilation (`QueryIntent::Read`, `QueryIntent::Write`, `QueryIntent::Transaction`).
  - Implement `.use_primary()` and `.use_replica()` builder methods.
  - Dispatch executor requests to appropriate sub-pool.

### Task 4: Transaction & Session Guardrails
- [ ] In `crates/runtime/src/tx.rs`:
  - Ensure transaction handles always borrow from the primary connection pool.

### Task 5: Integration & Failover Testing
- [ ] Add `crates/runtime/tests/replica_routing_test.rs`:
  - Test round-robin distribution across multiple replica mock pools.
  - Test write queries route strictly to primary.
  - Test automatic failover when replica connection is abruptly terminated.

---

## 4. Verification & Testing Strategy

```powershell
# 1. Run replica routing unit & integration tests
cargo test -p ruprizzle --test replica_routing_test

# 2. Conformance & benchmark checks
cargo test -p ruprizzle-deep-tests --test routing_conformance

# 3. Mechanical gates
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
```

---

## 5. Definition of Done

1. `RoutedPool` manages primary and replica pools with customizable load balancing.
2. Read queries distribute evenly across healthy replicas; writes/transactions always hit primary.
3. Unhealthy replicas are removed within health check interval with zero user request drops.
4. `.use_primary()` guarantees strong read-after-write consistency.
