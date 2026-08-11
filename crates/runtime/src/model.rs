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

    /// The physical columns of this model, in the order the generated
    /// `FromRow` implementation expects them. An empty slice disables explicit
    /// projections and keeps the old `SELECT *` behaviour for hand-written
    /// model impls.
    const COLUMNS: &'static [&'static str] = &[];
}
