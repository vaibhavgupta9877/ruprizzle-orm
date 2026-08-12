//! Query builders.

use std::marker::PhantomData;

use crate::Error;
use crate::col::{Column, Projection};
use crate::compile::{
    CompiledSql, delete, dialect_for_pool, insert, insert_many, select, update, upsert,
};
use crate::executor::Executor;
use crate::filter::{Filter, FilterNode};
use crate::include::IncludeSet;
use crate::model::{Model, RowDecode};
use crate::order::OrderBy;
use crate::page::Page;
use crate::pool::Pool;
use crate::value::{Encodable, Ordered, Value};

/// A typed `SELECT` query.
///
/// Not `Debug`: the executor behind it is a trait object, and requiring
/// `Debug` on `Executor` would exclude perfectly good executors for no gain.
/// Use [`to_sql`](SelectQuery::to_sql) to inspect a query instead — that is the
/// representation anyone debugging actually wants.
#[derive(Clone)]
#[allow(dead_code)]
pub struct SelectQuery<'db, M: Model, Out = M, I = ()> {
    exec: &'db dyn Executor,
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
    ///
    /// Accepts anything that can execute SQL, so the same query runs against a
    /// pool or inside a transaction. `&Pool` and `&Tx` both coerce here.
    #[must_use]
    pub fn new(exec: &'db dyn Executor) -> Self {
        Self {
            exec,
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
            exec: self.exec,
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
            exec: self.exec,
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
        let dialect = self.exec.dialect();
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

    /// Executes the query and returns the first row, if any.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Sqlx`] for database errors.
    pub async fn fetch_optional(self) -> Result<Option<Out>, Error>
    where
        Out: Send + Unpin + RowDecode,
    {
        let mut q = self;
        if q.limit.is_none() {
            q.limit = Some(1);
        }
        let compiled = q.to_sql();
        let batch = q.exec.fetch_all_raw(compiled.sql, compiled.binds).await?;
        crate::executor::decode_rows(batch).map(|mut v: Vec<Out>| {
            if v.is_empty() {
                None
            } else {
                Some(v.remove(0))
            }
        })
    }

    /// Executes the query and returns exactly one row.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Sqlx`] for database errors, including the case where no
    /// row matches.
    pub async fn fetch_one(self) -> Result<Out, Error>
    where
        Out: Send + Unpin + RowDecode,
    {
        self.fetch_optional()
            .await?
            .ok_or_else(|| Error::Message("no row found for query".into()))
    }

    /// Returns the number of rows the query would return.
    ///
    /// `ORDER BY`, `LIMIT` and `OFFSET` are ignored for the count: counting the
    /// matching rows is independent of ordering or pagination.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Sqlx`] for database errors.
    pub async fn count(self) -> Result<i64, Error> {
        let dialect = self.exec.dialect();
        let compiled =
            crate::compile::count::<M>(dialect.as_ref(), M::TABLE, &self.filter.node);

        let batch = self
            .exec
            .fetch_all_raw(compiled.sql, compiled.binds)
            .await?;
        let mut counts = crate::executor::decode_rows::<(i64,)>(batch)?;
        let (count,) = counts
            .pop()
            .ok_or_else(|| Error::Message("COUNT(*) returned no row".into()))?;
        Ok(count)
    }

    /// Whether any row matches.
    ///
    /// Compiles to `SELECT 1 ... LIMIT 1` rather than counting: the database
    /// can stop at the first match, which on a large table is the difference
    /// between an index probe and a full scan.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Sqlx`] for database errors.
    pub async fn exists(self) -> Result<bool, Error> {
        let dialect = self.exec.dialect();
        let compiled =
            crate::compile::exists::<M>(dialect.as_ref(), M::TABLE, &self.filter.node);

        let batch = self
            .exec
            .fetch_all_raw(compiled.sql, compiled.binds)
            .await?;
        Ok(!batch.is_empty())
    }
}

/// A stream of decoded rows, returned by [`SelectQuery::stream`].
pub struct RowStream<'db, Out> {
    inner: crate::executor::BoxRowStream<'db>,
    _out: PhantomData<fn() -> Out>,
}

