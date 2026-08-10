# Phase B Operability Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement Production Readiness Plan PR-04 through PR-07 so the runtime exposes query and migration tracing, configurable pool behavior, pool saturation/readiness APIs, and PII-safe error display.

**Architecture:** Keep `Executor` as the single query instrumentation choke point and instrument its existing `Pool` and `Tx` implementations without changing builder APIs. Expand the pool module with additive configuration and operational helpers, and make error redaction a `Display` policy while retaining explicit access to captured conflict data.

**Tech Stack:** Rust 2024, MSRV 1.85, sqlx 0.8 over `Any`, tracing 0.1, tracing-subscriber 0.3 for tests/docs, tokio, cargo test/clippy/fmt.

## Global Constraints

- **MSRV is 1.85.** Every change must compile on it.
- **`#![forbid(unsafe_code)]` stays in all crates.**
- **Zero clippy warnings.** Run `cargo clippy --workspace --all-targets -- -D warnings`.
- **No `unwrap()` or `expect()` in new library source.** Tests may use them.
- **No user bind values in tracing events.** Emit bind count only.
- **Keep `connect(url)` backward compatible.** Defaults mirror sqlx: max 10, min 0, acquire 30s, idle 600s, lifetime 1800s, test-before-acquire true.
- **Keep the current workspace version `0.1.0-alpha.2`; do not apply the older plan header's version note.**
- **Run DB-backed verification with `RUPRIZZLE_REQUIRE_DB=1` and the configured PostgreSQL URL.**
- **Update `ProjectPlan/ProductionReadinessPlan.md` for each completed PR in the same commit as that PR's implementation.**
- **Commit after each PR task; do not push.**

## File map

| File | Responsibility | Tasks |
|---|---|---|
| `crates/runtime/Cargo.toml` | Direct runtime tracing and test subscriber dependencies | 1 |
| `crates/runtime/src/executor.rs` | Pool query event instrumentation | 1 |
| `crates/runtime/src/tx.rs` | Transaction query and lifecycle event instrumentation | 1 |
| `crates/migrate/Cargo.toml` | Direct migration tracing dependency | 1 |
| `crates/migrate/src/runner.rs` | Migration start/completion events | 1 |
| `crates/runtime/tests/tracing.rs` | Query event regression tests | 1 |
| `crates/runtime/src/pool.rs` | Pool config, metrics, readiness | 2, 3 |
| `crates/runtime/src/lib.rs` | Public pool re-exports | 2, 3 |
| `crates/runtime/tests/pool_config.rs` | Pool config, stats, and ping tests | 2, 3 |
| `crates/runtime/src/error.rs` | Redacted error display and accessor | 4 |
| `crates/runtime/tests/error_redaction.rs` | PII redaction regression tests | 4 |
| `docs/query-guide.md` | Public observability, pooling, and error policy docs | 1, 2, 4 |
| `docs/known-limitations.md` | Remove completed operability deferral | 1 |
| `ProjectPlan/ProductionReadinessPlan.md` | Task checklists and Phase B status | 1, 2, 3, 4 |

---

### Task 0: Verify the clean baseline

**Files:** None.

- [ ] **Step 1: Confirm the branch and clean tree**

Run:

```powershell
git branch --show-current
git status --short --branch
```

Expected: branch `feat/p1-schema-parser`; only the already-committed design and no uncommitted files.

- [ ] **Step 2: Run formatting and workspace tests before implementation**

Run sequentially:

```powershell
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
$env:RUPRIZZLE_REQUIRE_DB = "1"; $env:RUPRIZZLE_TEST_PG_URL = "postgres://ruprizzle:ruprizzle@localhost:5432/ruprizzle_test"; cargo test --workspace
```

Expected: formatting clean, clippy exit 0, and the existing workspace test suite passes. If PostgreSQL is unavailable, stop and report the baseline failure rather than attributing it to Phase B.

---

### Task 1: PR-04 — Instrument query and migration execution

