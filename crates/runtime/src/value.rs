//! Owned values bound to SQL queries.

use std::sync::Arc;

use crate::types::chrono::{DateTime, NaiveDate, NaiveTime, Utc};
use crate::types::{Decimal, Uuid};

/// A value that can be encoded into a bound SQL parameter.
///
/// `to_value` returns an owned `Value`, so borrowed implementors can be used
/// at a call site without leaking lifetimes into the query API.
pub trait Encodable: Send + Sync {
    /// Encode `self` into a runtime value.
    fn to_value(&self) -> Value;
}

/// Marker trait for types that support ordered comparisons in SQL.
///
/// `String` is deliberately not `Ordered`; use the string-specific methods on
/// `Column<M, String>` instead.
pub trait Ordered: Ord + Encodable {}

/// An owned runtime value bound to a SQL query.
#[derive(Debug, Clone, PartialEq)]
#[allow(missing_docs)]
pub enum Value {
    /// SQL `NULL`.
    Null,
    Bool(bool),
    I32(i32),
    I64(i64),
    F64(f64),
    Decimal(Decimal),
    Str(Arc<str>),
    Uuid(Uuid),
    DateTime(DateTime<Utc>),
    Date(NaiveDate),
    Time(NaiveTime),
    Json(serde_json::Value),
    Bytes(Arc<[u8]>),
    Array(Vec<Value>),
}

impl Value {
    /// Returns `true` for `Value::Null`.
    #[must_use]
    pub const fn is_null(&self) -> bool {
        matches!(self, Value::Null)
    }

    /// Returns a JSON representation of the value for SQLite / MySQL JSON fallbacks.
    ///
    /// Errors on nested arrays or byte arrays, which cannot be represented
    /// unambiguously in a JSON text column.
    pub(crate) fn as_json(
        &self,
    ) -> Result<serde_json::Value, Box<dyn std::error::Error + Send + Sync>> {
        Ok(match self {
            Value::Null => serde_json::Value::Null,
            Value::Bool(b) => serde_json::Value::Bool(*b),
            Value::I32(i) => serde_json::to_value(*i)?,
            Value::I64(i) => serde_json::to_value(*i)?,
            Value::F64(f) => serde_json::to_value(*f)?,
            Value::Decimal(d) => serde_json::to_value(*d)?,
            Value::Str(s) => serde_json::Value::String(s.to_string()),
            Value::Uuid(u) => serde_json::Value::String(u.to_string()),
            Value::DateTime(dt) => serde_json::Value::String(dt.to_rfc3339()),
            Value::Date(d) => serde_json::Value::String(d.to_string()),
            Value::Time(t) => serde_json::Value::String(t.to_string()),
            Value::Json(v) => v.clone(),
            Value::Bytes(_) => return Err("byte arrays cannot be stored as JSON arrays".into()),
            Value::Array(values) => Value::array_to_json(values)?,
        })
    }

    fn array_to_json(
        values: &[Value],
    ) -> Result<serde_json::Value, Box<dyn std::error::Error + Send + Sync>> {
        if values.iter().any(|v| matches!(v, Value::Array(_))) {
            return Err("nested arrays are not supported".into());
        }
        Ok(serde_json::Value::Array(
            values
                .iter()
                .map(Value::as_json)
                .collect::<Result<Vec<_>, _>>()?,
        ))
    }
}

// Helpers for homogeneous Postgres arrays in `sqlx::Postgres` encoding.
macro_rules! pg_array_match {
    ($first:expr, $variant:pat, $ty:ty) => {
        match $first {
            Some($variant) => Some(<Vec<Option<$ty>> as sqlx::Type<sqlx::Postgres>>::type_info()),
            _ => None,
        }
    };
}

macro_rules! pg_array_encode {
    ($values:expr, $buf:expr, $check:pat, $variant:pat, $ty:ty, $convert:expr) => {
        if $values
            .iter()
            .find(|v| !v.is_null())
            .map_or(false, |v| matches!(v, $check))
        {
            let vec: Vec<Option<$ty>> = $values
                .iter()
                .map(|val| match val {
                    Value::Null => Ok(None),
                    $variant => Ok(Some($convert)),
                    other => Err(format!(
                        "array contains mixed or unsupported element: expected {}, found {:?}",
                        std::any::type_name::<$ty>(),
                        other
                    )
                    .into()),
                })
                .collect::<Result<Vec<Option<$ty>>, Box<dyn std::error::Error + Send + Sync>>>()?;
            return <Vec<Option<$ty>> as sqlx::Encode<'q, sqlx::Postgres>>::encode_by_ref(
                &vec, $buf,
            );
        }
    };
}

