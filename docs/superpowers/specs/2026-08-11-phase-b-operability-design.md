# Phase B Operability Design

**Date:** 2026-08-11
**Status:** Approved
**Scope:** Production Readiness Plan PR-04 through PR-07

## Goal

Make the runtime observable and operable in production without changing existing connection behavior or exposing bind values through query events and error `Display` output.

## Design

### PR-04: Query and migration tracing

`Executor` is the runtime choke point used by generated query builders. Instrument the `Pool` and `Tx` implementations at `fetch_all_raw` and `execute_raw` so every builder-issued database operation emits one event after completion. Successful operations use `DEBUG`; failed operations use `WARN`. Events target `ruprizzle::query` and include SQL text, bind count, result count where applicable, elapsed milliseconds, and the formatted error on failure. Bind values are never emitted. Pool streaming remains covered because it delegates to `fetch_all_raw`; transaction commit and rollback also emit query-target debug events.

Migration application emits `INFO` events at target `ruprizzle::migrate` when a migration starts and completes, including migration ID, statement count, and elapsed milliseconds.

The implementation adds direct `tracing` dependencies to the runtime and migration crates. Tests install a current-thread subscriber and verify both successful and failing query paths produce target events.

### PR-05: Configurable connection pool

Replace the pool module's alias-only surface with a public, non-exhaustive `PoolConfig` whose defaults mirror sqlx: max connections 10, min connections 0, 30-second acquire timeout, 10-minute idle timeout, 30-minute maximum lifetime, and connection testing enabled.

`connect(url)` remains backward compatible and delegates to `connect_with(url, &PoolConfig::default())`. `connect_with` applies every configuration field to `AnyPoolOptions` before connecting. The configuration type is re-exported from the crate root.

### PR-06: Pool metrics and readiness

Expose a point-in-time `PoolStats` value containing total pool size, idle connections, and calculated in-use connections. Add `stats(&Pool)` for scrape-friendly sampling and `ping(&Pool)` that executes `SELECT 1` for readiness probes. Re-export all new pool APIs from the crate root.

### PR-07: PII-safe error display

Keep the captured conflicting value in `Error::UniqueViolation`, but remove it from the `Display` string. Add `Error::conflicting_value()` so applications can deliberately access the value when policy permits. Existing table and column context remain in the default display output.

## Data flow and compatibility

Generated builders compile SQL with bind placeholders and pass values separately through `Executor`; tracing records only the SQL string and bind count. Existing `connect` callers continue to receive sqlx-compatible defaults. Pool and transaction implementations preserve their current error conversion and row behavior. The public error enum remains non-exhaustive.

The plan's SQL event field is retained for operational diagnosis. Raw SQL callers should not embed sensitive literals because the raw SQL text is intentionally observable; normal generated queries keep user values in binds, which are excluded.

## Testing and verification

Each PR follows red-green-refactor:

1. Add focused tests that express the public behavior.
2. Run the focused test and confirm it fails for the missing behavior.
3. Implement the smallest production change.
4. Run the focused test and the full workspace gate.
5. Update the corresponding checklist/status in `ProjectPlan/ProductionReadinessPlan.md`.
6. Commit the complete PR task before beginning the next one.

The full gate is:

```text
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
RUPRIZZLE_REQUIRE_DB=1 RUPRIZZLE_TEST_PG_URL=postgres://ruprizzle:ruprizzle@localhost:5432/ruprizzle_test cargo test --workspace
```

After PR-07, run the full gate again, review the final diff, and merge the feature branch into `main` locally without pushing.

## Out of scope

- A configurable query redaction/fingerprinting layer.
- Logging bind values.
- A metrics exporter dependency or framework-specific endpoint.
- Changes to migration correctness, CI, benchmarks, or release versioning.
