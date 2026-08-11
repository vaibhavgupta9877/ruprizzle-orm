//! Decode helpers that work against any `sqlx::Row`.
//!
//! The `sqlx::Any` driver does not implement `sqlx::Decode` for rich types such
//! as `Uuid`, `Decimal`, `DateTime`, `NaiveDate`, `NaiveTime` or JSON values.
//! These helpers work around that by first trying a native decode, then
//! fetching a type the driver *does* understand (`String` or `Vec<u8>`) and
//! parsing the value. They are used by the manually generated
//! `sqlx::FromRow` implementations.

use std::fmt;
use std::str::FromStr;

use sqlx::{ColumnIndex, Row};

/// Error wrapping a parse failure so it can be returned as a `sqlx::Error`.
#[derive(Debug)]
struct DecodeTextError(String);

impl fmt::Display for DecodeTextError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for DecodeTextError {}

fn decode_text_error<E: fmt::Display>(e: E) -> sqlx::Error {
    sqlx::Error::Decode(Box::new(DecodeTextError(e.to_string())))
}

fn decode_text<R, T>(row: &R, col: &str) -> Result<T, sqlx::Error>
where
    R: Row,
    usize: ColumnIndex<R>,
    for<'a> &'a str: ColumnIndex<R>,
    T: FromStr,
    T::Err: fmt::Display + Send + Sync + 'static,
    String: for<'r> sqlx::Decode<'r, R::Database> + sqlx::Type<R::Database>,
    Vec<u8>: for<'r> sqlx::Decode<'r, R::Database> + sqlx::Type<R::Database>,
{
    if let Ok(s) = row.try_get::<String, _>(col) {
        return s.parse().map_err(decode_text_error);
    }
    let bytes: Vec<u8> = row.try_get(col)?;
    let s = String::from_utf8(bytes).map_err(decode_text_error)?;
    s.parse().map_err(decode_text_error)
}

fn decode_text_idx<R, T>(row: &R, idx: usize) -> Result<T, sqlx::Error>
where
    R: Row,
    usize: ColumnIndex<R>,
    for<'a> &'a str: ColumnIndex<R>,
    T: FromStr,
    T::Err: fmt::Display + Send + Sync + 'static,
    String: for<'r> sqlx::Decode<'r, R::Database> + sqlx::Type<R::Database>,
    Vec<u8>: for<'r> sqlx::Decode<'r, R::Database> + sqlx::Type<R::Database>,
{
    if let Ok(s) = row.try_get::<String, _>(idx) {
        return s.parse().map_err(decode_text_error);
    }
    let bytes: Vec<u8> = row.try_get(idx)?;
    let s = String::from_utf8(bytes).map_err(decode_text_error)?;
    s.parse().map_err(decode_text_error)
}

/// Decode a column as text and then parse it into `T`.
///
/// The fallback is needed for `Any` and SQLite, which do not implement
/// `sqlx::Decode` for rich types such as `Uuid` or `Decimal`. The generic
/// signature lets the same generated `FromRow` work for `AnyRow`, `PgRow`
/// and `SqliteRow`; once per-backend `FromRow` is in place Postgres can move
/// these to `direct` instead.
pub fn text<R, T>(row: &R, col: &str) -> Result<T, sqlx::Error>
where
    R: Row,
    usize: ColumnIndex<R>,
    for<'a> &'a str: ColumnIndex<R>,
    T: FromStr,
    T::Err: fmt::Display + Send + Sync + 'static,
    String: for<'r> sqlx::Decode<'r, R::Database> + sqlx::Type<R::Database>,
    Vec<u8>: for<'r> sqlx::Decode<'r, R::Database> + sqlx::Type<R::Database>,
{
    decode_text(row, col)
}