impl Encodable for bool {
    fn to_value(&self) -> Value {
        Value::Bool(*self)
    }
}
impl Ordered for bool {}

impl Encodable for i32 {
    fn to_value(&self) -> Value {
        Value::I32(*self)
    }
}
impl Ordered for i32 {}

impl Encodable for i64 {
    fn to_value(&self) -> Value {
        Value::I64(*self)
    }
}
impl Ordered for i64 {}

impl Encodable for f64 {
    fn to_value(&self) -> Value {
        Value::F64(*self)
    }
}

impl Encodable for Decimal {
    fn to_value(&self) -> Value {
        Value::Decimal(*self)
    }
}
impl Ordered for Decimal {}

impl Encodable for String {
    fn to_value(&self) -> Value {
        Value::Str(Arc::from(self.as_str()))
    }
}

impl Encodable for str {
    fn to_value(&self) -> Value {
        Value::Str(Arc::from(self))
    }
}

impl<T: Encodable + ?Sized> Encodable for &T {
    fn to_value(&self) -> Value {
        T::to_value(&**self)
    }
}

impl Encodable for Value {
    fn to_value(&self) -> Value {
        self.clone()
    }
}

#[cfg(feature = "sqlite-rusqlite")]
impl rusqlite::ToSql for Value {
    fn to_sql(&self) -> Result<rusqlite::types::ToSqlOutput<'_>, rusqlite::Error> {
        use rusqlite::types::{ToSqlOutput, ValueRef};
        Ok(match self {
            Value::Null => ToSqlOutput::Borrowed(ValueRef::Null),
            Value::Bool(b) => ToSqlOutput::Borrowed(ValueRef::Integer(i64::from(*b))),
            Value::I32(i) => ToSqlOutput::Borrowed(ValueRef::Integer(i64::from(*i))),
            Value::I64(i) => ToSqlOutput::Borrowed(ValueRef::Integer(*i)),
            Value::F64(f) => ToSqlOutput::Borrowed(ValueRef::Real(*f)),
            Value::Decimal(d) => ToSqlOutput::Owned(rusqlite::types::Value::Text(d.to_string())),
            Value::Str(s) => ToSqlOutput::Borrowed(ValueRef::Text(s.as_bytes())),
            Value::Uuid(u) => ToSqlOutput::Owned(rusqlite::types::Value::Text(u.to_string())),
            Value::DateTime(dt) => {
                ToSqlOutput::Owned(rusqlite::types::Value::Text(dt.to_rfc3339()))
            }
            Value::Date(d) => ToSqlOutput::Owned(rusqlite::types::Value::Text(d.to_string())),
            Value::Time(t) => ToSqlOutput::Owned(rusqlite::types::Value::Text(t.to_string())),
            Value::Json(v) => ToSqlOutput::Owned(rusqlite::types::Value::Text(v.to_string())),
            Value::Bytes(b) => ToSqlOutput::Borrowed(ValueRef::Blob(b.as_ref())),
            Value::Array(values) => {
                let json = Value::array_to_json(values)
                    .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
                ToSqlOutput::Owned(rusqlite::types::Value::Text(json.to_string()))
            }
        })
    }
}

impl Encodable for Uuid {
    fn to_value(&self) -> Value {
        Value::Uuid(*self)
    }
}
impl Ordered for Uuid {}

impl Encodable for DateTime<Utc> {
    fn to_value(&self) -> Value {
        Value::DateTime(*self)
    }
}
impl Ordered for DateTime<Utc> {}

impl Encodable for NaiveDate {
    fn to_value(&self) -> Value {
        Value::Date(*self)
    }
}
impl Ordered for NaiveDate {}

