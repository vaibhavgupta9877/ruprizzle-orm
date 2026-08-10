//! Query builders.

use std::marker::PhantomData;

use crate::Error;
use crate::col::{Column, Projection};
use crate::compile::{
    CompiledSql, delete, dialect_for_pool, insert, insert_many, select, update, upsert,
};
use crate::filter::{Filter, FilterNode};
use crate::include::IncludeSet;
use crate::model::Model;
use crate::order::OrderBy;
use crate::pool::Pool;
use crate::value::{Encodable, Ordered, Value};

/// A typed `SELECT` query.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct SelectQuery<'db, M: Model, Out = M, I = ()> {
    pool: &'db Pool,
    filter: Filter<M>,
    projection: Vec<&'static str>,
    order: Vec<OrderBy<M>>,
    limit: Option<u64>,
    offset: Option<u64>,
    distinct: bool,
    includes: I,
    _out: PhantomData<fn() -> Out>,
}

impl<'db, M> SelectQuery<'db, M, M, ()>
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
            includes: (),
            _out: PhantomData,
        }
    }
}

impl<'db, M, Out, I> SelectQuery<'db, M, Out, I>
where
    M: Model,
{
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

    /// Returns the first `n` rows after a cursor value (exclusive).
    pub fn after<V: Ordered>(self, col: Column<M, V>, value: impl Into<V>, n: u64) -> Self {
        self.filter(col.gt(value)).order_by(col.asc()).limit(n)
    }

    /// Returns the first `n` rows before a cursor value (exclusive).
    pub fn before<V: Ordered>(self, col: Column<M, V>, value: impl Into<V>, n: u64) -> Self {
        self.filter(col.lt(value)).order_by(col.desc()).limit(n)
    }

    /// Makes the query `SELECT DISTINCT`.
    pub fn distinct(self) -> Self {
        Self {
            distinct: true,
            ..self
        }
    }

    /// Includes a related model, loaded in a second batched query.
    pub fn include<J: IncludeSet<M>>(self, include: J) -> SelectQuery<'db, M, Out, J> {
        SelectQuery {
            pool: self.pool,
            filter: self.filter,
            projection: self.projection,
            order: self.order,
            limit: self.limit,
            offset: self.offset,
            distinct: self.distinct,
            includes: include,
            _out: PhantomData,
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
            includes: (),
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

impl<'db, M, I> SelectQuery<'db, M, M, I>
where
    M: Model + Send + Unpin + for<'r> sqlx::FromRow<'r, sqlx::any::AnyRow>,
    I: IncludeSet<M>,
{
    /// Executes the query, then loads any requested includes.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Sqlx`] for database errors.
    pub async fn exec(self) -> Result<Vec<M>, Error> {
        let compiled = self.to_sql();
        let mut q = sqlx::query_as::<sqlx::Any, M>(compiled.sql.as_str());
        for v in compiled.binds {
            q = q.bind(v);
        }
        let mut rows = q.fetch_all(self.pool).await.map_err(Error::Sqlx)?;
        self.includes.load(self.pool, &mut rows).await?;
        Ok(rows)
    }
}

/// A typed `INSERT` query.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct InsertQuery<'db, M: Model> {
    pool: &'db Pool,
    values: Vec<(&'static str, Value)>,
    on_conflict: Option<Vec<&'static str>>,
    do_update: Option<Vec<&'static str>>,
    _marker: PhantomData<fn() -> M>,
}

impl<'db, M: Model> InsertQuery<'db, M> {
    /// Creates a new query.
    #[must_use]
    pub const fn new(pool: &'db Pool) -> Self {
        Self {
            pool,
            values: Vec::new(),
            on_conflict: None,
            do_update: None,
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

    /// Sets the conflict target for an upsert.
    pub fn on_conflict(mut self, cols: impl IntoIterator<Item = &'static str>) -> Self {
        self.on_conflict = Some(cols.into_iter().collect());
        self
    }

    /// Sets which columns to update on conflict (enables `DO UPDATE`).
    pub fn do_update(mut self, cols: impl IntoIterator<Item = &'static str>) -> Self {
        self.do_update = Some(cols.into_iter().collect());
        self
    }

    /// Compiles the query to SQL and binds.
    pub fn to_sql(&self) -> CompiledSql {
        let dialect = dialect_for_pool(self.pool);
        if let Some(ref conflict) = self.on_conflict {
            let do_update = self.do_update.as_deref().unwrap_or(&[]);
            upsert::<M>(
                dialect.as_ref(),
                M::TABLE,
                &self.values,
                conflict,
                do_update,
                &[],
            )
        } else {
            insert::<M>(dialect.as_ref(), M::TABLE, &self.values, &[])
        }
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
        let compiled = if let Some(ref conflict) = self.on_conflict {
            let do_update = self.do_update.as_deref().unwrap_or(&[]);
            upsert::<M>(
                dialect.as_ref(),
                M::TABLE,
                &self.values,
                conflict,
                do_update,
                &["*"],
            )
        } else {
            insert::<M>(dialect.as_ref(), M::TABLE, &self.values, &["*"])
        };
        let mut q = sqlx::query_as::<sqlx::Any, M>(compiled.sql.as_str());
        for v in compiled.binds {
            q = q.bind(v);
        }
        q.fetch_one(self.pool).await.map_err(Error::Sqlx)
    }
}

/// A typed multi-row `INSERT` query.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct InsertManyQuery<'db, M: Model> {
    pool: &'db Pool,
    rows: Vec<Vec<(&'static str, Value)>>,
    _marker: PhantomData<fn() -> M>,
}

impl<'db, M: Model> InsertManyQuery<'db, M> {
    /// Creates a new query.
    #[must_use]
    pub const fn new(pool: &'db Pool) -> Self {
        Self {
            pool,
            rows: Vec::new(),
            _marker: PhantomData,
        }
    }

    /// Adds a row to insert.
    pub fn row(mut self, columns: impl IntoIterator<Item = (&'static str, Value)>) -> Self {
        self.rows.push(columns.into_iter().collect());
        self
    }

    /// Executes the insert, returning all inserted rows.
    ///
    /// The rows are chunked to stay under the database's parameter limit.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Sqlx`] for database errors.
    pub async fn exec(self) -> Result<Vec<M>, Error>
    where
        M: Send + Unpin + for<'r> sqlx::FromRow<'r, sqlx::any::AnyRow>,
    {
        if self.rows.is_empty() {
            return Ok(Vec::new());
        }

        let dialect = dialect_for_pool(self.pool);
        let max = dialect.capabilities().max_query_params;
        let cols_per_row = self.rows[0].len() as u32;
        let chunk_size = (max / cols_per_row).max(1) as usize;

        let mut out = Vec::new();
        for chunk in self.rows.chunks(chunk_size) {
            let compiled = insert_many::<M>(dialect.as_ref(), M::TABLE, chunk, &["*"]);
            let mut q = sqlx::query_as::<sqlx::Any, M>(compiled.sql.as_str());
            for v in compiled.binds {
                q = q.bind(v);
            }
            let mut rows = q.fetch_all(self.pool).await.map_err(Error::Sqlx)?;
            out.append(&mut rows);
        }
        Ok(out)
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

/// Marker for a `DeleteQuery` that has not yet received a filter or
/// `.all_rows()` call.
pub struct UnfilteredDelete;

/// Marker for a `DeleteQuery` that is safe to execute.
pub struct FilteredDelete;

/// A typed `DELETE` query.
///
/// `exec` is only available when `S = FilteredDelete`, which is reached by
/// calling `.filter(...)` or `.all_rows()`.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct DeleteQuery<'db, M: Model, S = UnfilteredDelete> {
    pool: &'db Pool,
    filter: Filter<M>,
    all_rows: bool,
    _state: PhantomData<fn() -> S>,
    _marker: PhantomData<fn() -> M>,
}

impl<'db, M: Model> DeleteQuery<'db, M, UnfilteredDelete> {
    /// Creates a new unfiltered delete query.
    #[must_use]
    pub const fn new(pool: &'db Pool) -> Self {
        Self {
            pool,
            filter: Filter::new(FilterNode::And(Vec::new())),
            all_rows: false,
            _state: PhantomData,
            _marker: PhantomData,
        }
    }

    /// Adds a filter.
    pub fn filter(self, f: Filter<M>) -> DeleteQuery<'db, M, FilteredDelete> {
        DeleteQuery {
            pool: self.pool,
            filter: self.filter.and(f),
            all_rows: false,
            _state: PhantomData,
            _marker: PhantomData,
        }
    }

    /// Allows deleting all rows.
    pub fn all_rows(self) -> DeleteQuery<'db, M, FilteredDelete> {
        DeleteQuery {
            pool: self.pool,
            filter: self.filter,
            all_rows: true,
            _state: PhantomData,
            _marker: PhantomData,
        }
    }
}

impl<'db, M: Model> DeleteQuery<'db, M, FilteredDelete> {
    /// Adds an additional filter.
    pub fn filter(self, f: Filter<M>) -> Self {
        Self {
            filter: self.filter.and(f),
            ..self
        }
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
