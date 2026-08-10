//! Query builders.

use std::marker::PhantomData;

use crate::Error;
use crate::col::{Column, Projection};
use crate::compile::{CompiledSql, delete, dialect_for_pool, insert, select, update};
use crate::filter::{Filter, FilterNode};
use crate::model::Model;
use crate::order::OrderBy;
use crate::pool::Pool;
use crate::value::{Encodable, Value};

/// A typed `SELECT` query.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct SelectQuery<'db, M: Model, Out = M> {
    pool: &'db Pool,
    filter: Filter<M>,
    projection: Vec<&'static str>,
    order: Vec<OrderBy<M>>,
    limit: Option<u64>,
    offset: Option<u64>,
    distinct: bool,
    _out: PhantomData<fn() -> Out>,
}

impl<'db, M, Out> SelectQuery<'db, M, Out>
where
    M: Model,
{
    /// Creates a new query.
    #[must_use]
    pub const fn new(pool: &'db Pool) -> Self {
        Self {
            pool,
            filter: Filter::new(FilterNode::And(Vec::new())),
            projection: Vec::new(),
            order: Vec::new(),
            limit: None,
            offset: None,
            distinct: false,
            _out: PhantomData,
        }
    }

    /// Adds a filter (`AND`).
    pub fn filter(self, f: Filter<M>) -> Self {
        Self {
            filter: self.filter.and(f),
            ..self
        }
    }

    /// Adds a filter (`OR`).
    pub fn or_filter(self, f: Filter<M>) -> Self {
        Self {
            filter: self.filter.or(f),
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

    /// Makes the query `SELECT DISTINCT`.
    pub fn distinct(self) -> Self {
        Self {
            distinct: true,
            ..self
        }
    }

    /// Restricts the selected columns and changes the output type.
    pub fn columns<P: Projection<M>>(self, p: P) -> SelectQuery<'db, M, P::Output> {
        SelectQuery {
            pool: self.pool,
            filter: self.filter,
            projection: p.projection(),
            order: self.order,
            limit: self.limit,
            offset: self.offset,
            distinct: self.distinct,
            _out: PhantomData,
        }
    }

    /// Compiles the query to SQL and binds.
    pub fn to_sql(&self) -> CompiledSql {
        let dialect = dialect_for_pool(self.pool);
        select::<M>(
            dialect.as_ref(),
            M::TABLE,
            &self.projection,
            &self.filter.node,
            &self.order,
            self.limit,
            self.offset,
            self.distinct,
        )
    }

    /// Executes the query and returns all matching rows.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Sqlx`] for database errors.
    pub async fn fetch_all(self) -> Result<Vec<Out>, Error>
    where
        Out: Send + Unpin + for<'r> sqlx::FromRow<'r, sqlx::any::AnyRow>,
    {
        let compiled = self.to_sql();
        let mut q = sqlx::query_as::<sqlx::Any, Out>(compiled.sql.as_str());
        for v in compiled.binds {
            q = q.bind(v);
        }
        q.fetch_all(self.pool).await.map_err(Error::Sqlx)
    }

    /// Executes the query and returns the first row, if any.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Sqlx`] for database errors.
    pub async fn fetch_optional(self) -> Result<Option<Out>, Error>
    where
        Out: Send + Unpin + for<'r> sqlx::FromRow<'r, sqlx::any::AnyRow>,
    {
        let compiled = self.to_sql();
        let mut q = sqlx::query_as::<sqlx::Any, Out>(compiled.sql.as_str());
        for v in compiled.binds {
            q = q.bind(v);
        }
        q.fetch_optional(self.pool).await.map_err(Error::Sqlx)
    }

    /// Executes the query and returns exactly one row.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Sqlx`] for database errors, including the case where no
    /// row matches.
    pub async fn fetch_one(self) -> Result<Out, Error>
    where
        Out: Send + Unpin + for<'r> sqlx::FromRow<'r, sqlx::any::AnyRow>,
    {
        self.fetch_optional()
            .await?
            .ok_or_else(|| Error::Message("no row found for query".into()))
    }

    /// Returns the number of rows the query would return.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Sqlx`] for database errors.
    pub async fn count(self) -> Result<i64, Error> {
        let mut compiled = self.to_sql();
        // Replace the leading `SELECT ... FROM` with `SELECT COUNT(*) FROM`.
        if let Some(from_pos) = compiled.sql.find(" FROM ") {
            let rest = compiled.sql.split_off(from_pos);
            compiled.sql = format!("SELECT COUNT(*) {rest}");
        }

        let mut q = sqlx::query_scalar::<sqlx::Any, i64>(compiled.sql.as_str());
        for v in compiled.binds {
            q = q.bind(v);
        }
        q.fetch_one(self.pool).await.map_err(Error::Sqlx)
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

    /// Sets a column to a value.
    pub fn set<V: Encodable>(mut self, col: Column<M, V>, value: impl Into<V>) -> Self {
        self.values.push((col.column, value.into().to_value()));
        self
    }

    /// Sets a column only if `value` is `Some`.
    pub fn set_optional<V: Encodable>(
        self,
        col: Column<M, V>,
        value: Option<impl Into<V>>,
    ) -> Self {
        match value {
            Some(v) => self.set(col, v),
            None => self,
        }
    }

    /// Compiles the query to SQL and binds.
    pub fn to_sql(&self) -> CompiledSql {
        let dialect = dialect_for_pool(self.pool);
        insert::<M>(dialect.as_ref(), M::TABLE, &self.values, &[])
    }

    /// Executes the insert and returns the inserted row.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Sqlx`] for database errors.
    pub async fn exec(self) -> Result<M, Error>
    where
        M: Send + Unpin + for<'r> sqlx::FromRow<'r, sqlx::any::AnyRow>,
    {
        let dialect = dialect_for_pool(self.pool);
        let compiled = insert::<M>(dialect.as_ref(), M::TABLE, &self.values, &["*"]);
        let mut q = sqlx::query_as::<sqlx::Any, M>(compiled.sql.as_str());
        for v in compiled.binds {
            q = q.bind(v);
        }
        q.fetch_one(self.pool).await.map_err(Error::Sqlx)
    }
}

/// A typed `UPDATE` query.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct UpdateQuery<'db, M: Model> {
    pool: &'db Pool,
    sets: Vec<(&'static str, Value)>,
    filter: Filter<M>,
    all_rows: bool,
    _marker: PhantomData<fn() -> M>,
}

impl<'db, M: Model> UpdateQuery<'db, M> {
    /// Creates a new query.
    #[must_use]
    pub const fn new(pool: &'db Pool) -> Self {
        Self {
            pool,
            sets: Vec::new(),
            filter: Filter::new(FilterNode::And(Vec::new())),
            all_rows: false,
            _marker: PhantomData,
        }
    }

    /// Adds a filter.
    pub fn filter(self, f: Filter<M>) -> Self {
        Self {
            filter: self.filter.and(f),
            ..self
        }
    }

    /// Sets an explicit value.
    pub fn set<V: Encodable>(mut self, col: Column<M, V>, value: impl Into<V>) -> Self {
        self.sets.push((col.column, value.into().to_value()));
        self
    }

    /// Sets a column to `NULL`.
    pub fn set_null<V: Encodable>(mut self, col: Column<M, V>) -> Self {
        self.sets.push((col.column, Value::Null));
        self
    }

    /// Allows updating all rows. Without this, `exec` returns an error if no
    /// filter was supplied.
    pub fn all_rows(mut self) -> Self {
        self.all_rows = true;
        self
    }

    /// Compiles the query to SQL and binds.
    pub fn to_sql(&self) -> Result<CompiledSql, Error> {
        if !self.all_rows && matches!(self.filter.node, FilterNode::And(ref v) if v.is_empty()) {
            return Err(Error::Message(
                "update has no filter; call .all_rows() to update every row".into(),
            ));
        }
        if self.sets.is_empty() {
            return Err(Error::Message("update has no columns to set".into()));
        }
        let dialect = dialect_for_pool(self.pool);
        Ok(update::<M>(
            dialect.as_ref(),
            M::TABLE,
            &self.sets,
            &self.filter.node,
            &[],
        ))
    }

    /// Executes the update and returns the number of rows affected.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Sqlx`] for database errors.
    pub async fn exec(self) -> Result<u64, Error> {
        let compiled = self.to_sql()?;
        let mut q = sqlx::query::<sqlx::Any>(compiled.sql.as_str());
        for v in compiled.binds {
            q = q.bind(v);
        }
        q.execute(self.pool)
            .await
            .map(|r| r.rows_affected())
            .map_err(Error::Sqlx)
    }
}

/// A typed `DELETE` query.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct DeleteQuery<'db, M: Model> {
    pool: &'db Pool,
    filter: Filter<M>,
    all_rows: bool,
    _marker: PhantomData<fn() -> M>,
}

impl<'db, M: Model> DeleteQuery<'db, M> {
    /// Creates a new query.
    #[must_use]
    pub const fn new(pool: &'db Pool) -> Self {
        Self {
            pool,
            filter: Filter::new(FilterNode::And(Vec::new())),
            all_rows: false,
            _marker: PhantomData,
        }
    }

    /// Adds a filter.
    pub fn filter(self, f: Filter<M>) -> Self {
        Self {
            filter: self.filter.and(f),
            ..self
        }
    }

    /// Allows deleting all rows. Without this, `exec` returns an error if no
    /// filter was supplied.
    pub fn all_rows(mut self) -> Self {
        self.all_rows = true;
        self
    }

    /// Compiles the query to SQL and binds.
    pub fn to_sql(&self) -> Result<CompiledSql, Error> {
        if !self.all_rows && matches!(self.filter.node, FilterNode::And(ref v) if v.is_empty()) {
            return Err(Error::Message(
                "delete has no filter; call .all_rows() to delete every row".into(),
            ));
        }
        let dialect = dialect_for_pool(self.pool);
        Ok(delete::<M>(
            dialect.as_ref(),
            M::TABLE,
            &self.filter.node,
            &[],
        ))
    }

    /// Executes the delete and returns the number of rows removed.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Sqlx`] for database errors.
    pub async fn exec(self) -> Result<u64, Error> {
        let compiled = self.to_sql()?;
        let mut q = sqlx::query::<sqlx::Any>(compiled.sql.as_str());
        for v in compiled.binds {
            q = q.bind(v);
        }
        q.execute(self.pool)
            .await
            .map(|r| r.rows_affected())
            .map_err(Error::Sqlx)
    }
}
