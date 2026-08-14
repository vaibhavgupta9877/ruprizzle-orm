//! Filter algebra and constructors.

use std::marker::PhantomData;

use crate::compile::CompiledSql;
use crate::json::JsonPath;
use crate::model::Model;
use crate::query::SelectQuery;
use crate::value::Value;

/// An injection-safe raw SQL fragment with bound parameters.
///
/// The literal parts are stored separately, and values are bound as
/// parameters. Calling [`RawFragment::sql`] returns the fragment with
/// PostgreSQL-style placeholders (`$1`, `$2`, ...).
#[derive(Debug, Clone, PartialEq)]
pub struct RawFragment {
    /// Literal fragments of the SQL, between placeholders.
    pub(crate) parts: Vec<String>,
    /// Bound values for each placeholder.
    pub(crate) binds: Vec<Value>,
}

impl RawFragment {
    /// Creates a new raw fragment.
    #[must_use]
    pub fn new(parts: Vec<String>, binds: Vec<Value>) -> Self {
        assert_eq!(
            parts.len(),
            binds.len() + 1,
            "RawFragment parts must split the format string around each placeholder"
        );
        Self { parts, binds }
    }

    /// Returns the SQL fragment with `$1`, `$2`, ... placeholders.
    #[must_use]
    pub fn sql(&self) -> String {
        let mut out = String::new();
        for (i, part) in self.parts.iter().enumerate() {
            out.push_str(part);
            if i < self.binds.len() {
                out.push_str(&format!("${}", i + 1));
            }
        }
        out
    }

    /// Returns the bound values for this fragment.
    #[must_use]
    pub fn binds(&self) -> &[Value] {
        &self.binds
    }
}

/// A compiled `SELECT` subquery that returns a single column of type `T`.
///
/// It is typically produced by `SelectQuery::columns(...).into::<Subquery<T>>()`
/// and consumed by [`Column::in_subquery`](crate::col::Column::in_subquery).
pub struct Subquery<T> {
    /// The compiled SQL of the subquery, including its bound values.
    pub(crate) compiled: CompiledSql,
    _marker: PhantomData<fn() -> T>,
}

impl<T> Clone for Subquery<T> {
    fn clone(&self) -> Self {
        Self {
            compiled: self.compiled.clone(),
            _marker: PhantomData,
        }
    }
}

impl<T> std::fmt::Debug for Subquery<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Subquery")
            .field("compiled", &self.compiled)
            .finish()
    }
}

impl<T> PartialEq for Subquery<T> {
    fn eq(&self, other: &Self) -> bool {
        self.compiled == other.compiled
    }
}

impl<'db, M, T, I> From<SelectQuery<'db, M, (T,), I>> for Subquery<T>
where
    M: Model,
{
    fn from(query: SelectQuery<'db, M, (T,), I>) -> Self {
        Self {
            compiled: query.to_sql().unwrap_or_else(|e| {
                panic!("subquery SQL is not supported by the target dialect: {e}")
            }),
            _marker: PhantomData,
        }
    }
}

/// A compiled subquery used by `EXISTS` / `NOT EXISTS` filters.
///
/// Unlike [`Subquery`], `EXISTS` does not care about the projection shape,
/// so any [`SelectQuery`](crate::query::SelectQuery) can be converted into an `ExistsSubquery`.
pub struct ExistsSubquery {
    pub(crate) compiled: CompiledSql,
}

impl Clone for ExistsSubquery {
    fn clone(&self) -> Self {
        Self {
            compiled: self.compiled.clone(),
        }
    }
}

impl std::fmt::Debug for ExistsSubquery {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ExistsSubquery")
            .field("compiled", &self.compiled)
            .finish()
    }
}

impl PartialEq for ExistsSubquery {
    fn eq(&self, other: &Self) -> bool {
        self.compiled == other.compiled
    }
}

