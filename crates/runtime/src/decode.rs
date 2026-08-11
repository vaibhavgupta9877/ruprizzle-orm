//! Decode helpers for `sqlx::any::AnyRow`.
//!
//! The `sqlx::Any` driver does not implement `sqlx::Decode` for rich types such
//! as `Uuid`, `Decimal`, `DateTime`, `NaiveDate`, `NaiveTime` or JSON values.
//! These helpers work around that by first fetching a type `Any` *does*
//! understand (`String` or `Vec<u8>`) and then parsing the value. They are used
//! by the manually generated `sqlx::FromRow` implementations.

use std::fmt;
use std::str::FromStr;

use sqlx::{Row, any::AnyRow};

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

/// Decode a column as text and then parse it into `T`.
///
/// This first tries `String` (the SQLite path for most rich types) and then
/// falls back to `Vec<u8>` (the Postgres path for `UUID` / `BYTEA`).
pub fn text<T>(row: &AnyRow, col: &str) -> Result<T, sqlx::Error>
where
    T: FromStr,
    T::Err: fmt::Display + Send + Sync + 'static,
{
    if let Ok(s) = row.try_get::<String, _>(col) {
        return s.parse().map_err(decode_text_error);
    }
    let bytes: Vec<u8> = row.try_get(col)?;
    let s = String::from_utf8(bytes).map_err(decode_text_error)?;
    s.parse().map_err(decode_text_error)
}

/// Ordinal version of [`text`].
pub fn text_idx<T>(row: &AnyRow, idx: usize) -> Result<T, sqlx::Error>
where
    T: FromStr,
    T::Err: fmt::Display + Send + Sync + 'static,
{
    if let Ok(s) = row.try_get::<String, _>(idx) {
        return s.parse().map_err(decode_text_error);
    }
    let bytes: Vec<u8> = row.try_get(idx)?;
    let s = String::from_utf8(bytes).map_err(decode_text_error)?;
    s.parse().map_err(decode_text_error)
}

/// Decode an optional rich-typed column.
pub fn text_opt<T>(row: &AnyRow, col: &str) -> Result<Option<T>, sqlx::Error>
where
    T: FromStr,
    T::Err: fmt::Display + Send + Sync + 'static,
{
    match row.try_get::<Option<String>, _>(col) {
        Ok(Some(s)) => s.parse::<T>().map(Some).map_err(decode_text_error),
        Ok(None) => Ok(None),
        Err(_) => match row.try_get::<Option<Vec<u8>>, _>(col) {
            Ok(Some(bytes)) => {
                let s = String::from_utf8(bytes).map_err(decode_text_error)?;
                s.parse::<T>().map(Some).map_err(decode_text_error)
            }
            Ok(None) => Ok(None),
            Err(e) => Err(e),
        },
    }
}

/// Ordinal version of [`text_opt`].
pub fn text_opt_idx<T>(row: &AnyRow, idx: usize) -> Result<Option<T>, sqlx::Error>
where
    T: FromStr,
    T::Err: fmt::Display + Send + Sync + 'static,
{
    match row.try_get::<Option<String>, _>(idx) {
        Ok(Some(s)) => s.parse::<T>().map(Some).map_err(decode_text_error),
        Ok(None) => Ok(None),
        Err(_) => match row.try_get::<Option<Vec<u8>>, _>(idx) {
            Ok(Some(bytes)) => {
                let s = String::from_utf8(bytes).map_err(decode_text_error)?;
                s.parse::<T>().map(Some).map_err(decode_text_error)
            }
            Ok(None) => Ok(None),
            Err(e) => Err(e),
        },
    }
}

/// Decode a JSON column.
pub fn json(row: &AnyRow, col: &str) -> Result<serde_json::Value, sqlx::Error> {
    let s: String = row.try_get(col)?;
    serde_json::from_str(&s).map_err(|e| sqlx::Error::Decode(Box::new(e)))
}

/// Ordinal version of [`json`].
pub fn json_idx(row: &AnyRow, idx: usize) -> Result<serde_json::Value, sqlx::Error> {
    let s: String = row.try_get(idx)?;
    serde_json::from_str(&s).map_err(|e| sqlx::Error::Decode(Box::new(e)))
}

