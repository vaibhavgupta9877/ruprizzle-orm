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
            Value::Array(_) => {
                return Err(rusqlite::Error::InvalidParameterName(
                    "array bind values are not supported yet".into(),
                ));
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

// P1-4: encode `&'q Value` by reference so `Str`/`Bytes` never re-allocate.
// Binding `&Value` instead of `Value` lets sqlx see the `'q` lifetime it needs
// to borrow `Arc<str>` / `Arc<[u8]>` data directly.

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
            Value::Array(_) => Err("array bind values are not supported yet".into()),
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
            Value::Array(_) => Err("array bind values are not supported yet".into()),
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
            Value::Array(_) => return None,
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
            Value::Array(_) => Err("array bind values are not supported yet".into()),
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
            Value::Array(_) => return None,
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
            Value::Array(_) => Err("array bind values are not supported yet".into()),
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
            Value::Array(_) => return None,
        })
    }
}