impl Encodable for NaiveTime {
    fn to_value(&self) -> Value {
        Value::Time(*self)
    }
}
impl Ordered for NaiveTime {}

impl Encodable for serde_json::Value {
    fn to_value(&self) -> Value {
        Value::Json(self.clone())
    }
}

impl Encodable for Vec<u8> {
    fn to_value(&self) -> Value {
        Value::Bytes(Arc::from(self.as_slice()))
    }
}

impl<T: Encodable> Encodable for Vec<T> {
    fn to_value(&self) -> Value {
        Value::Array(self.iter().map(Encodable::to_value).collect())
    }
}

impl<T: Encodable> Encodable for Option<T> {
    fn to_value(&self) -> Value {
        match self {
            Some(v) => v.to_value(),
            None => Value::Null,
        }
    }
}

impl<T: Ordered> Ordered for Option<T> {}

impl<T: Encodable> Encodable for Arc<T> {
    fn to_value(&self) -> Value {
        (**self).to_value()
    }
}

impl sqlx::Type<sqlx::Any> for Value {
    fn type_info() -> <sqlx::Any as sqlx::Database>::TypeInfo {
        // Concrete type is reported through `Encode::produces`; this is a
        // harmless placeholder for `Value::Null`.
        <String as sqlx::Type<sqlx::Any>>::type_info()
    }
}

impl sqlx::Type<sqlx::Postgres> for Value {
    fn type_info() -> <sqlx::Postgres as sqlx::Database>::TypeInfo {
        // Concrete type is reported through `Encode::produces`; this is a
        // harmless placeholder for `Value::Null`.
        <String as sqlx::Type<sqlx::Postgres>>::type_info()
    }
}

impl sqlx::Type<sqlx::Sqlite> for Value {
    fn type_info() -> <sqlx::Sqlite as sqlx::Database>::TypeInfo {
        // Concrete type is reported through `Encode::produces`; this is a
        // harmless placeholder for `Value::Null`.
        <String as sqlx::Type<sqlx::Sqlite>>::type_info()
    }
}

impl sqlx::Type<sqlx::MySql> for Value {
    fn type_info() -> <sqlx::MySql as sqlx::Database>::TypeInfo {
        // Concrete type is reported through `Encode::produces`; this is a
        // harmless placeholder for `Value::Null`.
        <String as sqlx::Type<sqlx::MySql>>::type_info()
    }
}

// P1-4: encode `&'q Value` by reference so `Str`/`Bytes` data is never
// re-allocated. The lifetime `'q` is the borrow of the value inside the sqlx
// query, which lets `sqlx::query` use its `.fetch()` cursor for unbuffered
// streams.

