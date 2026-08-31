//! Real database access for Studio handlers.
//!
//! Every Studio screen that claims to show database state goes through this
//! module. There is deliberately no fallback that invents rows: when there is no
//! connection, or a statement fails, the caller gets an [`Err`] and is expected to
//! render the failure rather than something that looks like data.
//!
//! Values come back as `Option<String>` because Studio renders text. The SQL asks
//! the database for text (see [`to_text`]) instead of guessing at Rust types, which
//! keeps one code path across Postgres, `SQLite` and `MySQL`.

use std::borrow::Cow;

use ruprizzle::sqlx::{Column, ColumnIndex, Decode, Row, Type};
use ruprizzle::{Executor, Pool, RowBatch, Value};
use ruprizzle_core::ir::{Field, FieldKind, Provider, ScalarType};
use ruprizzle_dialect::DbDialect;

/// One decoded result row: every column rendered as text, `None` for SQL `NULL`.
pub type TextRow = Vec<Option<String>>;

/// Wraps `expr` in a cast to text for `provider`.
///
/// `DbDialect::cast_expr` is not used here: its `MySQL` mapping for
/// `ScalarType::String` is `CHAR(255)`, which would silently truncate a longer
/// value on the way to the browser.
#[must_use]
pub fn to_text(provider: Provider, expr: &str) -> String {
    match provider {
        Provider::Postgres => format!("({expr})::text"),
        Provider::Sqlite => format!("CAST({expr} AS TEXT)"),
        Provider::Mysql => format!("CAST({expr} AS CHAR)"),
    }
}

/// Runs `sql` and decodes the first `width` columns of every row as text.
///
/// # Errors
///
/// Returns a display message if the statement fails or a row cannot be decoded.
pub async fn fetch_text_rows(
    pool: &Pool,
    sql: String,
    binds: Vec<Value>,
    width: usize,
) -> Result<Vec<TextRow>, String> {
    let batch = pool
        .fetch_all_raw(Cow::Owned(sql), binds)
        .await
        .map_err(|e| e.to_string())?;

    match batch {
        RowBatch::Any(rows) => rows.iter().map(|row| row_cells(row, width)).collect(),
        RowBatch::Postgres(rows) => rows.iter().map(|row| row_cells(row, width)).collect(),
        RowBatch::Sqlite(rows) => rows.iter().map(|row| row_cells(row, width)).collect(),
        RowBatch::Mysql(rows) => rows.iter().map(|row| row_cells(row, width)).collect(),
        _ => Err(
            "this database driver is not supported by Studio; use the default sqlx driver"
                .to_string(),
        ),
    }
}

/// Runs `sql` and returns its single integer result.
///
/// # Errors
///
/// Returns a display message if the statement fails or returns no usable value.
pub async fn fetch_count(pool: &Pool, sql: String, binds: Vec<Value>) -> Result<i64, String> {
    let rows = fetch_text_rows(pool, sql, binds, 1).await?;
    rows.first()
        .and_then(|row| row.first().cloned().flatten())
        .ok_or_else(|| "count query returned no rows".to_string())
        .and_then(|text| {
            text.parse::<i64>()
                .map_err(|_| format!("count query returned `{text}`, which is not a number"))
        })
}

/// Runs a statement and returns the number of affected rows.
///
/// # Errors
///
/// Returns a display message if the statement fails.
pub async fn execute(pool: &Pool, sql: String, binds: Vec<Value>) -> Result<u64, String> {
    pool.execute_raw(Cow::Owned(sql), binds)
        .await
        .map_err(|e| e.to_string())
}

