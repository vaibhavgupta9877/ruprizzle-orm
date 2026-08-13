//! Metrics facade for `ruprizzle`.
//!
//! All metric calls go through this module so that the feature flag is checked
//! in one place. When the `metrics` feature is disabled, the calls compile to
//! no-ops and add no runtime overhead.

/// Increment a counter by `n`.
#[allow(unused_variables)]
pub fn counter(name: &'static str, n: u64) {
    #[cfg(feature = "metrics")]
    metrics::counter!(name).increment(n);
}

/// Increment a counter with label values.
#[allow(unused_variables)]
pub fn counter_with<K: AsRef<[(&'static str, &'static str)]>>(name: &'static str, labels: K, n: u64) {
    #[cfg(feature = "metrics")]
    metrics::counter!(name, labels.as_ref()).increment(n);
}

/// Record a histogram value.
#[allow(unused_variables)]
pub fn histogram(name: &'static str, value: f64) {
    #[cfg(feature = "metrics")]
    metrics::histogram!(name).record(value);
}

/// Set a gauge value.
#[allow(unused_variables)]
pub fn gauge(name: &'static str, value: f64) {
    #[cfg(feature = "metrics")]
    metrics::gauge!(name).set(value);
}

/// Metric names used across the runtime.
pub mod names {
    /// Total number of queries executed.
    pub const QUERY_TOTAL: &str = "ruprizzle_query_total";
    /// Total number of query errors, labelled by [`Error::kind`](crate::Error::kind).
    pub const QUERY_ERRORS_TOTAL: &str = "ruprizzle_query_errors_total";
    /// Query duration histogram, in seconds.
    pub const QUERY_DURATION_SECONDS: &str = "ruprizzle_query_duration_seconds";
    /// Current pool size gauge.
    pub const POOL_SIZE: &str = "ruprizzle_pool_size";
    /// Current idle connection gauge.
    pub const POOL_IDLE: &str = "ruprizzle_pool_idle";
    /// Current in-use connection gauge.
    pub const POOL_IN_USE: &str = "ruprizzle_pool_in_use";
    /// Current waiter count gauge.
    pub const POOL_WAITERS: &str = "ruprizzle_pool_waiters";
    /// Total number of applied migrations.
    pub const MIGRATION_APPLIED_TOTAL: &str = "ruprizzle_migration_applied_total";
    /// Per-migration duration histogram, in seconds.
    pub const MIGRATION_DURATION_SECONDS: &str = "ruprizzle_migration_duration_seconds";
}

pub use names::*;
