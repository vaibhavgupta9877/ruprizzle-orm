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

use ruprizzle::sqlx::{ColumnIndex, Decode, Row, Type};
use ruprizzle::{Executor, Pool, RowBatch, Value};
use ruprizzle_core::ir::Provider;

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