impl<'q> sqlx::Encode<'q, sqlx::Any> for &'q Value {
    fn encode_by_ref(
        &self,
        buf: &mut <sqlx::Any as sqlx::Database>::ArgumentBuffer<'q>,
    ) -> Result<sqlx::encode::IsNull, Box<dyn std::error::Error + Send + Sync + 'static>> {
        match *self {
            // The `Any` driver drops `IsNull::Yes` for non-`Option` Rust types,
            // which silently shifts every subsequent placeholder. Encoding `None`
            // as an `Option<String>` keeps the parameter in place and lets the
            // database cast the untyped NULL to the target column.
            Value::Null => {
                let n: Option<String> = None;
                sqlx::Encode::<sqlx::Any>::encode_by_ref(&n, buf)
            }
            Value::Bool(b) => sqlx::Encode::<sqlx::Any>::encode_by_ref(&b, buf),
            Value::I32(i) => sqlx::Encode::<sqlx::Any>::encode_by_ref(&i, buf),
            Value::I64(i) => sqlx::Encode::<sqlx::Any>::encode_by_ref(&i, buf),
            Value::F64(f) => sqlx::Encode::<sqlx::Any>::encode_by_ref(&f, buf),
            // The `Any` driver does not have `Encode<Any>` for chrono/uuid/decimal/json,
            // so we serialize these to a type the `Any` driver understands and let the
            // database cast from text/bytes.
            Value::Decimal(d) => {
                let s = d.to_string();
                sqlx::Encode::<sqlx::Any>::encode_by_ref(&s, buf)
            }
            Value::Str(s) => sqlx::Encode::<sqlx::Any>::encode(s.as_ref(), buf),
            Value::Uuid(u) => {
                let s = u.to_string();
                sqlx::Encode::<sqlx::Any>::encode_by_ref(&s, buf)
            }
            Value::DateTime(dt) => {
                let s = dt.to_rfc3339();
                sqlx::Encode::<sqlx::Any>::encode_by_ref(&s, buf)
            }
            Value::Date(d) => {
                let s = d.to_string();
                sqlx::Encode::<sqlx::Any>::encode_by_ref(&s, buf)
            }
            Value::Time(t) => {
                let s = t.to_string();
                sqlx::Encode::<sqlx::Any>::encode_by_ref(&s, buf)
            }
            Value::Json(v) => {
                let s = v.to_string();
                sqlx::Encode::<sqlx::Any>::encode_by_ref(&s, buf)
            }
            Value::Bytes(b) => sqlx::Encode::<sqlx::Any>::encode(b.as_ref(), buf),
            Value::Array(_) => Err(
                "arrays cannot be bound through the generic sqlx::Any driver; \
                 use the native postgres:// or sqlite:// driver or the \
                 postgres-tokio-postgres / sqlite-rusqlite features"
                    .into(),
            ),
        }
    }

    fn produces(&self) -> Option<<sqlx::Any as sqlx::Database>::TypeInfo> {
        Some(match *self {
            Value::Null => <String as sqlx::Type<sqlx::Any>>::type_info(),
            Value::Bool(_) => <bool as sqlx::Type<sqlx::Any>>::type_info(),
            Value::I32(_) => <i32 as sqlx::Type<sqlx::Any>>::type_info(),
            Value::I64(_) => <i64 as sqlx::Type<sqlx::Any>>::type_info(),
            Value::F64(_) => <f64 as sqlx::Type<sqlx::Any>>::type_info(),
            // These are bound as text/bytes; the database casts as needed.
            Value::Decimal(_)
            | Value::Str(_)
            | Value::Uuid(_)
            | Value::DateTime(_)
            | Value::Date(_)
            | Value::Time(_)
            | Value::Json(_) => <String as sqlx::Type<sqlx::Any>>::type_info(),
            Value::Bytes(_) => <Vec<u8> as sqlx::Type<sqlx::Any>>::type_info(),
            Value::Array(_) => return None,
        })
    }
}