impl<'db, M, Out, I> From<SelectQuery<'db, M, Out, I>> for ExistsSubquery
where
    M: Model,
{
    fn from(query: SelectQuery<'db, M, Out, I>) -> Self {
        Self {
            compiled: query.to_sql().unwrap_or_else(|e| {
                panic!("EXISTS subquery SQL is not supported by the target dialect: {e}")
            }),
        }
    }
}

/// A compiled common table expression (CTE).
///
/// Stored inside a [`SelectQuery`](crate::query::SelectQuery) and emitted as
/// `WITH ... AS (...)` or `WITH RECURSIVE ... AS (...)` when the query is
/// compiled.
#[derive(Debug, Clone, PartialEq)]
pub struct Cte {
    /// The CTE name.
    pub name: &'static str,
    /// The compiled body.
    pub compiled: CompiledSql,
    /// Whether this CTE is recursive.
    pub recursive: bool,
}

impl Cte {
    /// Creates a new non-recursive CTE.
    #[must_use]
    pub fn new(name: &'static str, compiled: CompiledSql) -> Self {
        Self {
            name,
            compiled,
            recursive: false,
        }
    }
}

/// A compiled query that can be used as the body of a CTE.
///
/// This is a thin wrapper around [`CompiledSql`] so that
/// [`SelectQuery::with`](crate::query::SelectQuery::with) and
/// [`SelectQuery::with_recursive`](crate::query::SelectQuery::with_recursive)
/// can accept anything that converts into it.
#[derive(Debug, Clone, PartialEq)]
pub struct CteQuery {
    pub(crate) compiled: CompiledSql,
}

impl CteQuery {
    pub(crate) fn new(compiled: CompiledSql) -> Self {
        Self { compiled }
    }
}

impl<'db, M, Out, I> From<SelectQuery<'db, M, Out, I>> for CteQuery
where
    M: Model,
{
    fn from(query: SelectQuery<'db, M, Out, I>) -> Self {
        Self::new(
            query
                .to_sql()
                .unwrap_or_else(|e| panic!("CTE SQL is not supported by the target dialect: {e}")),
        )
    }
}

/// A predicate that is tied to a model `M`.
pub struct Filter<M> {
    /// The root filter node.
    pub node: FilterNode,
    _marker: PhantomData<fn() -> M>,
}

impl<M> std::fmt::Debug for Filter<M> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Filter").field("node", &self.node).finish()
    }
}

impl<M> Clone for Filter<M> {
    fn clone(&self) -> Self {
        Self {
            node: self.node.clone(),
            _marker: PhantomData,
        }
    }
}

impl<M> PartialEq for Filter<M> {
    fn eq(&self, other: &Self) -> bool {
        self.node == other.node
    }
}

impl<M> Filter<M> {
    /// Creates a new filter.
    #[must_use]
    pub const fn new(node: FilterNode) -> Self {
        Self {
            node,
            _marker: PhantomData,
        }
    }

    /// Combines two filters with `AND`.
    #[must_use]
    pub fn and(self, other: Self) -> Self {
        Self::new(flatten_and(vec![self.node, other.node]))
    }

    /// Combines two filters with `OR`.
    #[must_use]
    pub fn or(self, other: Self) -> Self {
        Self::new(flatten_or(vec![self.node, other.node]))
    }

    /// Creates a filter from a raw SQL fragment.
    #[must_use]
    pub fn raw(fragment: RawFragment) -> Self {
        Self::new(FilterNode::Raw(fragment))
    }

    /// `EXISTS (subquery)`.
    #[must_use]
    pub fn exists(subquery: impl Into<ExistsSubquery>) -> Self {
        Self::new(FilterNode::ExistsSubquery {
            subquery: subquery.into().compiled,
            negated: false,
        })
    }

    /// `NOT EXISTS (subquery)`.
    #[must_use]
    pub fn not_exists(subquery: impl Into<ExistsSubquery>) -> Self {
        Self::new(FilterNode::ExistsSubquery {
            subquery: subquery.into().compiled,
            negated: true,
        })
    }
}

impl<M> std::ops::Not for Filter<M> {
    type Output = Self;

