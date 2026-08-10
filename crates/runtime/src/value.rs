//! Owned values bound to SQL queries.

use std::sync::Arc;

use crate::types::chrono::{DateTime, NaiveDate, NaiveTime, Utc};
use crate::types::{Decimal, Uuid};

/// A value that can be encoded into a bound SQL parameter.
///
/// This is intentionally a small, owned enum: filters are frequently built in
/// one scope and executed in another, so borrowed values would poison every
/// signature in the API.
pub trait Encodable: Send + Sync + 'static {
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