**Files:**
- Modify: `crates/runtime/Cargo.toml` dependency and dev-dependency sections.
- Modify: `crates/runtime/src/executor.rs` pool implementation.
- Modify: `crates/runtime/src/tx.rs` transaction implementation and lifecycle methods.
- Modify: `crates/migrate/Cargo.toml` dependency section.
- Modify: `crates/migrate/src/runner.rs` migration loop.
- Create: `crates/runtime/tests/tracing.rs`.
- Modify: `docs/query-guide.md` and `docs/known-limitations.md`.
- Modify: `ProjectPlan/ProductionReadinessPlan.md` PR-04 checklist and Phase B status.

**Interfaces:**
- Consumes the existing `Executor` methods: `fetch_all_raw`, `execute_raw`, and `stream_raw`.
- Produces `ruprizzle::query` debug success/warn failure events with SQL, bind count, result count where applicable, elapsed milliseconds, and error on failure.
- Produces `ruprizzle::migrate` info events with migration ID, statement count, and elapsed milliseconds.

- [ ] **Step 1: Add the direct dependencies required by the tests and implementation**

Add to `[dependencies]` in both `crates/runtime/Cargo.toml` and `crates/migrate/Cargo.toml`:

```toml
tracing = { version = "0.1", default-features = false, features = ["std"] }
```

Add to `[dev-dependencies]` in `crates/runtime/Cargo.toml`:

```toml
tracing-subscriber = { version = "0.3", default-features = false, features = ["registry"] }
```

- [ ] **Step 2: Write the failing tracing tests**

Create `crates/runtime/tests/tracing.rs`:

```rust
//! Every raw statement must emit a ruprizzle query event.

use std::sync::{Arc, Mutex};

use ruprizzle::Executor;
use tracing::{Event, Subscriber};
use tracing_subscriber::layer::{Context, Layer, SubscriberExt};
use tracing_subscriber::registry::Registry;

#[derive(Clone, Default)]
struct Captured(Arc<Mutex<Vec<String>>>);

impl<S: Subscriber> Layer<S> for Captured {
    fn on_event(&self, event: &Event<'_>, _ctx: Context<'_, S>) {
        if event.metadata().target() == "ruprizzle::query" {
            self.0
                .lock()
                .expect("capture lock")
                .push(event.metadata().name().to_owned());
        }
    }
}

fn run_with_capture(captured: Captured, operation: impl std::future::Future<Output = ()>) {
    let subscriber = Registry::default().with(captured);
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime");
    tracing::subscriber::with_default(subscriber, || runtime.block_on(operation));
}

#[test]
fn successful_query_emits_a_query_event() {
    let captured = Captured::default();
    let events = captured.0.clone();
    run_with_capture(captured, async {
        let pool = ruprizzle::connect("sqlite::memory:").await.expect("connect");
        pool.execute_raw("CREATE TABLE t (id INTEGER)".to_owned(), Vec::new())
            .await
            .expect("create table");
    });
    assert!(!events.lock().expect("capture lock").is_empty());
}

#[test]
fn failed_query_emits_a_query_event() {
    let captured = Captured::default();
    let events = captured.0.clone();
    run_with_capture(captured, async {
        let pool = ruprizzle::connect("sqlite::memory:").await.expect("connect");
        let _ = pool
            .execute_raw("THIS IS NOT SQL".to_owned(), Vec::new())
            .await;
    });
    assert!(!events.lock().expect("capture lock").is_empty());
}
```

- [ ] **Step 3: Run the focused tests and verify the expected red failure**

Run:

```powershell
cargo test -p ruprizzle --test tracing
```

Expected: the tests compile but fail because no `ruprizzle::query` events are emitted.

- [ ] **Step 4: Instrument pool execution at the Executor choke point**

In `crates/runtime/src/executor.rs`, replace `Pool`'s `fetch_all_raw` and `execute_raw` bodies with the following pattern. Keep `stream_raw` delegating to `fetch_all_raw`:

```rust
fn fetch_all_raw(
    &self,
    sql: String,
    binds: Vec<Value>,
) -> BoxFuture<'_, Result<Vec<AnyRow>, Error>> {
    Box::pin(async move {
        let bind_count = binds.len();
        let started = std::time::Instant::now();
        let mut q = sqlx::query::<sqlx::Any>(&sql);
        for bind in binds {
            q = q.bind(bind);
        }
        let result = q.fetch_all(self).await.map_err(Error::from);
        let elapsed_ms = started.elapsed().as_millis() as u64;
        match &result {
            Ok(rows) => tracing::debug!(
                target: "ruprizzle::query",
                sql = %sql,
                binds = bind_count,
                rows = rows.len(),
                elapsed_ms,
                "query"
            ),
            Err(error) => tracing::warn!(
                target: "ruprizzle::query",
                sql = %sql,
                binds = bind_count,
                elapsed_ms,
                error = error.kind(),
                "query failed"
            ),
        }
        result
    })
}

fn execute_raw(&self, sql: String, binds: Vec<Value>) -> BoxFuture<'_, Result<u64, Error>> {
    Box::pin(async move {
        let bind_count = binds.len();
        let started = std::time::Instant::now();
        let mut q = sqlx::query::<sqlx::Any>(&sql);
        for bind in binds {
            q = q.bind(bind);
        }
        let result = q
            .execute(self)
            .await
            .map(|result| result.rows_affected())
            .map_err(Error::from);
        let elapsed_ms = started.elapsed().as_millis() as u64;
        match &result {
            Ok(rows_affected) => tracing::debug!(
                target: "ruprizzle::query",
                sql = %sql,
                binds = bind_count,
                rows_affected,
                elapsed_ms,
                "execute"
            ),
            Err(error) => tracing::warn!(
                target: "ruprizzle::query",
                sql = %sql,
                binds = bind_count,
                elapsed_ms,
                error = error.kind(),
                "execute failed"
            ),
        }
        result
    })
}
```

- [ ] **Step 5: Instrument transaction execution and lifecycle events**

In `crates/runtime/src/tx.rs`, keep the existing transaction behavior and add the same post-completion event pattern to the `Executor for Tx` implementations. For `fetch_all_raw`, record `bind_count`, call `self.fetch_all_rows(&sql, binds).await`, then emit `query`/`query failed` with `rows` or `error`. For `execute_raw`, call `self.execute(&sql, binds).await`, then emit `execute`/`execute failed` with `rows_affected` or `error`. Do not emit bind values.

In `Tx::commit` and `Tx::rollback`, after the underlying sqlx operation succeeds, emit:

```rust
tracing::debug!(target: "ruprizzle::query", "transaction committed");
tracing::debug!(target: "ruprizzle::query", "transaction rolled back");
```

- [ ] **Step 6: Instrument migration application**

In `crates/migrate/src/runner.rs`, immediately after `let statements = split_statements(&m.up);`, add:

```rust
tracing::info!(
    target: "ruprizzle::migrate",
    migration = %m.id,
    statements = statements.len(),
    "applying migration"
);
```

Immediately before `applied_ids.push(m.id);`, after `tx.commit().await?`, add:

```rust
tracing::info!(
    target: "ruprizzle::migrate",
    migration = %m.id,
    elapsed_ms = elapsed,
    "migration applied"
);
```

- [ ] **Step 7: Run focused tests and verify green**

Run:

```powershell
cargo test -p ruprizzle --test tracing
```

Expected: both tracing tests pass.

- [ ] **Step 8: Document the observability contract**

Append to `docs/query-guide.md`:

```markdown
## Observability

Install a `tracing` subscriber in the application to see database activity:

```rust
tracing_subscriber::fmt()
    .with_env_filter("ruprizzle::query=debug,ruprizzle::migrate=info")
    .init();
```

`ruprizzle::query` reports SQL text, bind count, result counts, and elapsed
milliseconds. Bind values are not logged. `ruprizzle::migrate` reports migration
start and completion events with the migration ID and elapsed time. Avoid embedding
sensitive literals in raw SQL because raw SQL text is intentionally observable.
```

Remove `Connection pool metrics and query logging.` from the 0.2 deferral list in `docs/known-limitations.md`.

- [ ] **Step 9: Update the production plan status**

In `ProjectPlan/ProductionReadinessPlan.md`:

1. Change each PR-04 checklist marker from `- [ ]` to `- [x]`.
2. Add immediately below the PR-04 heading:

```markdown
**Status: COMPLETE.** Verified 2026-08-11 with focused tracing tests and the full workspace gate. Query events cover pool and transaction execution; bind values are not emitted.
```

Do not mark PR-05, PR-06, or PR-07 complete in this task.

- [ ] **Step 10: Run the PR-04 verification gate**

Run sequentially:

```powershell
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
$env:RUPRIZZLE_REQUIRE_DB = "1"; $env:RUPRIZZLE_TEST_PG_URL = "postgres://ruprizzle:ruprizzle@localhost:5432/ruprizzle_test"; cargo test --workspace
```

Expected: all commands exit 0.

- [ ] **Step 11: Review and commit PR-04**

Run `git diff --check`, inspect the diff for bind-value logging and accidental unrelated edits, then commit:

```powershell
git add crates/runtime/Cargo.toml crates/runtime/src/executor.rs crates/runtime/src/tx.rs crates/runtime/tests/tracing.rs crates/migrate/Cargo.toml crates/migrate/src/runner.rs docs/query-guide.md docs/known-limitations.md ProjectPlan/ProductionReadinessPlan.md
git commit -m "feat: instrument query and migration execution"
```

After committing, verify `git status --short` is clean before starting Task 2.

---

### Task 2: PR-05 — Make the connection pool configurable

**Files:**
- Modify: `crates/runtime/src/pool.rs`.
- Modify: `crates/runtime/src/lib.rs` pool re-export.
- Create: `crates/runtime/tests/pool_config.rs`.
- Modify: `docs/query-guide.md`.
- Modify: `ProjectPlan/ProductionReadinessPlan.md` PR-05 checklist and status.

**Interfaces:**
- Produces `PoolConfig` with public fields and `Default`.
- Produces `connect_with(url: &str, config: &PoolConfig) -> Result<Pool, crate::Error>`.
- Retains `connect(url: &str) -> Result<Pool, crate::Error>` as a default-config delegate.

- [ ] **Step 1: Write the failing tests**

Create `crates/runtime/tests/pool_config.rs`:

```rust
//! Pool configuration preserves sqlx defaults and accepts overrides.

use std::time::Duration;

use ruprizzle::pool::{PoolConfig, connect_with};

#[test]
fn defaults_match_sqlx() {
    let config = PoolConfig::default();
    assert_eq!(config.max_connections, 10);
    assert_eq!(config.min_connections, 0);
    assert_eq!(config.acquire_timeout, Duration::from_secs(30));
    assert_eq!(config.idle_timeout, Some(Duration::from_secs(600)));
    assert_eq!(config.max_lifetime, Some(Duration::from_secs(1800)));
    assert!(config.test_before_acquire);
}

#[tokio::test]
async fn configured_pool_connects() {
    let config = PoolConfig {
        max_connections: 3,
        ..PoolConfig::default()
    };
    let pool = connect_with("sqlite::memory:", &config).await.expect("connect");
    assert!(pool.options().get_max_connections() <= 3);
}
```

- [ ] **Step 2: Run the focused test and verify the expected red failure**

Run:

```powershell
cargo test -p ruprizzle --test pool_config
```

Expected: compilation fails because `PoolConfig` and `connect_with` do not exist.

- [ ] **Step 3: Implement `PoolConfig` and connection functions**

Replace `crates/runtime/src/pool.rs` with:

```rust
//! Connection pool construction, configuration, and metrics.

use std::time::Duration;

use sqlx::any::AnyPoolOptions;

/// A `sqlx` pool over the `Any` driver.
pub type Pool = sqlx::Pool<sqlx::Any>;

/// Configuration used to build a [`Pool`].
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct PoolConfig {
    /// Maximum connections held open by the pool.
    pub max_connections: u32,
    /// Connections kept warm while idle.
    pub min_connections: u32,
    /// Maximum time spent waiting to acquire a connection.
    pub acquire_timeout: Duration,
    /// Maximum idle connection duration; `None` keeps idle connections forever.
    pub idle_timeout: Option<Duration>,
    /// Maximum connection lifetime; `None` disables recycling by age.
    pub max_lifetime: Option<Duration>,
    /// Whether to test a connection before handing it out.
    pub test_before_acquire: bool,
}

impl Default for PoolConfig {
    fn default() -> Self {
        Self {
            max_connections: 10,
            min_connections: 0,
            acquire_timeout: Duration::from_secs(30),
            idle_timeout: Some(Duration::from_secs(600)),
            max_lifetime: Some(Duration::from_secs(1800)),
            test_before_acquire: true,
        }
    }
}

/// Connects using sqlx-compatible default pool settings.
///
/// # Errors
///
/// Returns an error if the URL cannot be parsed or the connection fails.
pub async fn connect(url: &str) -> Result<Pool, crate::Error> {
    connect_with(url, &PoolConfig::default()).await
}

/// Connects using explicit pool settings.
///
/// # Errors
///
/// Returns an error if the URL cannot be parsed or the connection fails.
pub async fn connect_with(url: &str, config: &PoolConfig) -> Result<Pool, crate::Error> {
    sqlx::any::install_default_drivers();
    AnyPoolOptions::new()
        .max_connections(config.max_connections)
        .min_connections(config.min_connections)
        .acquire_timeout(config.acquire_timeout)
        .idle_timeout(config.idle_timeout)
        .max_lifetime(config.max_lifetime)
        .test_before_acquire(config.test_before_acquire)
        .connect(url)
        .await
        .map_err(Into::into)
}
```

