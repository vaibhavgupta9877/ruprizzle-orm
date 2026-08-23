//! OpenTelemetry Semantic Spans and Metrics 2.0 Tests.

use ruprizzle::compile::sanitize_sql;
use ruprizzle::metrics::names::*;

#[test]
fn sql_sanitization_pii_safety() {
    let raw_sql = "SELECT * FROM users WHERE email = 'john.doe@secret.com' AND name = 'John O\\'Connor' AND age > 25";
    let clean_sql = sanitize_sql(raw_sql);

    assert!(!clean_sql.contains("john.doe@secret.com"));
    assert_eq!(
        clean_sql,
        "SELECT * FROM users WHERE email = '?' AND name = '?' AND age > 25"
    );
}

#[test]
fn prometheus_metrics_constants() {
    assert_eq!(QUERY_TOTAL, "ruprizzle_query_total");
    assert_eq!(QUERY_DURATION_SECONDS, "ruprizzle_query_duration_seconds");
    assert_eq!(SLOW_QUERIES_TOTAL, "ruprizzle_slow_queries_total");
    assert_eq!(ROWS_AFFECTED_TOTAL, "ruprizzle_rows_affected_total");
    assert_eq!(POOL_CONNECTIONS_ACTIVE, "ruprizzle_pool_connections_active");
    assert_eq!(POOL_CONNECTIONS_IDLE, "ruprizzle_pool_connections_idle");
    assert_eq!(
        POOL_WAIT_DURATION_SECONDS,
        "ruprizzle_pool_wait_duration_seconds"
    );
    assert_eq!(CACHE_HITS_TOTAL, "ruprizzle_cache_hits_total");
    assert_eq!(CACHE_MISSES_TOTAL, "ruprizzle_cache_misses_total");
    assert_eq!(REPLICA_ROUTING_TOTAL, "ruprizzle_replica_routing_total");
}