impl<'q> sqlx::Encode<'q, sqlx::Postgres> for &'q Value {
    fn encode_by_ref(
        &self,
        buf: &mut <sqlx::Postgres as sqlx::Database>::ArgumentBuffer<'q>,
    ) -> Result<sqlx::encode::IsNull, Box<dyn std::error::Error + Send + Sync + 'static>> {
        match *self {
            Value::Null => {
                let n: Option<String> = None;
                sqlx::Encode::<sqlx::Postgres>::encode_by_ref(&n, buf)
            }
            Value::Bool(b) => sqlx::Encode::<sqlx::Postgres>::encode_by_ref(&b, buf),
            Value::I32(i) => sqlx::Encode::<sqlx::Postgres>::encode_by_ref(&i, buf),
            Value::I64(i) => sqlx::Encode::<sqlx::Postgres>::encode_by_ref(&i, buf),
            Value::F64(f) => sqlx::Encode::<sqlx::Postgres>::encode_by_ref(&f, buf),
            Value::Decimal(d) => sqlx::Encode::<sqlx::Postgres>::encode_by_ref(d, buf),
            Value::Str(s) => sqlx::Encode::<sqlx::Postgres>::encode(s.as_ref(), buf),
            Value::Uuid(u) => sqlx::Encode::<sqlx::Postgres>::encode_by_ref(u, buf),
            Value::DateTime(dt) => sqlx::Encode::<sqlx::Postgres>::encode_by_ref(dt, buf),
            Value::Date(d) => sqlx::Encode::<sqlx::Postgres>::encode_by_ref(d, buf),
            Value::Time(t) => sqlx::Encode::<sqlx::Postgres>::encode_by_ref(t, buf),
            Value::Json(v) => sqlx::Encode::<sqlx::Postgres>::encode_by_ref(v, buf),
            Value::Bytes(b) => sqlx::Encode::<sqlx::Postgres>::encode(b.as_ref(), buf),
            Value::Array(values) => {
                if values.iter().any(|v| matches!(v, Value::Array(_))) {
                    return Err("nested arrays are not supported".into());
                }
                pg_array_encode!(values, buf, Value::Bool(_), Value::Bool(b), bool, *b);
                pg_array_encode!(values, buf, Value::I32(_), Value::I32(i), i32, *i);
                pg_array_encode!(values, buf, Value::I64(_), Value::I64(i), i64, *i);
                pg_array_encode!(values, buf, Value::F64(_), Value::F64(f), f64, *f);
                pg_array_encode!(
                    values,
                    buf,
                    Value::Decimal(_),
                    Value::Decimal(d),
                    Decimal,
                    *d
                );
                pg_array_encode!(
                    values,
                    buf,
                    Value::Str(_),
                    Value::Str(s),
                    &'q str,
                    s.as_ref()
                );
                pg_array_encode!(values, buf, Value::Uuid(_), Value::Uuid(u), Uuid, *u);
                pg_array_encode!(
                    values,
                    buf,
                    Value::DateTime(_),
                    Value::DateTime(dt),
                    DateTime<Utc>,
                    *dt
                );
                pg_array_encode!(values, buf, Value::Date(_), Value::Date(d), NaiveDate, *d);
                pg_array_encode!(values, buf, Value::Time(_), Value::Time(t), NaiveTime, *t);
                pg_array_encode!(
                    values,
                    buf,
                    Value::Json(_),
                    Value::Json(v),
                    serde_json::Value,
                    v.clone()
                );
                pg_array_encode!(
                    values,
                    buf,
                    Value::Bytes(_),
                    Value::Bytes(b),
                    &'q [u8],
                    b.as_ref()
                );
                // Empty or all-NULL: bind as an empty text array. This is a safe default
                // because untyped placeholders are usually cast by the query.
                let empty: Vec<Option<String>> = Vec::new();
                <Vec<Option<String>> as sqlx::Encode<'q, sqlx::Postgres>>::encode_by_ref(
                    &empty, buf,
                )
            }
        }
    }

    fn produces(&self) -> Option<<sqlx::Postgres as sqlx::Database>::TypeInfo> {
        Some(match *self {
            Value::Null => <String as sqlx::Type<sqlx::Postgres>>::type_info(),
            Value::Bool(_) => <bool as sqlx::Type<sqlx::Postgres>>::type_info(),
            Value::I32(_) => <i32 as sqlx::Type<sqlx::Postgres>>::type_info(),
            Value::I64(_) => <i64 as sqlx::Type<sqlx::Postgres>>::type_info(),
            Value::F64(_) => <f64 as sqlx::Type<sqlx::Postgres>>::type_info(),
            Value::Str(_) => <String as sqlx::Type<sqlx::Postgres>>::type_info(),
            Value::Decimal(_) => <Decimal as sqlx::Type<sqlx::Postgres>>::type_info(),
            Value::Uuid(_) => <Uuid as sqlx::Type<sqlx::Postgres>>::type_info(),
            Value::DateTime(_) => <DateTime<Utc> as sqlx::Type<sqlx::Postgres>>::type_info(),
            Value::Date(_) => <NaiveDate as sqlx::Type<sqlx::Postgres>>::type_info(),
            Value::Time(_) => <NaiveTime as sqlx::Type<sqlx::Postgres>>::type_info(),
            Value::Json(_) => {
                <sqlx::types::Json<serde_json::Value> as sqlx::Type<sqlx::Postgres>>::type_info()
            }
            Value::Bytes(_) => <Vec<u8> as sqlx::Type<sqlx::Postgres>>::type_info(),
            Value::Array(values) => {
                let first = values.iter().find(|v| !v.is_null());
                if let Some(t) = pg_array_match!(first, Value::Bool(_), bool) {
                    return Some(t);
                }
                if let Some(t) = pg_array_match!(first, Value::I32(_), i32) {
                    return Some(t);
                }
                if let Some(t) = pg_array_match!(first, Value::I64(_), i64) {
                    return Some(t);
                }
                if let Some(t) = pg_array_match!(first, Value::F64(_), f64) {
                    return Some(t);
                }
                if let Some(t) = pg_array_match!(first, Value::Decimal(_), Decimal) {
                    return Some(t);
                }
                if let Some(t) = pg_array_match!(first, Value::Str(_), &'q str) {
                    return Some(t);
                }
                if let Some(t) = pg_array_match!(first, Value::Uuid(_), Uuid) {
                    return Some(t);
                }
                if let Some(t) = pg_array_match!(first, Value::DateTime(_), DateTime<Utc>) {
                    return Some(t);
                }
                if let Some(t) = pg_array_match!(first, Value::Date(_), NaiveDate) {
                    return Some(t);
                }
                if let Some(t) = pg_array_match!(first, Value::Time(_), NaiveTime) {
                    return Some(t);
                }
                if let Some(t) = pg_array_match!(first, Value::Json(_), serde_json::Value) {
                    return Some(t);
                }
                if let Some(t) = pg_array_match!(first, Value::Bytes(_), &'q [u8]) {
                    return Some(t);
                }
                // Empty or all-NULL, or a nested array that will be caught at encode time:
                // default to text[] to give a sensible, well-formed Postgres type.
                <Vec<Option<String>> as sqlx::Type<sqlx::Postgres>>::type_info()
            }
        })
    }
}