- [ ] **Step 4: Re-export the additive API**

Change the pool re-export in `crates/runtime/src/lib.rs` to:

```rust
pub use pool::{Pool, PoolConfig, connect, connect_with};
```

- [ ] **Step 5: Run focused tests and verify green**

Run:

```powershell
cargo test -p ruprizzle --test pool_config
```

Expected: both tests pass. If sqlx 0.8 does not expose `pool.options().get_max_connections()`, remove only that options assertion and retain the default-value and successful-connect assertions; document the API limitation in the task commit.

- [ ] **Step 6: Document connection sizing**

Append to `docs/query-guide.md`:

```markdown
## Connection pooling

Use `PoolConfig` when the application needs limits different from sqlx defaults:

```rust
use std::time::Duration;
use ruprizzle::pool::{connect_with, PoolConfig};

let config = PoolConfig {
    max_connections: 8,
    acquire_timeout: Duration::from_secs(5),
    ..PoolConfig::default()
};
let pool = connect_with("postgres://...", &config).await?;
```

Set `max_connections` below the database's `max_connections` after accounting for
the number of application instances and other database clients.
```

- [ ] **Step 7: Update the production plan status**

Change all PR-05 checklist markers to `- [x]` and add below its heading:

```markdown
**Status: COMPLETE.** Verified 2026-08-11 with pool configuration tests and the full workspace gate. `connect(url)` remains backward compatible and `connect_with` applies all `PoolConfig` fields.
```

- [ ] **Step 8: Run the full gate, review, and commit PR-05**

Run:

```powershell
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
$env:RUPRIZZLE_REQUIRE_DB = "1"; $env:RUPRIZZLE_TEST_PG_URL = "postgres://ruprizzle:ruprizzle@localhost:5432/ruprizzle_test"; cargo test --workspace
git diff --check
```

Then commit:

```powershell
git add crates/runtime/src/pool.rs crates/runtime/src/lib.rs crates/runtime/tests/pool_config.rs docs/query-guide.md ProjectPlan/ProductionReadinessPlan.md
git commit -m "feat(pool): expose pool configuration"
```

Verify the tree is clean before Task 3.

---

### Task 3: PR-06 — Expose pool metrics and a readiness check

**Files:**
- Modify: `crates/runtime/src/pool.rs`.
- Modify: `crates/runtime/src/lib.rs`.
- Modify: `crates/runtime/tests/pool_config.rs`.
- Modify: `ProjectPlan/ProductionReadinessPlan.md` PR-06 checklist and status.

**Interfaces:**
- Produces non-exhaustive `PoolStats { size: u32, idle: usize, in_use: usize }`.
- Produces `stats(&Pool) -> PoolStats`.
- Produces `ping(&Pool) -> Result<(), crate::Error>`.

- [ ] **Step 1: Add failing tests to the existing pool test file**

Append to `crates/runtime/tests/pool_config.rs`:

```rust
#[tokio::test]
async fn stats_and_ping_report_a_live_pool() {
    let pool = ruprizzle::connect("sqlite::memory:").await.expect("connect");
    ruprizzle::pool::ping(&pool).await.expect("ping");
    let stats = ruprizzle::pool::stats(&pool);
    assert_eq!(stats.in_use + stats.idle, stats.size as usize);
}

#[tokio::test]
async fn ping_reports_an_unreachable_database() {
    let config = PoolConfig {
        acquire_timeout: Duration::from_millis(200),
        ..PoolConfig::default()
    };
    let Ok(pool) = connect_with("postgres://127.0.0.1:1/nope", &config).await else {
        return;
    };
    assert!(ruprizzle::pool::ping(&pool).await.is_err());
}
```

