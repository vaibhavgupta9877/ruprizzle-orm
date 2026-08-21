# Plan 05: OpenTelemetry Semantic Tracing & Metrics 2.0

**Date:** 2026-08-22  
**Author:** Vaibhav Gupta <vaibhavgupta9877@gmail.com>  
**Status:** Ready for Execution  
**Milestone:** v1.4.0 (Additive, Minor Release)  
**Primary Crates:** `crates/runtime`  
**Dependencies Baseline:** `tracing 0.1.41`, `opentelemetry 0.28.0`, `opentelemetry-semantic-conventions 0.28.0`, `metrics 0.24.1`

---

## 1. Context, Objectives & Scope

Production engineering teams operating large-scale distributed systems require standardized OpenTelemetry (OTel) tracing and Prometheus metrics to monitor query latency, connection pool saturation, and slow database operations across APM platforms (Datadog, Honeycomb, New Relic, Grafana Tempo).

In **v1.4**, `ruprizzle-runtime` delivers **Observability 2.0**:
1. **OpenTelemetry Database Semantic Conventions:** Generates compliant OTel spans matching the OpenTelemetry 0.28 specification (`db.system`, `db.name`, `db.statement`, `db.operation`, `net.peer.name`).
2. **PII-Safe Query Sanitization:** SQL statements recorded in telemetry spans omit literal bind parameters, eliminating risk of secret/PII data leakage into logs.
3. **Comprehensive Pool & Query Metrics:** Real-time gauges and histograms for active connections, pool acquisition delays, execution duration, and slow query events.
4. **Zero-Cost Feature Flags:** Behind `features = ["otel", "metrics"]`, compiling down to zero runtime allocations when disabled.

---

## 2. Technical Architecture & Specification

### 2.1 Standardized OTel Database Spans

Every query executed by `ruprizzle` creates an OpenTelemetry span following standard semantic conventions:

| Span Attribute | Description | Example Value |
|---|---|---|
| `db.system` | Target database engine | `"postgresql"`, `"sqlite"`, `"mysql"` |
| `db.name` | Database name | `"production_main"` |
| `db.operation` | Query operation type | `"SELECT"`, `"INSERT"`, `"UPDATE"`, `"DELETE"`, `"TRANSACTION"` |
| `db.statement` | Parameterized / sanitized SQL string | `"SELECT id, name FROM users WHERE tenant_id = $1"` |
| `db.table` | Target primary table name | `"users"` |
| `net.peer.name` | Database host | `"db.prod.internal"` |
| `net.peer.port` | Database port | `5432` |

```mermaid
sequenceDiagram
    participant App as Application Code
    participant Exec as Ruprizzle Executor
    participant Pool as Connection Pool
    participant DB as Database Server

    App->>Exec: User::find_many().where(...).all(&pool)
    Note over Exec: Create Span: "db.query: SELECT users"
    Exec->>Pool: acquire_connection()
    Note over Pool: Record metric: pool_wait_duration
    Pool-->>Exec: Connection ready
    Exec->>DB: Send wire query
    DB-->>Exec: Return rowset
    Exec->>Exec: Decode rows to Model structs
    Note over Exec: Record metric: query_duration_seconds
    Note over Exec: End Span (Status: OK)
    Exec-->>App: Result<Vec<User>>
```

---

### 2.2 Metrics Catalog (`crates/runtime/src/metrics.rs`)

| Metric Name | Type | Labels / Dimensions | Description |
|---|---|---|---|
| `ruprizzle_pool_connections_active` | Gauge | `pool_name`, `system` | Number of connections currently executing queries |
| `ruprizzle_pool_connections_idle` | Gauge | `pool_name`, `system` | Number of idle connections ready in pool |
| `ruprizzle_pool_wait_duration_seconds` | Histogram | `pool_name`, `system` | Latency spent waiting for an available connection from pool |
| `ruprizzle_query_duration_seconds` | Histogram | `system`, `operation`, `table`, `status` | End-to-end query execution time |
| `ruprizzle_slow_queries_total` | Counter | `system`, `operation`, `table` | Count of queries exceeding `slow_query_threshold` |
| `ruprizzle_rows_affected_total` | Counter | `system`, `operation`, `table` | Total rows modified by INSERT / UPDATE / DELETE |

---

### 2.3 Pool Builder Configuration

```rust
let pool = Pool::builder()
    .url("postgres://postgres:password@localhost:5432/mydb")
    .max_connections(20)
    .min_connections(5)
    .slow_query_threshold(std::time::Duration::from_millis(150))
    .enable_otel_tracing(true)
    .build()
    .await?;
```

---

## 3. Step-by-Step Implementation Tasks

### Task 1: Semantic Span Instrumentation in Runtime Executor
- [ ] In `crates/runtime/src/executor.rs`:
  - Instrument `fetch_all`, `fetch_optional`, `fetch_one`, and `execute` with `tracing::span!` using OTel DB attributes.
  - Record execution status (`tracing::Level::INFO` on success, `tracing::Level::ERROR` with error description on failure).

### Task 2: PII Sanitization for SQL Strings
- [ ] In `crates/runtime/src/compile.rs`:
  - Add helper to emit sanitized parameterized SQL string without literal bind values for tracing export.

### Task 3: Expand Metrics Registry & Pool Gauges
- [ ] In `crates/runtime/src/metrics.rs`:
  - Add definitions for `POOL_WAIT_DURATION_SECONDS`, `QUERY_DURATION_SECONDS`, `SLOW_QUERIES_TOTAL`, `ROWS_AFFECTED_TOTAL`.
- [ ] In `crates/runtime/src/pool.rs`:
  - Hook connection checkout and checkin events into active/idle metric gauges.
  - Record slow query counter when elapsed time exceeds configured threshold.

### Task 4: Unit & Soak Validation
- [ ] Add `crates/runtime/tests/otel_tracing_test.rs`:
  - Verify OTel span generation and attributes under mock tracing subscriber.
  - Verify zero-cost execution when `otel` feature is disabled.

---

## 4. Verification & Testing Strategy

```powershell
# 1. Run tracing and metrics unit tests
cargo test -p ruprizzle --features "otel,metrics" --test otel_tracing_test

# 2. Soak metrics test
cargo test -p ruprizzle --features "otel,metrics" --test soak

# 3. Mechanical gates
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
```

---

## 5. Definition of Done

1. Query execution emits fully compliant OpenTelemetry semantic spans with sanitized SQL.
2. Connection pool checkout and query durations accurately recorded in Prometheus histograms.
3. Slow queries triggering warnings and telemetry counters above configured thresholds.
4. Compiles with zero overhead when `otel` and `metrics` feature flags are not active.
