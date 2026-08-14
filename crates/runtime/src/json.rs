//! JSON path and typed JSON column tokens.

use std::marker::PhantomData;

use crate::filter::{CmpOp, Filter, FilterNode, JsonFilterOp};
use crate::order::OrderBy;
use crate::value::{Encodable, Ordered, Value};

/// A single step in a JSON path.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JsonPathSegment {
    /// Object key.
    Key(&'static str),
    /// Array index.
    Index(usize),
}

/// A sequence of JSON path segments.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct JsonPath(pub Vec<JsonPathSegment>);

impl From<&'static str> for JsonPath {
    /// Creates a one-segment key path.
    fn from(key: &'static str) -> Self {
        Self(vec![JsonPathSegment::Key(key)])
    }
}

impl JsonPath {
    /// Creates a path from an iterator of segments.
    #[must_use]
    pub fn new(segments: impl IntoIterator<Item = JsonPathSegment>) -> Self {
        Self(segments.into_iter().collect())
    }
}

/// A typed token for a JSON extraction expression.
#[derive(Clone, PartialEq, Debug)]
pub struct JsonColumn<M, T> {
    /// The SQL table name.
    pub table: &'static str,
    /// The SQL column name.
    pub column: &'static str,
    /// The JSON path inside the column.
    pub path: JsonPath,
    /// `true` for text extraction (`->>`/`#>>`), `false` for JSON (`->`/`#>`).
    pub text: bool,
    pub(crate) _marker: PhantomData<fn() -> (M, T)>,
}

impl<M, T: Encodable> JsonColumn<M, T> {
    /// `json_expr = value`.
    pub fn eq<V: Into<T>>(self, value: V) -> Filter<M> {
        let value = value.into().to_value();
        Filter::new(FilterNode::Json {
            table: self.table,
            column: self.column,
            path: self.path,
            text: self.text,
            op: JsonFilterOp::Cmp(CmpOp::Eq),
            value,
        })
    }

    /// `json_expr <> value`.
    pub fn ne<V: Into<T>>(self, value: V) -> Filter<M> {
        let value = value.into().to_value();
        Filter::new(FilterNode::Json {
            table: self.table,
            column: self.column,
            path: self.path,
            text: self.text,
            op: JsonFilterOp::Cmp(CmpOp::Ne),
            value,
        })
    }
}

impl<M, T: Ordered> JsonColumn<M, T> {
    /// `json_expr > value`.
    pub fn gt<V: Into<T>>(self, value: V) -> Filter<M> {
        let value = value.into().to_value();
        Filter::new(FilterNode::Json {
            table: self.table,
            column: self.column,
            path: self.path,
            text: self.text,
            op: JsonFilterOp::Cmp(CmpOp::Gt),
            value,
        })
    }

    /// `json_expr >= value`.
    pub fn gte<V: Into<T>>(self, value: V) -> Filter<M> {
        let value = value.into().to_value();
        Filter::new(FilterNode::Json {
            table: self.table,
            column: self.column,
            path: self.path,
            text: self.text,
            op: JsonFilterOp::Cmp(CmpOp::Gte),
            value,
        })
    }

    /// `json_expr < value`.
    pub fn lt<V: Into<T>>(self, value: V) -> Filter<M> {
        let value = value.into().to_value();
        Filter::new(FilterNode::Json {
            table: self.table,
            column: self.column,
            path: self.path,
            text: self.text,
            op: JsonFilterOp::Cmp(CmpOp::Lt),
            value,
        })
    }

    /// `json_expr <= value`.
    pub fn lte<V: Into<T>>(self, value: V) -> Filter<M> {
        let value = value.into().to_value();
        Filter::new(FilterNode::Json {
            table: self.table,
            column: self.column,
            path: self.path,
            text: self.text,
            op: JsonFilterOp::Cmp(CmpOp::Lte),
            value,
        })
    }
}

