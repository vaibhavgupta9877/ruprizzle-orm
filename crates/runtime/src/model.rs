//! The `Model` trait that generated entities implement.

/// A type that can be decoded from `AnyRow`, `PgRow`, `SqliteRow`, and
/// `rusqlite` rows.
///
/// This is an object-safe-ish bound used by `Executor` so it can return a
/// backend-tagged `RowBatch` and still have the caller decode it. Generated
/// models provide all three `sqlx::FromRow` implementations and (when the
/// `sqlite-rusqlite` feature is enabled) a `crate::rusqlite::FromRusqliteRow`
/// implementation. Hand-written tests can either derive `sqlx::FromRow` (which
/// is generic over `R: Row` for simple scalars) or implement the three impls
/// explicitly; when `sqlite-rusqlite` is on they must also implement
/// `rusqlite::FromRusqliteRow`.
#[cfg(not(feature = "sqlite-rusqlite"))]
pub trait RowDecode:
    Sized
    + Send
    + Sync
    + 'static
    + for<'r> sqlx::FromRow<'r, sqlx::any::AnyRow>
    + for<'r> sqlx::FromRow<'r, sqlx::postgres::PgRow>
    + for<'r> sqlx::FromRow<'r, sqlx::sqlite::SqliteRow>
{
}

/// A type that can be decoded from `AnyRow`, `PgRow`, `SqliteRow`, and
/// `rusqlite` rows.
#[cfg(feature = "sqlite-rusqlite")]
pub trait RowDecode:
    Sized
    + Send
    + Sync
    + 'static
    + for<'r> sqlx::FromRow<'r, sqlx::any::AnyRow>
    + for<'r> sqlx::FromRow<'r, sqlx::postgres::PgRow>
    + for<'r> sqlx::FromRow<'r, sqlx::sqlite::SqliteRow>
    + crate::rusqlite::FromRusqliteRow
{
}

#[cfg(not(feature = "sqlite-rusqlite"))]
impl<T> RowDecode for T where
    T: Sized
        + Send
        + Sync
        + 'static
        + for<'r> sqlx::FromRow<'r, sqlx::any::AnyRow>
        + for<'r> sqlx::FromRow<'r, sqlx::postgres::PgRow>
        + for<'r> sqlx::FromRow<'r, sqlx::sqlite::SqliteRow>
{
}

#[cfg(feature = "sqlite-rusqlite")]
impl<T> RowDecode for T where
    T: Sized
        + Send
        + Sync
        + 'static
        + for<'r> sqlx::FromRow<'r, sqlx::any::AnyRow>
        + for<'r> sqlx::FromRow<'r, sqlx::postgres::PgRow>
        + for<'r> sqlx::FromRow<'r, sqlx::sqlite::SqliteRow>
        + crate::rusqlite::FromRusqliteRow
{
}

/// A generated entity type.
pub trait Model: RowDecode {
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
