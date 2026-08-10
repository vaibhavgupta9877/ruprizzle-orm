//! Query builders.

use std::marker::PhantomData;

use crate::filter::{Filter, FilterNode};
use crate::model::Model;
use crate::order::OrderBy;
use crate::pool::Pool;
use crate::value::{Encodable, Value};
use crate::{Column, Error};

/// A typed `SELECT` query.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct SelectQuery<'db, M: Model, Out = M> {
    pool: &'db Pool,
    filter: Filter<M>,
    order: Vec<OrderBy<M>>,
    limit: Option<u64>,
    offset: Option<u64>,
    _out: PhantomData<fn() -> Out>,
}

impl<'db, M: Model, Out> SelectQuery<'db, M, Out> {
    /// Creates a new query.
    #[must_use]
    pub const fn new(pool: &'db Pool) -> Self {
        Self {
            pool,
            filter: Filter::new(FilterNode::And(Vec::new())),
            order: Vec::new(),
            limit: None,
            offset: None,
            _out: PhantomData,
        }
    }

    /// Adds a filter.
    pub fn filter(self, f: Filter<M>) -> Self {
        Self {
            filter: self.filter.and(f),
            ..self
        }
    }

    /// Adds an ordering.
    pub fn order_by(self, o: OrderBy<M>) -> Self {
        let mut order = self.order;
        order.push(o);
        Self { order, ..self }
    }

    /// Sets the limit.
    pub fn limit(self, n: u64) -> Self {
        Self {
            limit: Some(n),
            ..self
        }
    }

    /// Sets the offset.
    pub fn offset(self, n: u64) -> Self {
        Self {
            offset: Some(n),
            ..self
        }
    }

    /// Executes the query.
    ///
    /// # Errors
    ///
    /// Returns an error if the query cannot be executed.
    pub async fn fetch_all(self) -> Result<Vec<Out>, Error> {
        // P4: compile the filter and run it through sqlx.
        let () = std::future::ready(()).await;
        Err(Error::NotImplemented)
    }
}

/// A typed `INSERT` query.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct InsertQuery<'db, M: Model> {
    pool: &'db Pool,
    values: Vec<(&'static str, Value)>,
    _marker: PhantomData<fn() -> M>,
}

impl<'db, M: Model> InsertQuery<'db, M> {
    /// Creates a new query.
    #[must_use]
    pub const fn new(pool: &'db Pool) -> Self {
        Self {
            pool,
            values: Vec::new(),
            _marker: PhantomData,
        }
    }

    /// Sets a column value.
    pub fn set<V: Encodable>(mut self, col: Column<M, V>, value: impl Into<V>) -> Self {
        self.values.push((col.column, value.into().to_value()));
        self
    }

    /// Executes the query.
    ///
    /// # Errors
    ///
    /// Returns an error if the query cannot be executed.
    pub async fn exec(self) -> Result<M, Error> {
        let () = std::future::ready(()).await;
        Err(Error::NotImplemented)
    }
}
