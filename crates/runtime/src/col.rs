//! Typed column tokens.

use std::marker::PhantomData;
use std::sync::Arc;

use crate::filter::{CmpOp, Filter, FilterNode, JsonFilterOp, Subquery};
use crate::join::JoinOn;
use crate::json::{JsonColumn, JsonPath, JsonPathSegment, JsonSet};
use crate::order::OrderBy;
use crate::value::{Encodable, Ordered, Value};

/// A typed token for a physical column.
///
/// The `M` phantom type prevents mixing columns from different models into one
/// query. The `T` phantom type is the Rust type stored in the column, which is
/// used to type-check filter values at compile time.
pub struct Column<M, T> {
    /// The SQL table name.
    pub table: &'static str,
    /// The SQL column name.
    pub column: &'static str,
    _marker: PhantomData<fn() -> (M, T)>,
}

impl<M, T> std::fmt::Debug for Column<M, T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Column")
            .field("table", &self.table)
            .field("column", &self.column)
            .finish()
    }
}

impl<M, T> Copy for Column<M, T> {}
impl<M, T> Clone for Column<M, T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<M, T> PartialEq for Column<M, T> {
    fn eq(&self, other: &Self) -> bool {
        self.table == other.table && self.column == other.column
    }
}

impl<M, T> Eq for Column<M, T> {}

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

    /// Reference the same column in a join condition on the right side of another model.
    pub fn on<J>(self, other: Column<J, T>) -> JoinOn {
        JoinOn::new(self, other)
    }

    /// Use this column token with a different table name, typically a self-join alias.
    ///
    /// The model marker `M` is preserved, so the resulting column still has the
    /// same Rust type; only the SQL qualifier changes. This makes self-joins
    /// type-safe without needing a separate alias type.
    #[must_use]
    pub const fn aliased(self, table: &'static str) -> Self {
        Self {
            table,
            column: self.column,
            _marker: PhantomData,
        }
    }

    /// `column IN (subquery)`.
    pub fn in_subquery<Q: Into<Subquery<T>>>(self, subquery: Q) -> Filter<M> {
        let subquery = subquery.into().compiled;
        Filter::new(FilterNode::InSubquery {
            table: self.table,
            column: self.column,
            subquery,
            negated: false,
        })
    }

    /// `column NOT IN (subquery)`.
    pub fn not_in_subquery<Q: Into<Subquery<T>>>(self, subquery: Q) -> Filter<M> {
        let subquery = subquery.into().compiled;
        Filter::new(FilterNode::InSubquery {
            table: self.table,
            column: self.column,
            subquery,
            negated: true,
        })
    }

    /// Correlate this column to an outer query's column (`inner_col = outer_col`).
    #[must_use]
    pub fn correlated_to<J>(self, other: Column<J, T>) -> Filter<M> {
        Filter::new(FilterNode::ColumnCmp {
            left_table: self.table,
            left_col: self.column,
            op: CmpOp::Eq,
            right_table: other.table,
            right_col: other.column,
        })
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

    /// `column IN (values)`.
    pub fn in_set(self, values: impl IntoIterator<Item = impl Into<T>>) -> Filter<M> {
        Filter::new(FilterNode::In {
            table: self.table,
            column: self.column,
            values: values.into_iter().map(|v| v.into().to_value()).collect(),
            negated: false,
        })
    }

    /// `column NOT IN (values)`.
    pub fn not_in_set(self, values: impl IntoIterator<Item = impl Into<T>>) -> Filter<M> {
        Filter::new(FilterNode::In {
            table: self.table,
            column: self.column,
            values: values.into_iter().map(|v| v.into().to_value()).collect(),
            negated: true,
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

impl<M> Column<M, serde_json::Value> {
    /// Extract a JSON field by key (`column->'key'` or `column#>'{...}'`).
    #[must_use]
    pub fn get(self, key: &'static str) -> JsonColumn<M, serde_json::Value> {
        JsonColumn {
            table: self.table,
            column: self.column,
            path: JsonPath(vec![JsonPathSegment::Key(key)]),
            text: false,
            _marker: PhantomData,
        }
    }

    /// Extract a JSON field by key as text (`column->>'key'` or `column#>>'{...}'`).
    #[must_use]
    pub fn get_text(self, key: &'static str) -> JsonColumn<M, String> {
        JsonColumn {
            table: self.table,
            column: self.column,
            path: JsonPath(vec![JsonPathSegment::Key(key)]),
            text: true,
            _marker: PhantomData,
        }
    }

    /// `column @> value` (JSON containment).
    pub fn contains(self, value: serde_json::Value) -> Filter<M> {
        Filter::new(FilterNode::Json {
            table: self.table,
            column: self.column,
            path: JsonPath::default(),
            text: false,
            op: JsonFilterOp::Contains,
            value: value.to_value(),
        })
    }

    /// `column ? key` (top-level key existence).
    pub fn has_key(self, key: impl Into<String>) -> Filter<M> {
        Filter::new(FilterNode::Json {
            table: self.table,
            column: self.column,
            path: JsonPath::default(),
            text: false,
            op: JsonFilterOp::HasKey,
            value: Value::Str(Arc::from(key.into())),
        })
    }

    /// Build a `jsonb_set` expression for this column and key.
    pub fn jsonb_set(self, key: &'static str, value: serde_json::Value) -> JsonSet {
        JsonSet {
            column: self.column,
            path: JsonPath(vec![JsonPathSegment::Key(key)]),
            value: value.to_value(),
        }
    }
}

/// A selection of columns that changes the output type of a
/// [`SelectQuery`](crate::query::SelectQuery).
pub trait Projection<M> {
    /// The Rust type a row of the projection decodes into.
    type Output;

    /// Returns the list of column names to select.
    fn projection(&self) -> Vec<&'static str>;
}

impl<M, T> Projection<M> for Column<M, T> {
    type Output = (T,);

    fn projection(&self) -> Vec<&'static str> {
        vec![self.column]
    }
}

macro_rules! impl_projection_tuples {
    ($($n:tt $T:ident),+) => {
        impl<M, $($T),+> Projection<M> for ($(Column<M, $T>,)+) {
            type Output = ($($T,)+);

            #[allow(clippy::vec_init_then_push)]
            fn projection(&self) -> Vec<&'static str> {
                let mut cols = Vec::new();
                $(
                    cols.push(self.$n.column);
                )+
                cols
            }
        }
    };
}

impl_projection_tuples! { 0 T0 }
impl_projection_tuples! { 0 T0, 1 T1 }
impl_projection_tuples! { 0 T0, 1 T1, 2 T2 }
impl_projection_tuples! { 0 T0, 1 T1, 2 T2, 3 T3 }
impl_projection_tuples! { 0 T0, 1 T1, 2 T2, 3 T3, 4 T4 }
impl_projection_tuples! { 0 T0, 1 T1, 2 T2, 3 T3, 4 T4, 5 T5 }
impl_projection_tuples! { 0 T0, 1 T1, 2 T2, 3 T3, 4 T4, 5 T5, 6 T6 }
impl_projection_tuples! { 0 T0, 1 T1, 2 T2, 3 T3, 4 T4, 5 T5, 6 T6, 7 T7 }