- [ ] **Step 2: Run the focused tests and verify the expected red failure**

Run:

```powershell
cargo test -p ruprizzle --test pool_config
```

Expected: compilation fails because `stats` and `ping` do not exist.

- [ ] **Step 3: Implement `PoolStats`, `stats`, and `ping`**

Append to `crates/runtime/src/pool.rs`:

```rust
/// Point-in-time pool saturation data for metrics endpoints.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct PoolStats {
    /// Total connections currently held by the pool.
    pub size: u32,
    /// Connections immediately available for checkout.
    pub idle: usize,
    /// Connections currently checked out.
    pub in_use: usize,
}

/// Samples the current pool saturation.
#[must_use]
pub fn stats(pool: &Pool) -> PoolStats {
    let size = pool.size();
    let idle = pool.num_idle();
    PoolStats {
        size,
        idle,
        in_use: (size as usize).saturating_sub(idle),
    }
}

/// Checks database reachability for readiness probes.
///
/// # Errors
///
/// Returns an error if a connection cannot be acquired or `SELECT 1` fails.
pub async fn ping(pool: &Pool) -> Result<(), crate::Error> {
    sqlx::query("SELECT 1")
        .execute(pool)
        .await
        .map(|_| ())
        .map_err(Into::into)
}
```

- [ ] **Step 4: Re-export the operational APIs**

Change the pool re-export in `crates/runtime/src/lib.rs` to:

```rust
pub use pool::{Pool, PoolConfig, PoolStats, connect, connect_with, ping, stats};
```

- [ ] **Step 5: Run focused tests and verify green**

Run:

```powershell
cargo test -p ruprizzle --test pool_config
```

Expected: all four pool tests pass.

- [ ] **Step 6: Update the production plan status**

Change all PR-06 checklist markers to `- [x]` and add below its heading:

```markdown
**Status: COMPLETE.** Verified 2026-08-11 with live-pool stats/ping tests and the full workspace gate. Readiness uses `SELECT 1`; metrics expose total, idle, and in-use connections.
```

- [ ] **Step 7: Run the full gate, review, and commit PR-06**

Run:

```powershell
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
$env:RUPRIZZLE_REQUIRE_DB = "1"; $env:RUPRIZZLE_TEST_PG_URL = "postgres://ruprizzle:ruprizzle@localhost:5432/ruprizzle_test"; cargo test --workspace
git diff --check
```

Then commit:

```powershell
git add crates/runtime/src/pool.rs crates/runtime/src/lib.rs crates/runtime/tests/pool_config.rs ProjectPlan/ProductionReadinessPlan.md
git commit -m "feat(pool): add saturation metrics and readiness check"
```

Verify the tree is clean before Task 4.

---

### Task 4: PR-07 — Keep user data out of error display

**Files:**
- Modify: `crates/runtime/src/error.rs`.
- Create: `crates/runtime/tests/error_redaction.rs`.
- Modify: `docs/query-guide.md`.
- Modify: `ProjectPlan/ProductionReadinessPlan.md` PR-07 checklist and Phase B status.

**Interfaces:**
- Changes `Error::UniqueViolation` display to omit `value`.
- Produces `Error::conflicting_value(&self) -> Option<&str>`.

- [ ] **Step 1: Write the failing redaction tests**

Create `crates/runtime/tests/error_redaction.rs`:

```rust
//! User data must not reach logs through an error's Display.

use ruprizzle::Error;

#[test]
fn unique_violation_display_omits_the_value() {
    let error = Error::UniqueViolation {
        table: "users".to_owned(),
        columns: "email".to_owned(),
        value: Some("alice@example.com".to_owned()),
    };
    let rendered = error.to_string();
    assert!(!rendered.contains("alice@example.com"));
    assert!(rendered.contains("users"));
    assert!(rendered.contains("email"));
}

#[test]
fn conflicting_value_is_available_explicitly() {
    let error = Error::UniqueViolation {
        table: "users".to_owned(),
        columns: "email".to_owned(),
        value: Some("alice@example.com".to_owned()),
    };
    assert_eq!(error.conflicting_value(), Some("alice@example.com"));
}
```