/// Runs `sql` and returns its column names alongside every row rendered as text.
///
/// Unlike [`fetch_text_rows`] the shape is not known ahead of time, so each cell is
/// decoded through a fallback chain. A cell no chain member can read renders as
/// `<unreadable>` rather than failing the whole query — the user still sees the rest
/// of their result set, and sees plainly which cell Studio could not read.
///
/// Column names come from the first row, so an empty result set reports no columns.
///
/// # Errors
///
/// Returns a display message if the statement fails.
pub async fn fetch_dynamic(
    pool: &Pool,
    sql: String,
    binds: Vec<Value>,
) -> Result<(Vec<String>, Vec<TextRow>), String> {
    let batch = pool
        .fetch_all_raw(Cow::Owned(sql), binds)
        .await
        .map_err(|e| e.to_string())?;

    match batch {
        RowBatch::Any(rows) => Ok(dynamic_rows(&rows)),
        RowBatch::Postgres(rows) => Ok(dynamic_rows(&rows)),
        RowBatch::Sqlite(rows) => Ok(dynamic_rows(&rows)),
        RowBatch::Mysql(rows) => Ok(dynamic_rows(&rows)),
        _ => Err(
            "this database driver is not supported by Studio; use the default sqlx driver"
                .to_string(),
        ),
    }
}

/// Builds the SQL expression that binds a user-supplied text value into `field`.
///
/// Studio's editors are text inputs, so every value arrives as a string. Binding a
/// string straight into an `INTEGER` column fails on Postgres, so the placeholder is
/// forced to text and then cast to the field's own scalar type. String-shaped fields
/// are bound directly, because `MySQL`'s `CHAR(255)` cast would truncate them.
#[must_use]
pub fn bind_expr(
    dialect: &dyn DbDialect,
    provider: Provider,
    index: usize,
    field: &Field,
) -> String {
    let placeholder = dialect.placeholder(index);
    match scalar_of(field) {
        Some(ScalarType::String) | None => placeholder,
        Some(ty) => dialect.cast_expr(&to_text(provider, &placeholder), ty),
    }
}

/// The scalar type behind a field, if it has one.
///
/// Lists and relations return `None`: Studio's text editors cannot represent them,
/// and callers use that to keep those cells read-only.
#[must_use]
pub fn scalar_of(field: &Field) -> Option<ScalarType> {
    match &field.kind {
        FieldKind::Scalar(ty) => Some(*ty),
        // An enum column takes a text literal.
        FieldKind::Enum(_) => Some(ScalarType::String),
        FieldKind::List(_) | FieldKind::Relation(_) => None,
    }
}

/// Whether Studio can edit `field` through a plain text input.
#[must_use]
pub fn is_editable(field: &Field) -> bool {
    field.has_column() && scalar_of(field).is_some()
}

fn dynamic_rows<R>(rows: &[R]) -> (Vec<String>, Vec<TextRow>)
where
    R: Row,
    usize: ColumnIndex<R>,
    String: for<'r> Decode<'r, R::Database> + Type<R::Database>,
    i64: for<'r> Decode<'r, R::Database> + Type<R::Database>,
    f64: for<'r> Decode<'r, R::Database> + Type<R::Database>,
    bool: for<'r> Decode<'r, R::Database> + Type<R::Database>,
{
    let Some(first) = rows.first() else {
        return (Vec::new(), Vec::new());
    };
    let names: Vec<String> = first
        .columns()
        .iter()
        .map(|c| c.name().to_string())
        .collect();
    let width = names.len();

    let cells = rows
        .iter()
        .map(|row| (0..width).map(|idx| cell_text(row, idx)).collect())
        .collect();

    (names, cells)
}

fn cell_text<R>(row: &R, idx: usize) -> Option<String>
where
    R: Row,
    usize: ColumnIndex<R>,
    String: for<'r> Decode<'r, R::Database> + Type<R::Database>,
    i64: for<'r> Decode<'r, R::Database> + Type<R::Database>,
    f64: for<'r> Decode<'r, R::Database> + Type<R::Database>,
    bool: for<'r> Decode<'r, R::Database> + Type<R::Database>,
{
    if let Ok(value) = row.try_get::<Option<String>, _>(idx) {
        return value;
    }
    if let Ok(value) = row.try_get::<Option<i64>, _>(idx) {
        return value.map(|v| v.to_string());
    }
    if let Ok(value) = row.try_get::<Option<f64>, _>(idx) {
        return value.map(|v| v.to_string());
    }
    if let Ok(value) = row.try_get::<Option<bool>, _>(idx) {
        return value.map(|v| v.to_string());
    }
    Some("<unreadable>".to_string())
}