/// Ordinal version of [`text`].
pub fn text_idx<R, T>(row: &R, idx: usize) -> Result<T, sqlx::Error>
where
    R: Row,
    usize: ColumnIndex<R>,
    for<'a> &'a str: ColumnIndex<R>,
    T: FromStr,
    T::Err: fmt::Display + Send + Sync + 'static,
    String: for<'r> sqlx::Decode<'r, R::Database> + sqlx::Type<R::Database>,
    Vec<u8>: for<'r> sqlx::Decode<'r, R::Database> + sqlx::Type<R::Database>,
{
    decode_text_idx(row, idx)
}

/// Decode an optional rich-typed column.
pub fn text_opt<R, T>(row: &R, col: &str) -> Result<Option<T>, sqlx::Error>
where
    R: Row,
    usize: ColumnIndex<R>,
    for<'a> &'a str: ColumnIndex<R>,
    T: FromStr,
    T::Err: fmt::Display + Send + Sync + 'static,
    String: for<'r> sqlx::Decode<'r, R::Database> + sqlx::Type<R::Database>,
    Vec<u8>: for<'r> sqlx::Decode<'r, R::Database> + sqlx::Type<R::Database>,
{
    match row.try_get::<Option<String>, _>(col) {
        Ok(Some(s)) => s.parse::<T>().map(Some).map_err(decode_text_error),
        Ok(None) => Ok(None),
        Err(_) => match row.try_get::<Option<Vec<u8>>, _>(col) {
            Ok(Some(bytes)) => {
                let s = String::from_utf8(bytes).map_err(decode_text_error)?;
                s.parse().map(Some).map_err(decode_text_error)
            }
            Ok(None) => Ok(None),
            Err(e) => Err(e),
        },
    }
}

/// Ordinal version of [`text_opt`].
pub fn text_opt_idx<R, T>(row: &R, idx: usize) -> Result<Option<T>, sqlx::Error>
where
    R: Row,
    usize: ColumnIndex<R>,
    for<'a> &'a str: ColumnIndex<R>,
    T: FromStr,
    T::Err: fmt::Display + Send + Sync + 'static,
    String: for<'r> sqlx::Decode<'r, R::Database> + sqlx::Type<R::Database>,
    Vec<u8>: for<'r> sqlx::Decode<'r, R::Database> + sqlx::Type<R::Database>,
{
    match row.try_get::<Option<String>, _>(idx) {
        Ok(Some(s)) => s.parse::<T>().map(Some).map_err(decode_text_error),
        Ok(None) => Ok(None),
        Err(_) => match row.try_get::<Option<Vec<u8>>, _>(idx) {
            Ok(Some(bytes)) => {
                let s = String::from_utf8(bytes).map_err(decode_text_error)?;
                s.parse().map(Some).map_err(decode_text_error)
            }
            Ok(None) => Ok(None),
            Err(e) => Err(e),
        },
    }
}

/// Decode a JSON column.
pub fn json<R>(row: &R, col: &str) -> Result<serde_json::Value, sqlx::Error>
where
    R: Row,
    usize: ColumnIndex<R>,
    for<'a> &'a str: ColumnIndex<R>,
    String: for<'r> sqlx::Decode<'r, R::Database> + sqlx::Type<R::Database>,
{
    let s: String = row.try_get(col)?;
    serde_json::from_str(&s).map_err(|e| sqlx::Error::Decode(Box::new(e)))
}

/// Ordinal version of [`json`].
pub fn json_idx<R>(row: &R, idx: usize) -> Result<serde_json::Value, sqlx::Error>
where
    R: Row,
    usize: ColumnIndex<R>,
    for<'a> &'a str: ColumnIndex<R>,
    String: for<'r> sqlx::Decode<'r, R::Database> + sqlx::Type<R::Database>,
{
    let s: String = row.try_get(idx)?;
    serde_json::from_str(&s).map_err(|e| sqlx::Error::Decode(Box::new(e)))
}

