# Operations guide

This document is for the person running `ruprizzle` in production: what the
runtime emits, what to watch, and what to do when it misbehaves.

## Telemetry

`ruprizzle` uses two telemetry systems:

- **`tracing`** for events and spans. Enable a subscriber that captures the
  `ruprizzle::query`, `ruprizzle::slow_query`, `ruprizzle::migrate`,
  `ruprizzle::pool`, and `ruprizzle::connection` targets.
- **`metrics`** (behind the `metrics` feature) for counters, histograms, and
  gauges. Install a recorder such as `metrics-exporter-prometheus` or an
  OpenTelemetry adapter.

Both systems deliberately avoid PII. Binds are reported as a count, never as
values.

## What each span means

### `ruprizzle::query`

`DEBUG` event emitted after every executed statement.

- `sql` — the SQL shape, with placeholders intact.
- `binds` — number of placeholder values.
- `rows` / `rows_affected` — result summary.
- `elapsed_ms` — how long the statement took.

Enable this target when debugging latency or when you need a query log for
auditing.

### `ruprizzle::slow_query`

`WARN` event emitted when a query exceeds `PoolConfig::slow_query_threshold`.

- `sql` — the SQL shape.
- `binds` — placeholder count.
- `elapsed_ms` — actual duration.

Set the threshold to the 99th percentile of normal queries. A slow query is not
an outage by itself, but a sustained spike in these events is a leading
indicator of saturation or a missing index.

### `ruprizzle::migrate`

`INFO` events emitted during migration application.

- `migration` — the migration id.
- `statements` — number of statements in the file.
- `elapsed_ms` — how long the migration took.

A long `elapsed_ms` on a migration is expected for backfills. A sudden failure
is a `WARN` or `ERROR` with the migration id.

### `ruprizzle::pool` and `ruprizzle::connection`

`INFO`/`WARN` events for connection lifecycle (see W3-04).

- `connect`, `disconnect`, `acquire_timeout`, `reconnect`.

## Metrics

Enable the `metrics` feature and install a recorder.

### Query metrics

| Name | Type | Labels | Meaning |
|------|------|--------|---------|
| `ruprizzle_query_total` | counter | — | Total executed statements. |
| `ruprizzle_query_duration_seconds` | histogram | — | Query latency. |
| `ruprizzle_query_errors_total` | counter | `kind` | Errors by `Error::kind()`. |

Slow queries are emitted as `WARN` events on the `ruprizzle::slow_query` tracing
target, not as a metric counter.

### Pool metrics

| Name | Type | Meaning |
|------|------|---------|
| `ruprizzle_pool_size` | gauge | Connections held by the pool. |
| `ruprizzle_pool_idle` | gauge | Connections available for checkout. |
| `ruprizzle_pool_in_use` | gauge | Connections currently checked out. |
| `ruprizzle_pool_waiters` | gauge | Tasks waiting for a connection. |

### Migration metrics

| Name | Type | Meaning |
|------|------|---------|
| `ruprizzle_migration_applied_total` | counter | Migrations applied. |
| `ruprizzle_migration_duration_seconds` | histogram | Per-migration duration. |

## How to read `PoolStats`

```rust
use ruprizzle::pool;

let stats = pool::report_metrics(&pool);
println!("size={} idle={} in_use={} waiters={}",
         stats.size, stats.idle, stats.in_use, stats.waiters);
```

`report_metrics` also emits the current values to the `metrics` recorder.

Interpretation:

- `in_use` near `size` for a sustained period means the pool is fully utilized.
- `waiters` > 0 means requests are queueing. Increase `max_connections` or
  reduce query latency.
- `idle` near 0 means the pool is sized at the edge. `min_connections` can
  smooth out cold-start latency.

## What to do when `ping` fails

`pool::ping` runs `SELECT 1`. If it fails:

1. Check the `ruprizzle::connection` target for `connect` and `reconnect`
   events. Repeated reconnects mean the network or database process is unstable.
2. Check `ruprizzle::query_errors_total{kind="connection"}`. A rising count
   confirms connection-level failures.
3. Verify `acquire_timeout` is not too aggressive for a remote database.
4. If using `sqlite-rusqlite`, ensure the database file is on a local volume
   with write access.

## Read-replica routing

`RoutedPool` sends reads to replicas and keeps writes and transactions on the primary.

```rust,ignore
use ruprizzle::{connect, LoadBalancing, RoutedPool};

let routed = RoutedPool::builder(connect(&primary_url).await?)
    .add_replica(connect(&replica_a_url).await?)
    .add_replica(connect(&replica_b_url).await?)
    .load_balancing(LoadBalancing::LeastConnections)
    .fallback_to_primary(true)
    .build();

let read_pool = routed.select_replica();   // a healthy replica, or the primary
let tx = routed.begin().await?;            // always the primary
```

`LoadBalancing` is `RoundRobin` (the default), `LeastConnections`, or `Random`. A
replica is only ever chosen while it is marked healthy, and `check_health()` pings
every replica and updates those flags — call it on a timer from your own supervisor;
nothing pings in the background on your behalf. With `fallback_to_primary(true)`, a
read is served by the primary when no replica is healthy rather than failing; set it
to `false` when serving stale-free reads from the primary is worse than an error.

## Query result cache

`QueryCache` is the trait; `InMemoryCache` is the bundled implementation, bounded by a
maximum entry count and invalidated either by TTL or by tag.

```rust,ignore
use ruprizzle::{InMemoryCache, QueryCache};
use std::time::Duration;

let cache = InMemoryCache::new(10_000);
cache.set("users:active", encoded, Some(Duration::from_secs(30)), &["users"]);

let hit = cache.get("users:active");       // None once the TTL has passed
cache.invalidate_tag("users");             // drop everything tagged `users`
```

Tags are the invalidation unit: tag each cached result with the tables it was read
from, and invalidate that tag on every write to those tables. Entries expire lazily on
read; `prune_expired()` reclaims memory eagerly if you would rather not wait for the
next lookup. Implement `QueryCache` yourself to put the same interface in front of
Redis or any other shared store — nothing in the trait assumes a single process.

## Alerts

Suggested thresholds for a health dashboard:

- **Error rate**: `rate(ruprizzle_query_errors_total[5m]) > 0.01/sec`.
- **Slow query rate**: use the `ruprizzle_query_duration_seconds` histogram (e.g. p99 above your `PoolConfig::slow_query_threshold`) or watch `WARN` events on the `ruprizzle::slow_query` tracing target.
- **Pool saturation**: `ruprizzle_pool_waiters > 0` for more than 30 seconds.
- **Migrations stuck**: `ruprizzle_migration_applied_total` not increasing while
  a deployment is expected.

## Example Prometheus scrape

```toml
[dependencies]
ruprizzle = { version = "1.0.0", features = ["metrics"] }
metrics-exporter-prometheus = "0.15"
```

```rust
use metrics_exporter_prometheus::PrometheusBuilder;

PrometheusBuilder::new()
    .install_recorder()
    .expect("failed to install Prometheus recorder");
```

The recorder will expose `ruprizzle_*` metrics on the configured endpoint. Point
Grafana at the same endpoint for the dashboard above.
