//! The `Model` trait that generated entities implement.

/// A type that can be decoded from `AnyRow`, `PgRow`, `SqliteRow`,
/// `rusqlite` rows, and (when enabled) `tokio-postgres` rows.
///
/// This is an object-safe-ish bound used by `Executor` so it can return a
/// backend-tagged `RowBatch` and still have the caller decode it. Generated
/// models provide all three `sqlx::FromRow` implementations and optional
/// `FromRusqliteRow` / `FromTokioPostgresRow` implementations. Hand-written
/// tests can either derive `sqlx::FromRow` (which is generic over `R: Row` for
/// simple scalars) or implement the concrete backend traits.
#[cfg(all(
    not(feature = "sqlite-rusqlite"),
    not(feature = "postgres-tokio-postgres")
))]
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

/// Row decode bound when the `sqlite-rusqlite` backend is enabled but not the
/// `tokio-postgres` backend.
#[cfg(all(feature = "sqlite-rusqlite", not(feature = "postgres-tokio-postgres")))]
pub trait RowDecode:
    Sized
    + Send
    + Sync
    + 'static
    + for<'r> sqlx::FromRow<'r, sqlx::any::AnyRow>
    + for<'r> sqlx::FromRow<'r, sqlx::postgres::PgRow>
    + for<'r> sqlx::FromRow<'r, sqlx::sqlite::SqliteRow>
    + crate::rusqlite::FromRusqliteRow
    + crate::rusqlite::FromOwnedRow
{
}

/// Row decode bound when the `tokio-postgres` backend is enabled but not the
/// `sqlite-rusqlite` backend.
#[cfg(all(not(feature = "sqlite-rusqlite"), feature = "postgres-tokio-postgres"))]
pub trait RowDecode:
    Sized
    + Send
    + Sync
    + 'static
    + for<'r> sqlx::FromRow<'r, sqlx::any::AnyRow>
    + for<'r> sqlx::FromRow<'r, sqlx::postgres::PgRow>
    + for<'r> sqlx::FromRow<'r, sqlx::sqlite::SqliteRow>
    + crate::tokio_postgres::FromTokioPostgresRow
{
}

/// Row decode bound when both `sqlite-rusqlite` and `tokio-postgres` native
/// backends are enabled.
#[cfg(all(feature = "sqlite-rusqlite", feature = "postgres-tokio-postgres"))]
pub trait RowDecode:
    Sized
    + Send
    + Sync
    + 'static
    + for<'r> sqlx::FromRow<'r, sqlx::any::AnyRow>
    + for<'r> sqlx::FromRow<'r, sqlx::postgres::PgRow>
    + for<'r> sqlx::FromRow<'r, sqlx::sqlite::SqliteRow>
    + crate::rusqlite::FromRusqliteRow
    + crate::rusqlite::FromOwnedRow
    + crate::tokio_postgres::FromTokioPostgresRow
{
}

#[cfg(all(
    not(feature = "sqlite-rusqlite"),
    not(feature = "postgres-tokio-postgres")
))]
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

#[cfg(all(feature = "sqlite-rusqlite", not(feature = "postgres-tokio-postgres")))]
impl<T> RowDecode for T where
    T: Sized
        + Send
        + Sync
        + 'static
        + for<'r> sqlx::FromRow<'r, sqlx::any::AnyRow>
        + for<'r> sqlx::FromRow<'r, sqlx::postgres::PgRow>
        + for<'r> sqlx::FromRow<'r, sqlx::sqlite::SqliteRow>
        + crate::rusqlite::FromRusqliteRow
        + crate::rusqlite::FromOwnedRow
{
}

#[cfg(all(not(feature = "sqlite-rusqlite"), feature = "postgres-tokio-postgres"))]
impl<T> RowDecode for T where
    T: Sized
        + Send
        + Sync
        + 'static
        + for<'r> sqlx::FromRow<'r, sqlx::any::AnyRow>
        + for<'r> sqlx::FromRow<'r, sqlx::postgres::PgRow>
        + for<'r> sqlx::FromRow<'r, sqlx::sqlite::SqliteRow>
        + crate::tokio_postgres::FromTokioPostgresRow
{
}

#[cfg(all(feature = "sqlite-rusqlite", feature = "postgres-tokio-postgres"))]
impl<T> RowDecode for T where
    T: Sized
        + Send
        + Sync
        + 'static
        + for<'r> sqlx::FromRow<'r, sqlx::any::AnyRow>
        + for<'r> sqlx::FromRow<'r, sqlx::postgres::PgRow>
        + for<'r> sqlx::FromRow<'r, sqlx::sqlite::SqliteRow>
        + crate::rusqlite::FromRusqliteRow
        + crate::rusqlite::FromOwnedRow
        + crate::tokio_postgres::FromTokioPostgresRow
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
