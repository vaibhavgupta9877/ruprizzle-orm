//! Filter algebra and constructors.

use std::marker::PhantomData;

use crate::value::Value;

/// A predicate that is tied to a model `M`.
#[derive(Debug, Clone, PartialEq)]
pub struct Filter<M> {
    /// The root filter node.
    pub node: FilterNode,
    _marker: PhantomData<fn() -> M>,
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
    And(Vec<FilterNode>),
    Or(Vec<FilterNode>),
    Not(Box<FilterNode>),
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