    fn not(self) -> Self::Output {
        Self::new(FilterNode::Not(Box::new(self.node)))
    }
}

/// A filter node, independent of the model it operates on.
#[derive(Debug, Clone, PartialEq)]
#[allow(missing_docs)]
pub enum FilterNode {
    Cmp {
        table: &'static str,
        column: &'static str,
        op: CmpOp,
        value: Value,
    },
    Between {
        table: &'static str,
        column: &'static str,
        lo: Value,
        hi: Value,
    },
    Null {
        table: &'static str,
        column: &'static str,
        negated: bool,
    },
    In {
        table: &'static str,
        column: &'static str,
        values: Vec<Value>,
        negated: bool,
    },
    /// `column [NOT] IN (subquery)`.
    InSubquery {
        table: &'static str,
        column: &'static str,
        subquery: CompiledSql,
        negated: bool,
    },
    /// A comparison between two columns, used for join `ON` clauses.
    ColumnCmp {
        left_table: &'static str,
        left_col: &'static str,
        op: CmpOp,
        right_table: &'static str,
        right_col: &'static str,
    },
    /// Correlated `EXISTS (SELECT 1 FROM child_table WHERE child_fk = parent_pk AND filter)`.
    Exists {
        child_table: &'static str,
        child_col: &'static str,
        parent_table: &'static str,
        parent_col: &'static str,
        filter: Box<FilterNode>,
        negated: bool,
    },
    /// Correlated `[NOT] EXISTS (<subquery>)`.
    ExistsSubquery {
        subquery: CompiledSql,
        negated: bool,
    },
    And(Vec<FilterNode>),
    Or(Vec<FilterNode>),
    Not(Box<FilterNode>),
    /// A raw SQL fragment with bound parameters.
    Raw(RawFragment),
    /// A JSON column operation.
    Json {
        /// The SQL table name.
        table: &'static str,
        /// The SQL column name.
        column: &'static str,
        /// The JSON path inside the column.
        path: JsonPath,
        /// `true` for text extraction, `false` for JSON.
        text: bool,
        /// The JSON-specific operator.
        op: JsonFilterOp,
        /// The bound value.
        value: Value,
    },
}

/// JSON-specific filter operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JsonFilterOp {
    /// A scalar comparison (`=`, `<>`, `>`, ...).
    Cmp(CmpOp),
    /// JSON containment (`@>`).
    Contains,
    /// Top-level or nested key existence (`?`).
    HasKey,
}

/// Comparison operators.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(missing_docs)]
pub enum CmpOp {
    Eq,
    Ne,
    Gt,
    Gte,
    Lt,
    Lte,
    Like,
    Ilike,
}

/// Combines a list of filters with `AND`.
///
/// `all([])` is `TRUE`.
pub fn all<M>(filters: impl IntoIterator<Item = Filter<M>>) -> Filter<M> {
    Filter::new(flatten_and(filters.into_iter().map(|f| f.node).collect()))
}

/// Combines a list of filters with `OR`.
///
/// `any([])` is `FALSE`.
pub fn any<M>(filters: impl IntoIterator<Item = Filter<M>>) -> Filter<M> {
    Filter::new(flatten_or(filters.into_iter().map(|f| f.node).collect()))
}

fn flatten_and(nodes: Vec<FilterNode>) -> FilterNode {
    if nodes.is_empty() {
        return FilterNode::And(Vec::new());
    }
    let mut out = Vec::with_capacity(nodes.len());
    for node in nodes {
        match node {
            FilterNode::And(children) => out.extend(children),
            other => out.push(other),
        }
    }
    FilterNode::And(out)
}

fn flatten_or(nodes: Vec<FilterNode>) -> FilterNode {
    if nodes.is_empty() {
        return FilterNode::Or(Vec::new());
    }
    let mut out = Vec::with_capacity(nodes.len());
    for node in nodes {
        match node {
            FilterNode::Or(children) => out.extend(children),
            other => out.push(other),
        }
    }
    FilterNode::Or(out)
}