/// Counts every row in `table`.
///
/// # Errors
///
/// Returns a display message if the statement fails.
pub async fn count_rows(pool: &Pool, provider: Provider, table: &str) -> Result<i64, String> {
    let ident = quote_ident(provider, table);
    let count = to_text(provider, "COUNT(*)");
    fetch_count(pool, format!("SELECT {count} FROM {ident}"), Vec::new()).await
}

/// Counts the rows of `table` where `column` is not `NULL`.
///
/// # Errors
///
/// Returns a display message if the statement fails.
pub async fn count_non_null(
    pool: &Pool,
    provider: Provider,
    table: &str,
    column: &str,
) -> Result<i64, String> {
    let table_ident = quote_ident(provider, table);
    let column_ident = quote_ident(provider, column);
    let count = to_text(provider, "COUNT(*)");
    fetch_count(
        pool,
        format!("SELECT {count} FROM {table_ident} WHERE {column_ident} IS NOT NULL"),
        Vec::new(),
    )
    .await
}

/// Counts the rows of `table` where `column` is `NULL`.
///
/// # Errors
///
/// Returns a display message if the statement fails.
pub async fn count_null(
    pool: &Pool,
    provider: Provider,
    table: &str,
    column: &str,
) -> Result<i64, String> {
    let table_ident = quote_ident(provider, table);
    let column_ident = quote_ident(provider, column);
    let count = to_text(provider, "COUNT(*)");
    fetch_count(
        pool,
        format!("SELECT {count} FROM {table_ident} WHERE {column_ident} IS NULL"),
        Vec::new(),
    )
    .await
}

/// Quotes an identifier for `provider`, escaping any embedded quote character.
///
/// Identifiers reaching this module come from the parsed schema and from database
/// introspection, never straight from a request path, but they are quoted anyway so
/// that a table named after a keyword still works.
#[must_use]
pub fn quote_ident(provider: Provider, ident: &str) -> String {
    match provider {
        Provider::Mysql => format!("`{}`", ident.replace('`', "``")),
        Provider::Postgres | Provider::Sqlite => format!("\"{}\"", ident.replace('"', "\"\"")),
    }
}

fn row_cells<R>(row: &R, width: usize) -> Result<TextRow, String>
where
    R: Row,
    usize: ColumnIndex<R>,
    String: for<'r> Decode<'r, R::Database> + Type<R::Database>,
    i64: for<'r> Decode<'r, R::Database> + Type<R::Database>,
{
    (0..width)
        .map(|idx| match row.try_get::<Option<String>, _>(idx) {
            Ok(value) => Ok(value),
            // `sqlx::Any` will not hand a numeric column over as text; fall back
            // rather than failing the whole page on an unexpected column type.
            Err(text_error) => match row.try_get::<Option<i64>, _>(idx) {
                Ok(value) => Ok(value.map(|value| value.to_string())),
                Err(_) => Err(text_error.to_string()),
            },
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_cast_is_provider_specific() {
        assert_eq!(to_text(Provider::Postgres, "COUNT(*)"), "(COUNT(*))::text");
        assert_eq!(
            to_text(Provider::Sqlite, "COUNT(*)"),
            "CAST(COUNT(*) AS TEXT)"
        );
        // Deliberately unbounded: `CHAR(255)` would truncate.
        assert_eq!(
            to_text(Provider::Mysql, "COUNT(*)"),
            "CAST(COUNT(*) AS CHAR)"
        );
    }

    #[test]
    fn identifiers_are_quoted_and_escaped() {
        assert_eq!(quote_ident(Provider::Postgres, "order"), "\"order\"");
        assert_eq!(quote_ident(Provider::Mysql, "order"), "`order`");
        assert_eq!(quote_ident(Provider::Postgres, "a\"b"), "\"a\"\"b\"");
        assert_eq!(quote_ident(Provider::Mysql, "a`b"), "`a``b`");
    }
}