- [ ] **Step 2: Run the focused tests and verify the expected red failure**

Run:

```powershell
cargo test -p ruprizzle --test error_redaction
```

Expected: the display test fails because the value is present, and the accessor test fails to compile because `conflicting_value` is missing.

- [ ] **Step 3: Redact display output and add the explicit accessor**

In `crates/runtime/src/error.rs`, change the `UniqueViolation` attribute to:

```rust
#[error("unique constraint violated on `{table}.{columns}`")]
```

Add after the enum:

```rust
impl Error {
    /// Returns the captured value that violated a unique constraint, if any.
    ///
    /// This is user data and is intentionally not part of [`Display`].
    #[must_use]
    pub fn conflicting_value(&self) -> Option<&str> {
        match self {
            Self::UniqueViolation { value, .. } => value.as_deref(),
            _ => None,
        }
    }
}
```

- [ ] **Step 4: Run focused tests and verify green**

Run:

```powershell
cargo test -p ruprizzle --test error_redaction
```

Expected: both tests pass.

- [ ] **Step 5: Document the error policy**

Append to `docs/query-guide.md`:

```markdown
## Error handling and sensitive values

Constraint errors include table and column context in their default `Display`
text, but conflicting values are deliberately omitted because they may contain
PII. If an application has an explicit policy for using the value, call
`Error::conflicting_value()` and handle the returned data deliberately rather
than logging the complete error blindly.
```

- [ ] **Step 6: Update PR-07 and Phase B status in the production plan**

Change all PR-07 checklist markers to `- [x]` and add below its heading:

```markdown
**Status: COMPLETE.** Verified 2026-08-11 with redaction/accessor tests and the full workspace gate. `UniqueViolation` keeps the captured value available explicitly but omits it from `Display`.
```

Immediately below the Phase B exit-gate paragraph, add:

```markdown
**Status: COMPLETE.** Verified 2026-08-11: PR-04 query/migration tracing, PR-05 pool configuration, PR-06 pool stats/readiness, and PR-07 default error redaction are implemented and tested.
```

- [ ] **Step 7: Run the final full gate and review the complete phase**

Run:

```powershell
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
$env:RUPRIZZLE_REQUIRE_DB = "1"; $env:RUPRIZZLE_TEST_PG_URL = "postgres://ruprizzle:ruprizzle@localhost:5432/ruprizzle_test"; cargo test --workspace
git diff --check
git status --short
```

Expected: all verification commands exit 0 and only intended files are present before the PR-07 commit.

- [ ] **Step 8: Commit PR-07**

```powershell
git add crates/runtime/src/error.rs crates/runtime/tests/error_redaction.rs docs/query-guide.md ProjectPlan/ProductionReadinessPlan.md
git commit -m "fix: keep conflicting values out of error display"
```

Verify the branch is clean and capture `git log --oneline -5`.

---

### Task 5: Merge the completed Phase B branch into `main`

**Files:** None beyond the already committed Phase B changes.

- [ ] **Step 1: Verify the feature branch commit and clean tree**

Run:

```powershell
git status --short --branch
git log --oneline --decorate -8
```

Expected: clean `feat/p1-schema-parser` with separate commits for the design and PR-04 through PR-07.

- [ ] **Step 2: Switch to `main` only after the tree is clean**

Run:

```powershell
git switch main
```

Do not use force, reset, or checkout-overwrite flags.

- [ ] **Step 3: Merge the feature branch locally**

Run:

```powershell
git merge --no-ff feat/p1-schema-parser -m "merge: Phase B operability"
```

Expected: merge completes without conflicts. If a conflict occurs, stop and inspect it rather than discarding either side.

- [ ] **Step 4: Verify the merged main branch**

Run:

```powershell
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
$env:RUPRIZZLE_REQUIRE_DB = "1"; $env:RUPRIZZLE_TEST_PG_URL = "postgres://ruprizzle:ruprizzle@localhost:5432/ruprizzle_test"; cargo test --workspace
git status --short --branch
```

Expected: all commands exit 0, `main` is clean, and no push is performed.