impl<Out> futures_core::Stream for RowStream<'_, Out>
where
    Out: Unpin + RowDecode,
{
    type Item = Result<Out, Error>;

    fn poll_next(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        // Both fields are `Unpin`, so no `unsafe` projection is needed.
        let this = self.get_mut();
        std::pin::Pin::new(&mut this.inner)
            .poll_next(cx)
            .map(|o| {
                o.map(|r| {
                    r.and_then(|raw| match raw {
                        crate::executor::RawRow::Any(r) => Out::from_row(&r).map_err(Error::Sqlx),
                        crate::executor::RawRow::Postgres(r) => Out::from_row(&r).map_err(Error::Sqlx),
                        crate::executor::RawRow::Sqlite(r) => Out::from_row(&r).map_err(Error::Sqlx),
                        #[cfg(feature = "sqlite-rusqlite")]
                        crate::executor::RawRow::Rusqlite(mut r) => Out::from_rusqlite_row(&mut r),
                    })
                })
            })
    }
}

impl<'db, M, Out> SelectQuery<'db, M, Out, ()>
where
    M: Model,
{
    /// Executes the query and returns all matching rows.
    ///
    /// Only available when the query has no `.include(...)`: fetching all rows
    /// without loading declared includes would silently return the wrong data.
    /// Use [`exec`](SelectQuery::exec) for include-aware execution.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Sqlx`] for database errors.
    pub async fn fetch_all(self) -> Result<Vec<Out>, Error>
    where
        Out: Send + Unpin + RowDecode,
    {
        let compiled = self.to_sql();
        let batch = self
            .exec
            .fetch_all_raw(compiled.sql, compiled.binds)
            .await?;
        crate::executor::decode_rows(batch)
    }

    /// Streams matching rows instead of collecting them.
    ///
    /// Only available when the query has no `.include(...)`: a stream without
    /// loaded includes would silently return the wrong data. Use
    /// [`exec`](SelectQuery::exec) for include-aware execution.
    ///
    /// Rows are decoded one at a time as the stream is polled. The underlying
    /// fetch is currently buffered by both executors (see
    /// [`Executor::stream_raw`]), so this bounds decode cost rather than peak
    /// memory; the buffering lives behind the executor so it can be replaced
    /// with a true cursor without touching this API.
    #[must_use]
    pub fn stream(self) -> RowStream<'db, Out>
    where
        Out: Send + Unpin + RowDecode,
    {
        let compiled = self.to_sql();
        RowStream {
            inner: self.exec.stream_raw(compiled.sql, compiled.binds),
            _out: PhantomData,
        }
    }

    /// Fetches one page, reporting whether another page follows.
    ///
    /// Fetches `size + 1` rows and discards the extra, so `has_next` is exact
    /// rather than inferred from a full page. The model's primary key is
    /// appended to `ORDER BY` so the total order is deterministic — without a
    /// unique tiebreaker, rows sharing an ordering value can repeat across
    /// pages or be skipped entirely.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Sqlx`] for database errors.
    pub async fn page(self, size: u64) -> Result<Page<Out>, Error>
    where
        Out: Send + Unpin + RowDecode,
    {
        let pk: Column<M, i64> = Column::new(M::TABLE, M::PRIMARY_KEY);
        let q = self.order_by(pk.asc()).limit(size.saturating_add(1));

        let mut items: Vec<Out> = q.fetch_all().await?;
        let has_next = items.len() as u64 > size;
        if has_next {
            items.truncate(size as usize);
        }
        Ok(Page::new(items, has_next, None))
    }
}

impl<'db, M, I> SelectQuery<'db, M, M, I>
where
    M: Model,
    I: IncludeSet<M>,
{
    /// Executes the query, then loads any requested includes.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Sqlx`] for database errors.
    pub async fn exec(self) -> Result<Vec<M>, Error> {
        let compiled = self.to_sql();
        let batch = self
            .exec
            .fetch_all_raw(compiled.sql, compiled.binds)
            .await?;
        let mut rows: Vec<M> = crate::executor::decode_rows(batch)?;
        self.includes.load(self.exec, &mut rows).await?;
        Ok(rows)
    }
}