impl<'q> sqlx::Encode<'q, sqlx::MySql> for &'q Value {
    fn encode_by_ref(
        &self,
        buf: &mut <sqlx::MySql as sqlx::Database>::ArgumentBuffer<'q>,
    ) -> Result<sqlx::encode::IsNull, Box<dyn std::error::Error + Send + Sync + 'static>> {
        match *self {
            Value::Null => {
                let n: Option<String> = None;
                sqlx::Encode::<sqlx::MySql>::encode_by_ref(&n, buf)
            }
            Value::Bool(b) => sqlx::Encode::<sqlx::MySql>::encode_by_ref(&b, buf),
            Value::I32(i) => sqlx::Encode::<sqlx::MySql>::encode_by_ref(&i, buf),
            Value::I64(i) => sqlx::Encode::<sqlx::MySql>::encode_by_ref(&i, buf),
            Value::F64(f) => sqlx::Encode::<sqlx::MySql>::encode_by_ref(&f, buf),
            Value::Decimal(d) => {
                let s = d.to_string();
                sqlx::Encode::<sqlx::MySql>::encode_by_ref(&s, buf)
            }
            Value::Str(s) => sqlx::Encode::<sqlx::MySql>::encode(s.as_ref(), buf),
            Value::Uuid(u) => {
                let s = u.to_string();
                sqlx::Encode::<sqlx::MySql>::encode_by_ref(&s, buf)
            }
            Value::DateTime(dt) => {
                let s = dt.to_rfc3339();
                sqlx::Encode::<sqlx::MySql>::encode_by_ref(&s, buf)
            }
            Value::Date(d) => {
                let s = d.to_string();
                sqlx::Encode::<sqlx::MySql>::encode_by_ref(&s, buf)
            }
            Value::Time(t) => {
                let s = t.to_string();
                sqlx::Encode::<sqlx::MySql>::encode_by_ref(&s, buf)
            }
            Value::Json(v) => {
                let s = v.to_string();
                sqlx::Encode::<sqlx::MySql>::encode_by_ref(&s, buf)
            }
            Value::Bytes(b) => sqlx::Encode::<sqlx::MySql>::encode(b.as_ref(), buf),
            Value::Array(values) => {
                let json = Value::array_to_json(values)?;
                let s = json.to_string();
                sqlx::Encode::<sqlx::MySql>::encode_by_ref(&s, buf)
            }
        }
    }

    fn produces(&self) -> Option<<sqlx::MySql as sqlx::Database>::TypeInfo> {
        Some(match *self {
            Value::Null
            | Value::Decimal(_)
            | Value::Str(_)
            | Value::Uuid(_)
            | Value::DateTime(_)
            | Value::Date(_)
            | Value::Time(_)
            | Value::Json(_) => <String as sqlx::Type<sqlx::MySql>>::type_info(),
            Value::Bool(_) => <bool as sqlx::Type<sqlx::MySql>>::type_info(),
            Value::I32(_) => <i32 as sqlx::Type<sqlx::MySql>>::type_info(),
            Value::I64(_) => <i64 as sqlx::Type<sqlx::MySql>>::type_info(),
            Value::F64(_) => <f64 as sqlx::Type<sqlx::MySql>>::type_info(),
            Value::Bytes(_) => <Vec<u8> as sqlx::Type<sqlx::MySql>>::type_info(),
            Value::Array(_) => <String as sqlx::Type<sqlx::MySql>>::type_info(),
        })
    }
}

