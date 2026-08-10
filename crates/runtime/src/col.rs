//! Typed column tokens.

use std::marker::PhantomData;

use crate::filter::{CmpOp, Filter, FilterNode};
use crate::order::OrderBy;
use crate::value::{Encodable, Ordered, Value};

/// A typed token for a physical column.
///
/// The `M` phantom type prevents mixing columns from different models into one
/// query. The `T` phantom type is the Rust type stored in the column, which is
/// used to type-check filter values at compile time.
#[derive(Debug, PartialEq, Eq)]
pub struct Column<M, T> {
    /// The SQL table name.
    pub table: &'static str,
    /// The SQL column name.
    pub column: &'static str,
    _marker: PhantomData<fn() -> (M, T)>,
}

impl<M, T> Copy for Column<M, T> {}
impl<M, T> Clone for Column<M, T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<M, T> Column<M, T> {
    /// Creates a new column token.
    #[must_use]
    pub const fn new(table: &'static str, column: &'static str) -> Self {
        Self {
            table,
            column,
            _marker: PhantomData,
        }
    }
}

impl<M, T: Encodable> Column<M, T> {
    /// `column = value`.
    pub fn eq<V: Into<T>>(self, value: V) -> Filter<M> {
        let value = value.into().to_value();
        Filter::new(FilterNode::Cmp {
            table: self.table,
            column: self.column,
            op: CmpOp::Eq,
            value,
        })
    }

    /// `column <> value`.
    pub fn ne<V: Into<T>>(self, value: V) -> Filter<M> {
        let value = value.into().to_value();
        Filter::new(FilterNode::Cmp {
            table: self.table,
            column: self.column,
            op: CmpOp::Ne,
            value,
        })
    }

    /// `column IN (...)`.
    pub fn in_<V: Into<T>>(self, values: Vec<V>) -> Filter<M> {
        Filter::new(FilterNode::In {
            table: self.table,
            column: self.column,
            values: values.into_iter().map(|v| v.into().to_value()).collect(),
            negated: false,
        })
    }

    /// `ASC`.
    pub fn asc(self) -> OrderBy<M> {
        OrderBy::new(self.table, self.column, false)
    }

    /// `DESC`.
    pub fn desc(self) -> OrderBy<M> {
        OrderBy::new(self.table, self.column, true)
    }
}

impl<M, T: Ordered> Column<M, T> {
    /// `column > value`.
    pub fn gt<V: Into<T>>(self, value: V) -> Filter<M> {
        let value = value.into().to_value();
        Filter::new(FilterNode::Cmp {
            table: self.table,
            column: self.column,
            op: CmpOp::Gt,
            value,
        })
    }

    /// `column >= value`.
    pub fn gte<V: Into<T>>(self, value: V) -> Filter<M> {
        let value = value.into().to_value();
        Filter::new(FilterNode::Cmp {
            table: self.table,
            column: self.column,
            op: CmpOp::Gte,
            value,
        })
    }

    /// `column < value`.
    pub fn lt<V: Into<T>>(self, value: V) -> Filter<M> {
        let value = value.into().to_value();
        Filter::new(FilterNode::Cmp {
            table: self.table,
            column: self.column,
            op: CmpOp::Lt,
            value,
        })
    }

    /// `column <= value`.
    pub fn lte<V: Into<T>>(self, value: V) -> Filter<M> {
        let value = value.into().to_value();
        Filter::new(FilterNode::Cmp {
            table: self.table,
            column: self.column,
            op: CmpOp::Lte,
            value,
        })
    }

    /// `column BETWEEN lo AND hi`.
    pub fn between<Lo: Into<T>, Hi: Into<T>>(self, lo: Lo, hi: Hi) -> Filter<M> {
        Filter::new(FilterNode::Between {
            table: self.table,
            column: self.column,
            lo: lo.into().to_value(),
            hi: hi.into().to_value(),
        })
    }
}

impl<M> Column<M, String> {
    /// `column LIKE '%pattern%'`.
    pub fn contains(self, pattern: &str) -> Filter<M> {
        Filter::new(FilterNode::Cmp {
            table: self.table,
            column: self.column,
            op: CmpOp::Like,
            value: Value::Str(format!("%{pattern}%").into()),
        })
    }

    /// `column LIKE 'pattern%'`.
    pub fn starts_with(self, prefix: &str) -> Filter<M> {
        Filter::new(FilterNode::Cmp {
            table: self.table,
            column: self.column,
            op: CmpOp::Like,
            value: Value::Str(format!("{prefix}%").into()),
        })
    }

    /// `column LIKE '%pattern'`.
    pub fn ends_with(self, suffix: &str) -> Filter<M> {
        Filter::new(FilterNode::Cmp {
            table: self.table,
            column: self.column,
            op: CmpOp::Like,
            value: Value::Str(format!("%{suffix}").into()),
        })
    }

    /// Case-insensitive pattern match (`ILIKE` on Postgres, `LIKE` on SQLite).
    pub fn ilike(self, pattern: &str) -> Filter<M> {
        Filter::new(FilterNode::Cmp {
            table: self.table,
            column: self.column,
            op: CmpOp::Ilike,
            value: Value::Str(format!("%{pattern}%").into()),
        })
    }
}

impl<M, T> Column<M, Option<T>> {
    /// `column IS NULL`.
    pub fn is_null(self) -> Filter<M> {
        Filter::new(FilterNode::Null {
            table: self.table,
            column: self.column,
            negated: false,
        })
    }

    /// `column IS NOT NULL`.
    pub fn is_not_null(self) -> Filter<M> {
        Filter::new(FilterNode::Null {
            table: self.table,
            column: self.column,
            negated: true,
        })
    }
}