/// A typed `INSERT` query.
#[allow(dead_code)]
pub struct InsertQuery<'db, M: Model> {
    pool: &'db Pool,
    values: Vec<(&'static str, Value)>,
    on_conflict: Option<Vec<&'static str>>,
    do_update: Option<Vec<&'static str>>,
    nested: Option<NestedInsert<'db, M>>,
    _marker: PhantomData<fn() -> M>,
}

impl<'db, M: Model> std::fmt::Debug for InsertQuery<'db, M> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("InsertQuery")
            .field("values", &self.values)
            .field("on_conflict", &self.on_conflict)
            .field("do_update", &self.do_update)
            .field("nested", &self.nested.is_some())
            .finish()
    }
}

impl<'db, M: Model> InsertQuery<'db, M> {
    /// Returns the underlying pool.
    pub fn pool(&self) -> &'db Pool {
        self.pool
    }

    /// Creates a new query.
    #[must_use]
    pub const fn new(pool: &'db Pool) -> Self {
        Self {
            pool,
            values: Vec::new(),
            on_conflict: None,
            do_update: None,
            nested: None,
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

    /// Attaches a one-level nested create to this insert.
    ///
    /// `get_parent_pk` extracts the parent primary key from the inserted row;
    /// `child_fk_col` is the child's foreign-key column; `children` is the set
    /// of child rows to insert; and `setter` attaches the returned child rows
    /// to the parent model.
    pub fn with_related<C: Model>(
        mut self,
        get_parent_pk: fn(&M) -> Value,
        child_fk_col: &'static str,
        children: InsertManyQuery<'db, C>,
        setter: impl NestedSetter<M> + 'static,
    ) -> Self {
        self.nested = Some(NestedInsert {
            get_parent_pk,
            child_table: C::TABLE,
            child_fk_col,
            child_rows: children.rows,
            setter: Box::new(setter),
            _marker: PhantomData,
        });
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
    pub async fn exec(mut self) -> Result<M, Error> {
        let nested = self.nested.take();
        match nested {
            None => self.exec_single().await,
            Some(nested) => self.exec_nested(nested).await,
        }
    }

    async fn exec_single(self) -> Result<M, Error> {
        let dialect = dialect_for_pool(self.pool);
        let returning: &[&str] = if M::COLUMNS.is_empty() {
            &["*"]
        } else {
            M::COLUMNS
        };
        let compiled = if let Some(ref conflict) = self.on_conflict {
            let do_update = self.do_update.as_deref().unwrap_or(&[]);
            upsert::<M>(
                dialect.as_ref(),
                M::TABLE,
                &self.values,
                conflict,
                do_update,
                returning,
            )
        } else {
            insert::<M>(dialect.as_ref(), M::TABLE, &self.values, returning)
        };
        let batch = self
            .pool
            .fetch_all_raw(compiled.sql, compiled.binds)
            .await?;
        let mut rows = crate::executor::decode_rows::<M>(batch)?;
        let row = rows
            .pop()
            .ok_or_else(|| Error::Message("INSERT RETURNING returned no row".into()))?;
        Ok(row)
    }

    async fn exec_nested(self, nested: NestedInsert<'db, M>) -> Result<M, Error> {
        let tx = crate::tx::Tx::begin(self.pool).await?;

        let dialect = dialect_for_pool(self.pool);
        let returning: &[&str] = if M::COLUMNS.is_empty() {
            &["*"]
        } else {
            M::COLUMNS
        };
        let compiled = insert::<M>(dialect.as_ref(), M::TABLE, &self.values, returning);
        let batch = tx.fetch_all_raw(compiled.sql, compiled.binds).await?;
        let mut parent_rows = crate::executor::decode_rows::<M>(batch)?;
        let mut parent = parent_rows
            .pop()
            .ok_or_else(|| Error::Message("INSERT RETURNING returned no row".into()))?;

        let pk_value = (nested.get_parent_pk)(&parent);

        if !nested.child_rows.is_empty() {
            let mut rows: Vec<Vec<(&'static str, Value)>> = nested.child_rows;
            for row in &mut rows {
                row.push((nested.child_fk_col, pk_value.clone()));
            }

            let max = dialect.capabilities().max_query_params;
            let cols_per_row = rows[0].len() as u32;
            let chunk_size = (max / cols_per_row).max(1) as usize;

            let mut child_rows: Option<crate::executor::RowBatch> = None;
            for chunk in rows.chunks(chunk_size) {
                let returning: &[&str] = if M::COLUMNS.is_empty() {
                    &["*"]
                } else {
                    M::COLUMNS
                };
                let compiled =
                    insert_many::<M>(dialect.as_ref(), nested.child_table, chunk, returning);
                let chunk = tx.fetch_all_raw(compiled.sql, compiled.binds).await?;
                match child_rows.as_mut() {
                    Some(acc) => acc.merge(chunk)?,
                    None => child_rows = Some(chunk),
                }
            }

            nested.setter.set(&mut parent, child_rows.unwrap_or(crate::executor::RowBatch::Any(Vec::new())));
        } else {
            nested.setter.set(&mut parent, crate::executor::RowBatch::Any(Vec::new()));
        }

        tx.commit().await?;
        Ok(parent)
    }
}

/// Trait for attaching nested child rows to a freshly inserted parent.
///
/// Generated code provides one implementation per relation.
pub trait NestedSetter<M: Model> {
    /// Attaches the loaded child rows to the parent model.
    fn set(&self, parent: &mut M, batch: crate::executor::RowBatch);
}

/// Specification for a one-level nested create.
struct NestedInsert<'db, M: Model> {
    get_parent_pk: fn(&M) -> Value,
    child_table: &'static str,
    child_fk_col: &'static str,
    child_rows: Vec<Vec<(&'static str, Value)>>,
    setter: Box<dyn NestedSetter<M>>,
    _marker: PhantomData<&'db ()>,
}

impl<'db, M: Model> std::fmt::Debug for NestedInsert<'db, M> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NestedInsert")
            .field("child_table", &self.child_table)
            .field("child_fk_col", &self.child_fk_col)
            .field("child_rows", &self.child_rows)
            .finish()
    }
}

/// A typed multi-row `INSERT` query.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct InsertManyQuery<'db, M: Model> {
    pool: &'db Pool,
    pub(crate) rows: Vec<Vec<(&'static str, Value)>>,
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

    /// Adds multiple rows to insert.
    pub fn rows(
        mut self,
        rows: impl IntoIterator<Item = impl IntoIterator<Item = (&'static str, Value)>>,
    ) -> Self {
        for r in rows {
            self.rows.push(r.into_iter().collect());
        }
        self
    }

    /// Executes the insert, returning all inserted rows.
    ///
    /// The rows are chunked to stay under the database's parameter limit.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Sqlx`] for database errors.
    pub async fn exec(self) -> Result<Vec<M>, Error> {
        if self.rows.is_empty() {
            return Ok(Vec::new());
        }

        let dialect = dialect_for_pool(self.pool);
        let max = dialect.capabilities().max_query_params;
        let cols_per_row = self.rows[0].len() as u32;
        let chunk_size = (max / cols_per_row).max(1) as usize;

        let mut out = Vec::new();
        for chunk in self.rows.chunks(chunk_size) {
            let returning: &[&str] = if M::COLUMNS.is_empty() {
                &["*"]
            } else {
                M::COLUMNS
            };
            let compiled = insert_many::<M>(dialect.as_ref(), M::TABLE, chunk, returning);
            let batch = self.pool.fetch_all_raw(compiled.sql, compiled.binds).await?;
            let mut rows = crate::executor::decode_rows::<M>(batch)?;
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
        self.pool.execute_raw(compiled.sql, compiled.binds).await
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
        self.pool.execute_raw(compiled.sql, compiled.binds).await
    }
}