impl<M> JsonColumn<M, String> {
    /// `json_expr LIKE '%pattern%'`.
    pub fn like(self, pattern: &str) -> Filter<M> {
        Filter::new(FilterNode::Json {
            table: self.table,
            column: self.column,
            path: self.path,
            text: self.text,
            op: JsonFilterOp::Cmp(CmpOp::Like),
            value: Value::Str(format!("%{pattern}%").into()),
        })
    }

    /// Case-insensitive pattern match (`ILIKE` on Postgres, `LIKE` on SQLite).
    pub fn ilike(self, pattern: &str) -> Filter<M> {
        Filter::new(FilterNode::Json {
            table: self.table,
            column: self.column,
            path: self.path,
            text: self.text,
            op: JsonFilterOp::Cmp(CmpOp::Ilike),
            value: Value::Str(format!("%{pattern}%").into()),
        })
    }
}

impl<M> JsonColumn<M, serde_json::Value> {
    /// Append an object-key segment and keep JSON extraction.
    pub fn get(mut self, key: &'static str) -> JsonColumn<M, serde_json::Value> {
        self.path.0.push(JsonPathSegment::Key(key));
        self
    }

    /// Append an object-key segment and switch to text extraction.
    pub fn get_text(mut self, key: &'static str) -> JsonColumn<M, String> {
        self.path.0.push(JsonPathSegment::Key(key));
        JsonColumn {
            table: self.table,
            column: self.column,
            path: self.path,
            text: true,
            _marker: PhantomData,
        }
    }

    /// Append an array-index segment and keep JSON extraction.
    pub fn at(mut self, index: usize) -> JsonColumn<M, serde_json::Value> {
        self.path.0.push(JsonPathSegment::Index(index));
        self
    }

    /// `json_expr > value`.
    pub fn gt<V: Into<serde_json::Value>>(self, value: V) -> Filter<M> {
        let value = value.into().to_value();
        Filter::new(FilterNode::Json {
            table: self.table,
            column: self.column,
            path: self.path,
            text: self.text,
            op: JsonFilterOp::Cmp(CmpOp::Gt),
            value,
        })
    }

    /// `json_expr >= value`.
    pub fn gte<V: Into<serde_json::Value>>(self, value: V) -> Filter<M> {
        let value = value.into().to_value();
        Filter::new(FilterNode::Json {
            table: self.table,
            column: self.column,
            path: self.path,
            text: self.text,
            op: JsonFilterOp::Cmp(CmpOp::Gte),
            value,
        })
    }

    /// `json_expr < value`.
    pub fn lt<V: Into<serde_json::Value>>(self, value: V) -> Filter<M> {
        let value = value.into().to_value();
        Filter::new(FilterNode::Json {
            table: self.table,
            column: self.column,
            path: self.path,
            text: self.text,
            op: JsonFilterOp::Cmp(CmpOp::Lt),
            value,
        })
    }

    /// `json_expr <= value`.
    pub fn lte<V: Into<serde_json::Value>>(self, value: V) -> Filter<M> {
        let value = value.into().to_value();
        Filter::new(FilterNode::Json {
            table: self.table,
            column: self.column,
            path: self.path,
            text: self.text,
            op: JsonFilterOp::Cmp(CmpOp::Lte),
            value,
        })
    }
}

impl<M, T> JsonColumn<M, T> {
    /// `ORDER BY json_expr ASC`.
    pub fn asc(&self) -> OrderBy<M> {
        OrderBy {
            table: self.table,
            column: self.column,
            desc: false,
            json_path: Some(self.path.clone()),
            text: self.text,
            _marker: PhantomData,
        }
    }

    /// `ORDER BY json_expr DESC`.
    pub fn desc(&self) -> OrderBy<M> {
        OrderBy {
            table: self.table,
            column: self.column,
            desc: true,
            json_path: Some(self.path.clone()),
            text: self.text,
            _marker: PhantomData,
        }
    }
}

/// A JSON set update expression.
#[derive(Debug, Clone, PartialEq)]
pub struct JsonSet {
    pub(crate) column: &'static str,
    pub(crate) path: JsonPath,
    pub(crate) value: Value,
}