/// Decode an optional JSON column.
pub fn json_opt<R>(row: &R, col: &str) -> Result<Option<serde_json::Value>, sqlx::Error>
where
    R: Row,
    usize: ColumnIndex<R>,
    for<'a> &'a str: ColumnIndex<R>,
    String: for<'r> sqlx::Decode<'r, R::Database> + sqlx::Type<R::Database>,
{
    match row.try_get::<Option<String>, _>(col) {
        Ok(Some(s)) => serde_json::from_str(&s)
            .map(Some)
            .map_err(|e| sqlx::Error::Decode(Box::new(e))),
        Ok(None) => Ok(None),
        Err(e) => Err(e),
    }
}

/// Ordinal version of [`json_opt`].
pub fn json_opt_idx<R>(row: &R, idx: usize) -> Result<Option<serde_json::Value>, sqlx::Error>
where
    R: Row,
    usize: ColumnIndex<R>,
    for<'a> &'a str: ColumnIndex<R>,
    String: for<'r> sqlx::Decode<'r, R::Database> + sqlx::Type<R::Database>,
{
    match row.try_get::<Option<String>, _>(idx) {
        Ok(Some(s)) => serde_json::from_str(&s)
            .map(Some)
            .map_err(|e| sqlx::Error::Decode(Box::new(e))),
        Ok(None) => Ok(None),
        Err(e) => Err(e),
    }
}

/// Decode a byte-blob column.
pub fn bytes<R>(row: &R, col: &str) -> Result<Vec<u8>, sqlx::Error>
where
    R: Row,
    usize: ColumnIndex<R>,
    for<'a> &'a str: ColumnIndex<R>,
    Vec<u8>: for<'r> sqlx::Decode<'r, R::Database> + sqlx::Type<R::Database>,
{
    row.try_get(col)
}

/// Ordinal version of [`bytes`].
pub fn bytes_idx<R>(row: &R, idx: usize) -> Result<Vec<u8>, sqlx::Error>
where
    R: Row,
    usize: ColumnIndex<R>,
    for<'a> &'a str: ColumnIndex<R>,
    Vec<u8>: for<'r> sqlx::Decode<'r, R::Database> + sqlx::Type<R::Database>,
{
    row.try_get(idx)
}

/// Decode an optional byte-blob column.
pub fn bytes_opt<R>(row: &R, col: &str) -> Result<Option<Vec<u8>>, sqlx::Error>
where
    R: Row,
    usize: ColumnIndex<R>,
    for<'a> &'a str: ColumnIndex<R>,
    Vec<u8>: for<'r> sqlx::Decode<'r, R::Database> + sqlx::Type<R::Database>,
{
    row.try_get(col)
}

/// Ordinal version of [`bytes_opt`].
pub fn bytes_opt_idx<R>(row: &R, idx: usize) -> Result<Option<Vec<u8>>, sqlx::Error>
where
    R: Row,
    usize: ColumnIndex<R>,
    for<'a> &'a str: ColumnIndex<R>,
    Vec<u8>: for<'r> sqlx::Decode<'r, R::Database> + sqlx::Type<R::Database>,
{
    row.try_get(idx)
}

/// Decode a column whose type the driver already understands.
pub fn direct<R, T>(row: &R, col: &str) -> Result<T, sqlx::Error>
where
    R: Row,
    usize: ColumnIndex<R>,
    for<'a> &'a str: ColumnIndex<R>,
    T: for<'r> sqlx::Decode<'r, R::Database> + sqlx::Type<R::Database>,
{
    row.try_get(col)
}

/// Ordinal version of [`direct`].
pub fn direct_idx<R, T>(row: &R, idx: usize) -> Result<T, sqlx::Error>
where
    R: Row,
    usize: ColumnIndex<R>,
    for<'a> &'a str: ColumnIndex<R>,
    T: for<'r> sqlx::Decode<'r, R::Database> + sqlx::Type<R::Database>,
{
    row.try_get(idx)
}

