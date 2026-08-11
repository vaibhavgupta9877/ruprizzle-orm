//! The `Model` trait that generated entities implement.

/// A generated entity type.
pub trait Model: Sized + Send + Sync + 'static {
    /// The table this model maps to.
    const TABLE: &'static str;

    /// The primary-key column.
    ///
    /// Cursor pagination and streaming append this to `ORDER BY` so the total
    /// order is deterministic: without a unique tiebreaker, two rows sharing an
    /// ordering value can appear on two consecutive pages or be skipped
    /// entirely. Defaults to `"id"` so hand-written `Model` impls in tests stay
    /// valid; generated code always sets it explicitly.
    const PRIMARY_KEY: &'static str = "id";
}