impl<'q> sqlx::Encode<'q, sqlx::Sqlite> for &'q Value {
    fn encode_by_ref(
        &self,
        buf: &mut <sqlx::Sqlite as sqlx::Database>::ArgumentBuffer<'q>,
    ) -> Result<sqlx::encode::IsNull, Box<dyn std::error::Error + Send + Sync + 'static>> {
        match *self {
            Value::Null => {
                let n: Option<String> = None;
                sqlx::Encode::<sqlx::Sqlite>::encode_by_ref(&n, buf)
            }
            Value::Bool(b) => sqlx::Encode::<sqlx::Sqlite>::encode_by_ref(&b, buf),
            Value::I32(i) => sqlx::Encode::<sqlx::Sqlite>::encode_by_ref(&i, buf),
            Value::I64(i) => sqlx::Encode::<sqlx::Sqlite>::encode_by_ref(&i, buf),
            Value::F64(f) => sqlx::Encode::<sqlx::Sqlite>::encode_by_ref(&f, buf),
            Value::Decimal(d) => {
                let s = d.to_string();
                sqlx::Encode::<sqlx::Sqlite>::encode_by_ref(&s, buf)
            }
            Value::Str(s) => sqlx::Encode::<sqlx::Sqlite>::encode(s.as_ref(), buf),
            Value::Uuid(u) => {
                let s = u.to_string();
                sqlx::Encode::<sqlx::Sqlite>::encode_by_ref(&s, buf)
            }
            Value::DateTime(dt) => {
                let s = dt.to_rfc3339();
                sqlx::Encode::<sqlx::Sqlite>::encode_by_ref(&s, buf)
            }
            Value::Date(d) => {
                let s = d.to_string();
                sqlx::Encode::<sqlx::Sqlite>::encode_by_ref(&s, buf)
            }
            Value::Time(t) => {
                let s = t.to_string();
                sqlx::Encode::<sqlx::Sqlite>::encode_by_ref(&s, buf)
            }
            Value::Json(v) => {
                let s = v.to_string();
                sqlx::Encode::<sqlx::Sqlite>::encode_by_ref(&s, buf)
            }
            Value::Bytes(b) => sqlx::Encode::<sqlx::Sqlite>::encode(b.as_ref(), buf),
            Value::Array(values) => {
                let json = Value::array_to_json(values)?;
                let s = json.to_string();
                sqlx::Encode::<sqlx::Sqlite>::encode_by_ref(&s, buf)
            }
        }
    }

    fn produces(&self) -> Option<<sqlx::Sqlite as sqlx::Database>::TypeInfo> {
        Some(match *self {
            Value::Null
            | Value::Decimal(_)
            | Value::Str(_)
            | Value::Uuid(_)
            | Value::DateTime(_)
            | Value::Date(_)
            | Value::Time(_)
            | Value::Json(_) => <String as sqlx::Type<sqlx::Sqlite>>::type_info(),
            Value::Bool(_) => <bool as sqlx::Type<sqlx::Sqlite>>::type_info(),
            Value::I32(_) => <i32 as sqlx::Type<sqlx::Sqlite>>::type_info(),
            Value::I64(_) => <i64 as sqlx::Type<sqlx::Sqlite>>::type_info(),
            Value::F64(_) => <f64 as sqlx::Type<sqlx::Sqlite>>::type_info(),
            Value::Bytes(_) => <Vec<u8> as sqlx::Type<sqlx::Sqlite>>::type_info(),
            Value::Array(_) => <String as sqlx::Type<sqlx::Sqlite>>::type_info(),
        })
    }
}
