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
    + for<'r> sqlx::FromRow<'r, sqlx::mysql::MySqlRow>
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
    + for<'r> sqlx::FromRow<'r, sqlx::mysql::MySqlRow>
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
    + for<'r> sqlx::FromRow<'r, sqlx::mysql::MySqlRow>
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
    + for<'r> sqlx::FromRow<'r, sqlx::mysql::MySqlRow>
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
        + for<'r> sqlx::FromRow<'r, sqlx::mysql::MySqlRow>
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
        + for<'r> sqlx::FromRow<'r, sqlx::mysql::MySqlRow>
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
        + for<'r> sqlx::FromRow<'r, sqlx::mysql::MySqlRow>
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
        + for<'r> sqlx::FromRow<'r, sqlx::mysql::MySqlRow>
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

    /// The soft-delete timestamp column name, if configured with `@deletedAt`.
    const DELETED_AT_COLUMN: Option<&'static str> = None;

    /// The audit update timestamp column name, if configured with `@updatedAt`.
    const UPDATED_AT_COLUMN: Option<&'static str> = None;
}

/// The soft-delete predicate `<table>.<deleted_at> IS NULL` for `M`, or `None`
/// when `M` has no `@deletedAt` column.
///
/// [`SelectQuery`](crate::SelectQuery) applies this through `effective_filter`.
/// Every other read path that compiles its own SQL for a model (partitioned
/// includes, m2m reloads, join right-hand sides, hierarchy CTEs) must apply it
/// too, or soft-deleted rows leak through that path. `table` is the qualifier to
/// use, which differs from `M::TABLE` under a join alias.
pub(crate) fn live_rows_node<M: Model>(table: &'static str) -> Option<crate::filter::FilterNode> {
    M::DELETED_AT_COLUMN.map(|column| crate::filter::FilterNode::Null {
        table,
        column,
        negated: false,
    })
}

/// [`live_rows_node`] as a `Filter<M>` on `M::TABLE`; the empty filter when `M`
/// is not soft-deletable.
pub(crate) fn live_rows<M: Model>() -> crate::filter::Filter<M> {
    crate::filter::Filter::new(
        live_rows_node::<M>(M::TABLE).unwrap_or_else(|| crate::filter::FilterNode::And(Vec::new())),
    )
}