/// Decode an optional JSON column.
pub fn json_opt(row: &AnyRow, col: &str) -> Result<Option<serde_json::Value>, sqlx::Error> {
    match row.try_get::<Option<String>, _>(col) {
        Ok(Some(s)) => serde_json::from_str(&s)
            .map(Some)
            .map_err(|e| sqlx::Error::Decode(Box::new(e))),
        Ok(None) => Ok(None),
        Err(e) => Err(e),
    }
}

/// Ordinal version of [`json_opt`].
pub fn json_opt_idx(row: &AnyRow, idx: usize) -> Result<Option<serde_json::Value>, sqlx::Error> {
    match row.try_get::<Option<String>, _>(idx) {
        Ok(Some(s)) => serde_json::from_str(&s)
            .map(Some)
            .map_err(|e| sqlx::Error::Decode(Box::new(e))),
        Ok(None) => Ok(None),
        Err(e) => Err(e),
    }
}

/// Decode a byte-blob column.
pub fn bytes(row: &AnyRow, col: &str) -> Result<Vec<u8>, sqlx::Error> {
    row.try_get(col)
}

/// Ordinal version of [`bytes`].
pub fn bytes_idx(row: &AnyRow, idx: usize) -> Result<Vec<u8>, sqlx::Error> {
    row.try_get(idx)
}

/// Decode an optional byte-blob column.
pub fn bytes_opt(row: &AnyRow, col: &str) -> Result<Option<Vec<u8>>, sqlx::Error> {
    row.try_get(col)
}

/// Ordinal version of [`bytes_opt`].
pub fn bytes_opt_idx(row: &AnyRow, idx: usize) -> Result<Option<Vec<u8>>, sqlx::Error> {
    row.try_get(idx)
}

/// Decode a column whose type the `Any` driver already understands.
pub fn direct<T>(row: &AnyRow, col: &str) -> Result<T, sqlx::Error>
where
    T: for<'r> sqlx::Decode<'r, sqlx::Any> + sqlx::Type<sqlx::Any>,
{
    row.try_get(col)
}

/// Ordinal version of [`direct`].
pub fn direct_idx<T>(row: &AnyRow, idx: usize) -> Result<T, sqlx::Error>
where
    T: for<'r> sqlx::Decode<'r, sqlx::Any> + sqlx::Type<sqlx::Any>,
{
    row.try_get(idx)
}

/// Decode an optional column whose type the `Any` driver already understands.
pub fn direct_opt<T>(row: &AnyRow, col: &str) -> Result<Option<T>, sqlx::Error>
where
    T: for<'r> sqlx::Decode<'r, sqlx::Any> + sqlx::Type<sqlx::Any>,
{
    row.try_get(col)
}

/// Ordinal version of [`direct_opt`].
pub fn direct_opt_idx<T>(row: &AnyRow, idx: usize) -> Result<Option<T>, sqlx::Error>
where
    T: for<'r> sqlx::Decode<'r, sqlx::Any> + sqlx::Type<sqlx::Any>,
{
    row.try_get(idx)
}

/// Decode a boolean column.
///
/// SQLite stores booleans as `INTEGER` (0/1), while Postgres has a native
/// `BOOL` type, so this helper tries the integer path first and falls back to
/// a native boolean decode.
pub fn boolean(row: &AnyRow, col: &str) -> Result<bool, sqlx::Error> {
    if let Ok(i) = row.try_get::<i64, _>(col) {
        return Ok(i != 0);
    }
    row.try_get::<bool, _>(col)
}

/// Ordinal version of [`boolean`].
pub fn boolean_idx(row: &AnyRow, idx: usize) -> Result<bool, sqlx::Error> {
    if let Ok(i) = row.try_get::<i64, _>(idx) {
        return Ok(i != 0);
    }
    row.try_get::<bool, _>(idx)
}

/// Decode an optional boolean column.
pub fn boolean_opt(row: &AnyRow, col: &str) -> Result<Option<bool>, sqlx::Error> {
    match row.try_get::<Option<i64>, _>(col) {
        Ok(Some(i)) => Ok(Some(i != 0)),
        Ok(None) => Ok(None),
        Err(_) => row.try_get::<Option<bool>, _>(col),
    }
}

/// Ordinal version of [`boolean_opt`].
pub fn boolean_opt_idx(row: &AnyRow, idx: usize) -> Result<Option<bool>, sqlx::Error> {
    match row.try_get::<Option<i64>, _>(idx) {
        Ok(Some(i)) => Ok(Some(i != 0)),
        Ok(None) => Ok(None),
        Err(_) => row.try_get::<Option<bool>, _>(idx),
    }
}