/// Decode an optional column whose type the driver already understands.
pub fn direct_opt<R, T>(row: &R, col: &str) -> Result<Option<T>, sqlx::Error>
where
    R: Row,
    usize: ColumnIndex<R>,
    for<'a> &'a str: ColumnIndex<R>,
    T: for<'r> sqlx::Decode<'r, R::Database> + sqlx::Type<R::Database>,
{
    row.try_get(col)
}

/// Ordinal version of [`direct_opt`].
pub fn direct_opt_idx<R, T>(row: &R, idx: usize) -> Result<Option<T>, sqlx::Error>
where
    R: Row,
    usize: ColumnIndex<R>,
    for<'a> &'a str: ColumnIndex<R>,
    T: for<'r> sqlx::Decode<'r, R::Database> + sqlx::Type<R::Database>,
{
    row.try_get(idx)
}

/// Decode a boolean column.
///
/// SQLite stores booleans as `INTEGER` (0/1), while Postgres has a native
/// `BOOL` type, so this helper tries the native `bool` path first and falls
/// back to the integer path.
pub fn boolean<R>(row: &R, col: &str) -> Result<bool, sqlx::Error>
where
    R: Row,
    usize: ColumnIndex<R>,
    for<'a> &'a str: ColumnIndex<R>,
    bool: for<'r> sqlx::Decode<'r, R::Database> + sqlx::Type<R::Database>,
    i64: for<'r> sqlx::Decode<'r, R::Database> + sqlx::Type<R::Database>,
{
    if let Ok(b) = row.try_get::<bool, _>(col) {
        return Ok(b);
    }
    let i: i64 = row.try_get(col)?;
    Ok(i != 0)
}

/// Ordinal version of [`boolean`].
pub fn boolean_idx<R>(row: &R, idx: usize) -> Result<bool, sqlx::Error>
where
    R: Row,
    usize: ColumnIndex<R>,
    for<'a> &'a str: ColumnIndex<R>,
    bool: for<'r> sqlx::Decode<'r, R::Database> + sqlx::Type<R::Database>,
    i64: for<'r> sqlx::Decode<'r, R::Database> + sqlx::Type<R::Database>,
{
    if let Ok(b) = row.try_get::<bool, _>(idx) {
        return Ok(b);
    }
    let i: i64 = row.try_get(idx)?;
    Ok(i != 0)
}

/// Decode an optional boolean column.
pub fn boolean_opt<R>(row: &R, col: &str) -> Result<Option<bool>, sqlx::Error>
where
    R: Row,
    usize: ColumnIndex<R>,
    for<'a> &'a str: ColumnIndex<R>,
    bool: for<'r> sqlx::Decode<'r, R::Database> + sqlx::Type<R::Database>,
    i64: for<'r> sqlx::Decode<'r, R::Database> + sqlx::Type<R::Database>,
{
    match row.try_get::<Option<bool>, _>(col) {
        Ok(Some(b)) => Ok(Some(b)),
        Ok(None) => Ok(None),
        Err(_) => match row.try_get::<Option<i64>, _>(col) {
            Ok(Some(i)) => Ok(Some(i != 0)),
            Ok(None) => Ok(None),
            Err(e) => Err(e),
        },
    }
}

/// Ordinal version of [`boolean_opt`].
pub fn boolean_opt_idx<R>(row: &R, idx: usize) -> Result<Option<bool>, sqlx::Error>
where
    R: Row,
    usize: ColumnIndex<R>,
    for<'a> &'a str: ColumnIndex<R>,
    bool: for<'r> sqlx::Decode<'r, R::Database> + sqlx::Type<R::Database>,
    i64: for<'r> sqlx::Decode<'r, R::Database> + sqlx::Type<R::Database>,
{
    match row.try_get::<Option<bool>, _>(idx) {
        Ok(Some(b)) => Ok(Some(b)),
        Ok(None) => Ok(None),
        Err(_) => match row.try_get::<Option<i64>, _>(idx) {
            Ok(Some(i)) => Ok(Some(i != 0)),
            Ok(None) => Ok(None),
            Err(e) => Err(e),
        },
    }
}
