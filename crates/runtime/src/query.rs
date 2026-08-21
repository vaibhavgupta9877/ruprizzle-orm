//! Query builders.

use std::borrow::Cow;
use std::marker::PhantomData;
use std::sync::Arc;

use crate::Error;
use crate::aggregate::{AggregateEntry, AggregateSet, GroupBy};
use crate::col::{Column, Projection};
use crate::compile::{
    CompiledSql, SetExpr, delete, dialect_for_pool, insert, insert_many, join_select_with_columns,
    select, update_with_sets, upsert,
};
use crate::executor::Executor;
use crate::filter::{Cte, CteQuery, Filter, FilterNode};
use crate::include::IncludeSet;
use crate::join::{Join2, JoinKind, JoinOn, JoinSpec, LeftJoin2, Maybe};
use crate::json::JsonSet;
use crate::m2m::{AnyM2mWrite, M2mWrite};
use crate::model::{Model, RowDecode};
use crate::order::OrderBy;
use crate::page::Page;
use crate::pool::Pool;
use crate::query_manifest;
use crate::rel::{AnyRelDelete, AnyRelWrite, DeleteAction, DeleteCascade, RelAction, RelWrite};
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
    ctes: Vec<Cte>,
    includes: I,
    join: Option<JoinSpec>,
    with_deleted: bool,
    only_deleted: bool,
    _out: PhantomData<fn() -> Out>,
}

/// A prepared `SELECT` statement.
///
/// The SQL is compiled once; bind values can be swapped and the statement can
/// be re-executed without rebuilding the query string. Bind positions are
/// positional and match the placeholders in the compiled SQL.
#[allow(dead_code)]
pub struct PreparedSelect<'db, M: Model, Out = M> {
    exec: &'db dyn Executor,
    sql: Cow<'static, str>,
    binds: Vec<Value>,
    _marker: PhantomData<fn() -> (M, Out)>,
}

impl<'db, M: Model, Out> Clone for PreparedSelect<'db, M, Out> {
    fn clone(&self) -> Self {
        Self {
            exec: self.exec,
            sql: self.sql.clone(),
            binds: self.binds.clone(),
            _marker: PhantomData,
        }
    }
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
            ctes: Vec::new(),
            includes: (),
            join: None,
            with_deleted: false,
            only_deleted: false,
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

    /// Adds a filter (`AND`) only if `f` is `Some`.
    pub fn filter_if(self, f: Option<Filter<M>>) -> Self {
        match f {
            Some(f) => self.filter(f),
            None => self,
        }
    }

    /// Adds a filter (`OR`) only if `f` is `Some`.
    pub fn or_filter_if(self, f: Option<Filter<M>>) -> Self {
        match f {
            Some(f) => self.or_filter(f),
            None => self,
        }
    }

    /// Filters rows by discriminator column value for Single Table Inheritance (STI) and polymorphic models.
    pub fn filter_type<V: Encodable>(self, col: Column<M, V>, value: impl Into<V>) -> Self {
        self.filter(col.eq(value))
    }

    /// Adds an ordering.
    pub fn order_by(self, o: OrderBy<M>) -> Self {
        let mut order = self.order;
        order.push(o);
        Self { order, ..self }
    }

    /// Adds an ordering only if `o` is `Some`.
    pub fn order_by_if(self, o: Option<OrderBy<M>>) -> Self {
        match o {
            Some(o) => self.order_by(o),
            None => self,
        }
    }

    /// Sets the limit.
    pub fn limit(self, n: u64) -> Self {
        Self {
            limit: Some(n),
            ..self
        }
    }

    /// Sets the limit only if `n` is `Some`.
    pub fn limit_if(self, n: Option<u64>) -> Self {
        match n {
            Some(n) => self.limit(n),
            None => self,
        }
    }

    /// Sets the offset.
    pub fn offset(self, n: u64) -> Self {
        Self {
            offset: Some(n),
            ..self
        }
    }

    /// Sets the offset only if `n` is `Some`.
    pub fn offset_if(self, n: Option<u64>) -> Self {
        match n {
            Some(n) => self.offset(n),
            None => self,
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

    /// Adds a non-recursive common table expression (CTE).
    ///
    /// `query` can be any [`SelectQuery`]; it is compiled and emitted as
    /// `WITH <name> AS (...)`.
    #[must_use]
    pub fn with<Q: Into<CteQuery>>(mut self, name: &'static str, query: Q) -> Self {
        self.ctes.push(Cte::new(name, query.into().compiled));
        self
    }

    /// Adds a recursive common table expression (CTE).
    ///
    /// The body is `(anchor) UNION ALL (recursive)`, with placeholders
    /// renumbered across the union. The CTE is emitted as
    /// `WITH RECURSIVE <name> AS (...)`.
    #[must_use]
    pub fn with_recursive<A: Into<CteQuery>, R: Into<CteQuery>>(
        mut self,
        name: &'static str,
        anchor: A,
        recursive: R,
    ) -> Self {
        let anchor = anchor.into().compiled;
        let recursive = recursive.into().compiled.renumbered(anchor.binds.len());
        let mut binds = anchor.binds.clone();
        binds.extend(recursive.binds.clone());
        let sql = format!(
            "({}) UNION ALL ({})",
            anchor.sql.as_ref(),
            recursive.sql.as_ref()
        );
        self.ctes.push(Cte {
            name,
            compiled: CompiledSql {
                sql: Cow::Owned(sql),
                binds,
            },
            recursive: true,
        });
        self
    }

    /// Adds an inner join to another model.
    pub fn inner_join<J: Model>(
        self,
        on: impl Into<JoinOn>,
    ) -> SelectQuery<'db, M, Join2<M, J>, I> {
        self.join_with::<J, Join2<M, J>>(JoinKind::Inner, None, on)
    }

    /// Adds a left join to another model.
    pub fn left_join<J: Model>(
        self,
        on: impl Into<JoinOn>,
    ) -> SelectQuery<'db, M, LeftJoin2<M, J>, I> {
        self.join_with::<J, LeftJoin2<M, J>>(JoinKind::Left, None, on)
    }

    /// Adds a right join to another model.
    pub fn right_join<J: Model>(
        self,
        on: impl Into<JoinOn>,
    ) -> SelectQuery<'db, M, Join2<Maybe<M>, J>, I> {
        self.join_with::<J, Join2<Maybe<M>, J>>(JoinKind::Right, None, on)
    }

    /// Adds a full join to another model.
    pub fn full_join<J: Model>(
        self,
        on: impl Into<JoinOn>,
    ) -> SelectQuery<'db, M, Join2<Maybe<M>, Maybe<J>>, I> {
        self.join_with::<J, Join2<Maybe<M>, Maybe<J>>>(JoinKind::Full, None, on)
    }

    /// Adds an aliased inner join to another model.
    ///
    /// Use `Column::aliased` in the `ON` condition so the right-hand side is
    /// qualified by the alias. This is the fully-typed way to write self-joins:
    ///
    /// ```ignore
    /// User::query().inner_join_aliased::<User>("u2", User::id.on(User::manager_id.aliased("u2")))
    /// ```
    pub fn inner_join_aliased<J: Model>(
        self,
        alias: &'static str,
        on: impl Into<JoinOn>,
    ) -> SelectQuery<'db, M, Join2<M, J>, I> {
        self.join_with::<J, Join2<M, J>>(JoinKind::Inner, Some(alias), on)
    }

    /// Adds an aliased left join to another model.
    pub fn left_join_aliased<J: Model>(
        self,
        alias: &'static str,
        on: impl Into<JoinOn>,
    ) -> SelectQuery<'db, M, LeftJoin2<M, J>, I> {
        self.join_with::<J, LeftJoin2<M, J>>(JoinKind::Left, Some(alias), on)
    }

    /// Adds an aliased right join to another model.
    pub fn right_join_aliased<J: Model>(
        self,
        alias: &'static str,
        on: impl Into<JoinOn>,
    ) -> SelectQuery<'db, M, Join2<Maybe<M>, J>, I> {
        self.join_with::<J, Join2<Maybe<M>, J>>(JoinKind::Right, Some(alias), on)
    }

    /// Adds an aliased full join to another model.
    pub fn full_join_aliased<J: Model>(
        self,
        alias: &'static str,
        on: impl Into<JoinOn>,
    ) -> SelectQuery<'db, M, Join2<Maybe<M>, Maybe<J>>, I> {
        self.join_with::<J, Join2<Maybe<M>, Maybe<J>>>(JoinKind::Full, Some(alias), on)
    }

    /// Includes soft-deleted rows in the query results (disables default `WHERE deleted_at IS NULL`).
    pub fn with_deleted(mut self) -> Self {
        self.with_deleted = true;
        self.only_deleted = false;
        self
    }

    /// Queries *only* soft-deleted rows (`WHERE deleted_at IS NOT NULL`).
    pub fn only_deleted(mut self) -> Self {
        self.with_deleted = false;
        self.only_deleted = true;
        self
    }

    fn join_with<J: Model, O>(
        self,
        kind: JoinKind,
        right_alias: Option<&'static str>,
        on: impl Into<JoinOn>,
    ) -> SelectQuery<'db, M, O, I> {
        SelectQuery {
            exec: self.exec,
            filter: self.filter,
            projection: Vec::new(),
            order: self.order,
            limit: self.limit,
            offset: self.offset,
            distinct: self.distinct,
            ctes: self.ctes,
            includes: self.includes,
            join: Some(JoinSpec {
                kind,
                right_table: J::TABLE,
                right_columns: J::COLUMNS,
                right_alias,
                on: on.into(),
            }),
            with_deleted: self.with_deleted,
            only_deleted: self.only_deleted,
            _out: PhantomData,
        }
    }

    /// Switches the query to return aggregate results rather than rows.
    ///
    /// The output type is a tuple, e.g. `(Option<i64>, i64)` for
    /// `aggregate((User::age.sum(), User::id.count()))`.
    #[must_use]
    pub fn aggregate<R, A>(self, set: A) -> AggregateQuery<'db, M, R>
    where
        R: RowDecode,
        A: AggregateSet<M, R>,
    {
        let mut entries = Vec::new();
        set.push_entries(&mut entries);
        AggregateQuery {
            exec: self.exec,
            filter: self.effective_filter(),
            aggregates: entries,
            group_by: Vec::new(),
            having: Filter::new(FilterNode::And(Vec::new())),
            order: self.order,
            limit: self.limit,
            offset: self.offset,
            ctes: self.ctes,
            _out: PhantomData,
        }
    }

    /// Groups the query by one or more columns.
    ///
    /// Returns a [`GroupedQuery`] that supports `having` and `aggregate`.
    #[must_use]
    pub fn group_by<G: GroupBy<M>>(self, g: G) -> GroupedQuery<'db, M, Out, I> {
        GroupedQuery {
            inner: self,
            group_by: g.columns(),
            having: Filter::new(FilterNode::And(Vec::new())),
            _marker: PhantomData,
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
            ctes: self.ctes,
            includes: include,
            join: self.join,
            with_deleted: self.with_deleted,
            only_deleted: self.only_deleted,
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
            ctes: self.ctes,
            includes: (),
            join: None,
            with_deleted: self.with_deleted,
            only_deleted: self.only_deleted,
            _out: PhantomData,
        }
    }

    /// Returns `true` when this query is known to load every row of the model's
    /// table with no filter, limit, offset or distinct.
    pub(crate) fn is_full_table(&self) -> bool {
        self.ctes.is_empty()
            && self.limit.is_none()
            && self.offset.is_none()
            && !self.distinct
            && matches!(
                self.filter.node,
                crate::filter::FilterNode::And(ref v) if v.is_empty()
            )
    }

    /// Returns an error if the query's join is not supported by the dialect.
    fn check_join_support(&self) -> Result<(), Error> {
        let dialect = self.exec.dialect();
        if let Some(ref join) = self.join {
            match join.kind {
                JoinKind::Right if !dialect.supports_right_join() => {
                    return Err(Error::Message(format!(
                        "RIGHT JOIN is not supported by {}",
                        dialect.name()
                    )));
                }
                JoinKind::Full if !dialect.supports_full_join() => {
                    return Err(Error::Message(format!(
                        "FULL OUTER JOIN is not supported by {}",
                        dialect.name()
                    )));
                }
                _ => {}
            }
        }
        Ok(())
    }

    /// Returns the effective filter incorporating soft-delete filters if configured on the model.
    pub(crate) fn effective_filter(&self) -> Filter<M> {
        let mut filter = self.filter.clone();
        if let Some(col) = M::DELETED_AT_COLUMN {
            if !self.with_deleted {
                if self.only_deleted {
                    filter = filter.and(Filter::new(FilterNode::Null {
                        table: M::TABLE,
                        column: col,
                        negated: true,
                    }));
                } else {
                    filter = filter.and(Filter::new(FilterNode::Null {
                        table: M::TABLE,
                        column: col,
                        negated: false,
                    }));
                }
            }
        }
        filter
    }

    /// Compiles the main query (without the CTE `WITH` prefix) to SQL and binds.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Message`] if the query uses a `RIGHT JOIN` or
    /// `FULL OUTER JOIN` that the target dialect does not support.
    pub(crate) fn to_sql_without_cte(&self) -> Result<CompiledSql, Error> {
        self.check_join_support()?;
        let dialect = self.exec.dialect();
        let eff_filter = self.effective_filter();
        if let Some(ref join) = self.join {
            Ok(join_select_with_columns::<M>(
                dialect,
                M::TABLE,
                M::COLUMNS,
                join.right_table,
                join.right_columns,
                join.right_alias,
                join.kind,
                &join.on.node,
                &eff_filter.node,
                &self.order,
                self.limit,
                self.offset,
                self.distinct,
            ))
        } else {
            Ok(select::<M>(
                dialect,
                M::TABLE,
                &self.projection,
                &eff_filter.node,
                &self.order,
                self.limit,
                self.offset,
                self.distinct,
            ))
        }
    }

    /// Compiles the query to SQL and binds.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Message`] if the query uses a `RIGHT JOIN` or
    /// `FULL OUTER JOIN` that the target dialect does not support.
    pub fn to_sql(&self) -> Result<CompiledSql, Error> {
        let dialect = self.exec.dialect();
        let main = self.to_sql_without_cte()?;
        let compiled = crate::compile::with_cte_prefix(dialect, &self.ctes, main);
        query_manifest::record(
            compiled.sql.clone().into_owned(),
            Some(file!()),
            Some(line!()),
            dialect.name(),
        );
        Ok(compiled)
    }

    /// Combines this query with another using `UNION`.
    ///
    /// The output shape of both sides must match exactly; this is enforced at
    /// compile time by the shared `Out` type parameter.
    pub fn union<M2, I2>(self, other: SelectQuery<'db, M2, Out, I2>) -> SetOpQuery<'db, Out>
    where
        M2: Model,
    {
        SetOpQuery::new(self.exec, SetOp::Union, self, other)
    }

    /// Combines this query with another using `UNION ALL`.
    pub fn union_all<M2, I2>(self, other: SelectQuery<'db, M2, Out, I2>) -> SetOpQuery<'db, Out>
    where
        M2: Model,
    {
        SetOpQuery::new(self.exec, SetOp::UnionAll, self, other)
    }

    /// Combines this query with another using `INTERSECT`.
    pub fn intersect<M2, I2>(self, other: SelectQuery<'db, M2, Out, I2>) -> SetOpQuery<'db, Out>
    where
        M2: Model,
    {
        SetOpQuery::new(self.exec, SetOp::Intersect, self, other)
    }

    /// Combines this query with another using `EXCEPT`.
    pub fn except<M2, I2>(self, other: SelectQuery<'db, M2, Out, I2>) -> SetOpQuery<'db, Out>
    where
        M2: Model,
    {
        SetOpQuery::new(self.exec, SetOp::Except, self, other)
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
        self.check_join_support()?;
        let dialect = self.exec.dialect();
        let eff_filter = self.effective_filter();
        let compiled = crate::compile::count::<M>(dialect, M::TABLE, &eff_filter.node);
        let compiled = crate::compile::with_cte_prefix(dialect, &self.ctes, compiled);

        #[cfg(feature = "sqlite-rusqlite")]
        let mut counts: Vec<(i64,)> = if let Some(pool) = self.exec.as_rusqlite() {
            self.exec.on_query();
            pool.fetch_all_sync_decoded::<(i64,)>(compiled.sql, compiled.binds)
                .await?
        } else {
            let batch = self
                .exec
                .fetch_all_raw(compiled.sql, compiled.binds)
                .await?;
            crate::executor::decode_rows(batch)?
        };

        #[cfg(not(feature = "sqlite-rusqlite"))]
        let mut counts: Vec<(i64,)> = {
            let batch = self
                .exec
                .fetch_all_raw(compiled.sql, compiled.binds)
                .await?;
            crate::executor::decode_rows(batch)?
        };

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
        self.check_join_support()?;
        let dialect = self.exec.dialect();
        let eff_filter = self.effective_filter();
        let compiled = crate::compile::exists::<M>(dialect, M::TABLE, &eff_filter.node);
        let compiled = crate::compile::with_cte_prefix(dialect, &self.ctes, compiled);

        #[cfg(feature = "sqlite-rusqlite")]
        {
            if let Some(pool) = self.exec.as_rusqlite() {
                self.exec.on_query();
                let rows = pool
                    .fetch_all_sync_decoded::<(i64,)>(compiled.sql, compiled.binds)
                    .await?;
                return Ok(!rows.is_empty());
            }
        }

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
        std::pin::Pin::new(&mut this.inner).poll_next(cx).map(|o| {
            o.map(|r| {
                r.and_then(|raw| match raw {
                    crate::executor::RawRow::Any(r) => Out::from_row(&r).map_err(Error::Sqlx),
                    crate::executor::RawRow::Postgres(r) => Out::from_row(&r).map_err(Error::Sqlx),
                    crate::executor::RawRow::Sqlite(r) => Out::from_row(&r).map_err(Error::Sqlx),
                    crate::executor::RawRow::Mysql(r) => Out::from_row(&r).map_err(Error::Sqlx),
                    #[cfg(feature = "sqlite-rusqlite")]
                    crate::executor::RawRow::Rusqlite(r) => Out::from_owned_row(&r),
                    #[cfg(feature = "postgres-tokio-postgres")]
                    crate::executor::RawRow::PostgresNative(r) => Out::from_tokio_postgres_row(&r),
                })
            })
        })
    }
}

impl<'db, M, Out> SelectQuery<'db, M, Out, ()>
where
    M: Model,
{
    /// Prepares a reusable compiled statement.
    ///
    /// The SQL is compiled once; bind values can be swapped with
    /// [`PreparedSelect::bind`] and the statement can be executed repeatedly
    /// without rebuilding the query string.
    pub fn prepare(self) -> Result<PreparedSelect<'db, M, Out>, Error> {
        let compiled = self.to_sql()?;
        Ok(PreparedSelect {
            exec: self.exec,
            sql: compiled.sql,
            binds: compiled.binds,
            _marker: PhantomData,
        })
    }

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
        let compiled = self.to_sql()?;

        #[cfg(feature = "sqlite-rusqlite")]
        if let Some(pool) = self.exec.as_rusqlite() {
            self.exec.on_query();
            return pool
                .fetch_all_sync_decoded::<Out>(compiled.sql, compiled.binds)
                .await;
        }

        let batch = self
            .exec
            .fetch_all_raw(compiled.sql, compiled.binds)
            .await?;
        crate::executor::decode_rows(batch)
    }

    /// Alias for [`fetch_all`](SelectQuery::fetch_all).
    pub async fn all(self) -> Result<Vec<Out>, Error>
    where
        Out: Send + Unpin + RowDecode,
    {
        self.fetch_all().await
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
    pub fn stream(self) -> Result<RowStream<'db, Out>, Error>
    where
        Out: Send + Unpin + RowDecode,
    {
        let compiled = self.to_sql()?;
        Ok(RowStream {
            inner: self.exec.stream_raw(compiled.sql, compiled.binds),
            _out: PhantomData,
        })
    }

    /// Streams matching rows without buffering the whole result set.
    ///
    /// Only available when the query has no `.include(...)`: a stream without
    /// loaded includes would silently return the wrong data. Use
    /// [`exec`](SelectQuery::exec) for include-aware execution.
    ///
    /// For `sqlx` backends the rows are produced incrementally by the driver,
    /// which keeps peak memory proportional to the number of in-flight rows
    /// rather than the total result size. The `postgres-tokio-postgres` backend
    /// uses an unbuffered server-side portal.
    pub fn stream_unbuffered(self) -> Result<RowStream<'db, Out>, Error>
    where
        Out: Send + Unpin + RowDecode,
    {
        let compiled = self.to_sql()?;
        Ok(RowStream {
            inner: self
                .exec
                .stream_unbuffered_raw(compiled.sql, compiled.binds),
            _out: PhantomData,
        })
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

    /// Executes the query and returns the first row, if any.
    ///
    /// Only available when the query has no `.include(...)`: fetching a single
    /// row without loading declared includes would silently return the wrong
    /// data. Use [`exec_optional`](SelectQuery::exec_optional) for include-aware
    /// execution.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Sqlx`] for database errors.
    pub async fn fetch_optional(mut self) -> Result<Option<Out>, Error>
    where
        Out: Send + Unpin + RowDecode,
    {
        // `fetch_optional` only needs one row. Forcing `LIMIT 1` prevents the
        // database from materialising and ruprizzle from decoding a larger
        // result set when the caller set a higher limit.
        self.limit = Some(1);
        let compiled = self.to_sql()?;

        #[cfg(feature = "sqlite-rusqlite")]
        let v: Vec<Out> = if let Some(pool) = self.exec.as_rusqlite() {
            self.exec.on_query();
            pool.fetch_all_sync_decoded::<Out>(compiled.sql, compiled.binds)
                .await?
        } else {
            let batch = self
                .exec
                .fetch_all_raw(compiled.sql, compiled.binds)
                .await?;
            crate::executor::decode_rows(batch)?
        };

        #[cfg(not(feature = "sqlite-rusqlite"))]
        let v: Vec<Out> = {
            let batch = self
                .exec
                .fetch_all_raw(compiled.sql, compiled.binds)
                .await?;
            crate::executor::decode_rows(batch)?
        };

        Ok(v.into_iter().next())
    }

    /// Executes the query and returns exactly one row.
    ///
    /// Only available when the query has no `.include(...)`. Use
    /// [`exec_one`](SelectQuery::exec_one) when includes are requested.
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
}

/// A SQL set operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SetOp {
    /// `UNION` (deduplicating).
    Union,
    /// `UNION ALL` (preserves duplicates).
    UnionAll,
    /// `INTERSECT`.
    Intersect,
    /// `EXCEPT`.
    Except,
}

impl SetOp {
    /// Returns the SQL keyword for this operation.
    pub(crate) fn sql(self) -> &'static str {
        match self {
            SetOp::Union => "UNION",
            SetOp::UnionAll => "UNION ALL",
            SetOp::Intersect => "INTERSECT",
            SetOp::Except => "EXCEPT",
        }
    }
}

/// A query built from two `SELECT`s combined by a set operation.
#[derive(Clone)]
#[allow(dead_code)]
pub struct SetOpQuery<'db, Out> {
    exec: &'db dyn Executor,
    op: SetOp,
    left: CompiledSql,
    right: CompiledSql,
    ctes: Vec<Cte>,
    limit: Option<u64>,
    offset: Option<u64>,
    compile_error: Option<String>,
    _out: PhantomData<fn() -> Out>,
}

impl<'db, Out> SetOpQuery<'db, Out> {
    pub(crate) fn new<M, M2, I, I2>(
        exec: &'db dyn Executor,
        op: SetOp,
        left: SelectQuery<'db, M, Out, I>,
        right: SelectQuery<'db, M2, Out, I2>,
    ) -> Self
    where
        M: Model,
        M2: Model,
    {
        let mut compile_error = None;
        let left_sql = match left.to_sql_without_cte() {
            Ok(c) => c,
            Err(e) => {
                compile_error = Some(e.to_string());
                CompiledSql {
                    sql: Cow::Borrowed(""),
                    binds: Vec::new(),
                }
            }
        };
        let right_sql = match right.to_sql_without_cte() {
            Ok(c) => c,
            Err(e) => {
                if compile_error.is_none() {
                    compile_error = Some(e.to_string());
                }
                CompiledSql {
                    sql: Cow::Borrowed(""),
                    binds: Vec::new(),
                }
            }
        };
        let mut ctes = left.ctes;
        ctes.extend(right.ctes);
        Self {
            exec,
            op,
            left: left_sql,
            right: right_sql,
            ctes,
            limit: None,
            offset: None,
            compile_error,
            _out: PhantomData,
        }
    }

    /// Sets the limit on the combined result.
    pub fn limit(mut self, n: u64) -> Self {
        self.limit = Some(n);
        self
    }

    /// Sets the limit on the combined result only if `n` is `Some`.
    pub fn limit_if(mut self, n: Option<u64>) -> Self {
        if let Some(n) = n {
            self.limit = Some(n);
        }
        self
    }

    /// Sets the offset on the combined result.
    pub fn offset(mut self, n: u64) -> Self {
        self.offset = Some(n);
        self
    }

    /// Sets the offset on the combined result only if `n` is `Some`.
    pub fn offset_if(mut self, n: Option<u64>) -> Self {
        if let Some(n) = n {
            self.offset = Some(n);
        }
        self
    }

    /// Compiles the query to SQL and binds.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Message`] if either side could not be compiled, or if
    /// the target dialect does not support the set operation (`INTERSECT` or
    /// `EXCEPT` on MySQL).
    pub fn to_sql(&self) -> Result<CompiledSql, Error> {
        if let Some(ref e) = self.compile_error {
            return Err(Error::Message(e.clone()));
        }
        let dialect = self.exec.dialect();
        match self.op {
            SetOp::Intersect if !dialect.supports_intersect() => {
                return Err(Error::Message(format!(
                    "INTERSECT is not supported by {}",
                    dialect.name()
                )));
            }
            SetOp::Except if !dialect.supports_except() => {
                return Err(Error::Message(format!(
                    "EXCEPT is not supported by {}",
                    dialect.name()
                )));
            }
            _ => {}
        }
        let mut compiled = crate::compile::set_op(
            dialect,
            self.op,
            &self.ctes,
            self.left.clone(),
            self.right.clone(),
        );
        if let Some(n) = self.limit {
            compiled.sql = Cow::Owned(format!("{} LIMIT {}", compiled.sql, n));
        }
        if let Some(n) = self.offset {
            compiled.sql = Cow::Owned(format!("{} OFFSET {}", compiled.sql, n));
        }
        query_manifest::record(
            compiled.sql.clone().into_owned(),
            Some(file!()),
            Some(line!()),
            dialect.name(),
        );
        Ok(compiled)
    }
}

impl<'db, Out> SetOpQuery<'db, Out>
where
    Out: RowDecode,
{
    /// Executes the query and returns all matching rows.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Sqlx`] for database errors.
    pub async fn fetch_all(self) -> Result<Vec<Out>, Error>
    where
        Out: Send + Unpin,
    {
        let compiled = self.to_sql()?;

        #[cfg(feature = "sqlite-rusqlite")]
        if let Some(pool) = self.exec.as_rusqlite() {
            self.exec.on_query();
            return pool
                .fetch_all_sync_decoded::<Out>(compiled.sql, compiled.binds)
                .await;
        }

        let batch = self
            .exec
            .fetch_all_raw(compiled.sql, compiled.binds)
            .await?;
        crate::executor::decode_rows(batch)
    }

    /// Streams matching rows instead of collecting them.
    pub fn stream(self) -> Result<RowStream<'db, Out>, Error>
    where
        Out: Send + Unpin,
    {
        let compiled = self.to_sql()?;
        Ok(RowStream {
            inner: self.exec.stream_raw(compiled.sql, compiled.binds),
            _out: PhantomData,
        })
    }

    /// Streams matching rows without buffering the whole result set.
    pub fn stream_unbuffered(self) -> Result<RowStream<'db, Out>, Error>
    where
        Out: Send + Unpin,
    {
        let compiled = self.to_sql()?;
        Ok(RowStream {
            inner: self
                .exec
                .stream_unbuffered_raw(compiled.sql, compiled.binds),
            _out: PhantomData,
        })
    }

    /// Executes the query and returns the first row, if any.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Sqlx`] for database errors.
    pub async fn fetch_optional(self) -> Result<Option<Out>, Error>
    where
        Out: Send + Unpin,
    {
        self.limit(1)
            .fetch_all()
            .await
            .map(|rows| rows.into_iter().next())
    }

    /// Executes the query and returns exactly one row.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Sqlx`] for database errors, including the case where no
    /// row matches.
    pub async fn fetch_one(self) -> Result<Out, Error>
    where
        Out: Send + Unpin,
    {
        self.fetch_optional()
            .await?
            .ok_or_else(|| Error::Message("no row found for query".into()))
    }
}

impl<'db, M, Out> PreparedSelect<'db, M, Out>
where
    M: Model,
{
    /// Returns the compiled SQL.
    pub fn sql(&self) -> &str {
        &self.sql
    }

    /// Returns the current bind values.
    pub fn binds(&self) -> &[Value] {
        &self.binds
    }

    /// Sets the bind value at the given position.
    pub fn bind(&mut self, index: usize, value: impl Encodable) -> &mut Self {
        if index < self.binds.len() {
            self.binds[index] = value.to_value();
        }
        self
    }

    /// Replaces all bind values.
    ///
    /// The length must match the number of placeholders in the compiled SQL;
    /// extra values are ignored and missing values leave the existing binds.
    pub fn bind_many(&mut self, binds: Vec<Value>) -> &mut Self {
        let n = self.binds.len().min(binds.len());
        self.binds[..n].clone_from_slice(&binds[..n]);
        if binds.len() > self.binds.len() {
            self.binds.extend_from_slice(&binds[self.binds.len()..]);
        }
        self
    }

    /// Executes the prepared statement and returns all matching rows.
    pub async fn fetch_all(&self) -> Result<Vec<Out>, Error>
    where
        Out: Send + Unpin + RowDecode,
    {
        #[cfg(feature = "sqlite-rusqlite")]
        if let Some(pool) = self.exec.as_rusqlite() {
            self.exec.on_query();
            return pool
                .fetch_all_sync_decoded::<Out>(self.sql.clone(), self.binds.clone())
                .await;
        }

        let batch = self
            .exec
            .fetch_all_raw(self.sql.clone(), self.binds.clone())
            .await?;
        crate::executor::decode_rows(batch)
    }

    /// Executes the prepared statement and streams matching rows.
    pub fn stream(&self) -> RowStream<'db, Out>
    where
        Out: Send + Unpin + RowDecode,
    {
        RowStream {
            inner: self.exec.stream_raw(self.sql.clone(), self.binds.clone()),
            _out: PhantomData,
        }
    }

    /// Executes the prepared statement and streams rows without buffering.
    pub fn stream_unbuffered(&self) -> RowStream<'db, Out>
    where
        Out: Send + Unpin + RowDecode,
    {
        RowStream {
            inner: self
                .exec
                .stream_unbuffered_raw(self.sql.clone(), self.binds.clone()),
            _out: PhantomData,
        }
    }

    /// Executes the prepared statement and returns the first row, if any.
    pub async fn fetch_optional(&self) -> Result<Option<Out>, Error>
    where
        Out: Send + Unpin + RowDecode,
    {
        let rows = self.fetch_all().await?;
        Ok(rows.into_iter().next())
    }

    /// Executes the prepared statement and returns exactly one row.
    pub async fn fetch_one(&self) -> Result<Out, Error>
    where
        Out: Send + Unpin + RowDecode,
    {
        self.fetch_optional()
            .await?
            .ok_or_else(|| Error::Message("no row found for query".into()))
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
        let compiled = self.to_sql()?;

        #[cfg(feature = "sqlite-rusqlite")]
        let mut rows: Vec<M> = if let Some(pool) = self.exec.as_rusqlite() {
            self.exec.on_query();
            pool.fetch_all_sync_decoded::<M>(compiled.sql, compiled.binds)
                .await?
        } else {
            let batch = self
                .exec
                .fetch_all_raw(compiled.sql, compiled.binds)
                .await?;
            crate::executor::decode_rows(batch)?
        };

        #[cfg(not(feature = "sqlite-rusqlite"))]
        let mut rows: Vec<M> = {
            let batch = self
                .exec
                .fetch_all_raw(compiled.sql, compiled.binds)
                .await?;
            crate::executor::decode_rows(batch)?
        };

        self.includes
            .load(self.exec, &mut rows, self.is_full_table())
            .await?;
        Ok(rows)
    }

    /// Executes the query, loads any requested includes, and returns the first
    /// row if one matches.
    ///
    /// A single-row fetch is never a full-table scan, so the full-table include
    /// fast path is disabled for this call.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Sqlx`] for database errors.
    pub async fn exec_optional(mut self) -> Result<Option<M>, Error> {
        self.limit = Some(1);
        let compiled = self.to_sql()?;

        #[cfg(feature = "sqlite-rusqlite")]
        let mut rows: Vec<M> = if let Some(pool) = self.exec.as_rusqlite() {
            self.exec.on_query();
            pool.fetch_all_sync_decoded::<M>(compiled.sql, compiled.binds)
                .await?
        } else {
            let batch = self
                .exec
                .fetch_all_raw(compiled.sql, compiled.binds)
                .await?;
            crate::executor::decode_rows(batch)?
        };

        #[cfg(not(feature = "sqlite-rusqlite"))]
        let mut rows: Vec<M> = {
            let batch = self
                .exec
                .fetch_all_raw(compiled.sql, compiled.binds)
                .await?;
            crate::executor::decode_rows(batch)?
        };

        self.includes.load(self.exec, &mut rows, false).await?;
        Ok(rows.into_iter().next())
    }

    /// Executes the query, loads any requested includes, and returns exactly one
    /// row.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Sqlx`] for database errors, including the case where no
    /// row matches.
    pub async fn exec_one(self) -> Result<M, Error> {
        self.exec_optional()
            .await?
            .ok_or_else(|| Error::Message("no row found for query".into()))
    }
}

async fn fetch_inserted_row<M: Model>(
    exec: &dyn Executor,
    dialect: &dyn ruprizzle_dialect::DbDialect,
    values: &[(&'static str, Value)],
) -> Result<M, Error> {
    let projection = if M::COLUMNS.is_empty() {
        "*".to_owned()
    } else {
        M::COLUMNS
            .iter()
            .map(|col| dialect.quote_ident(col))
            .collect::<Vec<_>>()
            .join(", ")
    };
    let table = dialect.quote_ident(M::TABLE);
    let key = dialect.quote_ident(M::PRIMARY_KEY);
    let (predicate, binds) =
        if let Some((_, value)) = values.iter().find(|(column, _)| *column == M::PRIMARY_KEY) {
            (
                format!("{key} = {}", dialect.placeholder(0)),
                vec![value.clone()],
            )
        } else if dialect.name() == "mysql" {
            (format!("{key} = LAST_INSERT_ID()"), Vec::new())
        } else {
            return Err(Error::Message(
                "inserted row cannot be fetched: primary key was not supplied".into(),
            ));
        };

    let sql = format!("SELECT {projection} FROM {table} WHERE {predicate}");
    let batch = exec.fetch_all_raw(sql.into(), binds).await?;
    crate::executor::decode_rows(batch)?
        .into_iter()
        .next()
        .ok_or_else(|| Error::Message("inserted row was not found after INSERT".into()))
}

/// A typed `INSERT` query.
#[allow(dead_code)]
pub struct InsertQuery<'db, M: Model> {
    pool: &'db Pool,
    values: Vec<(&'static str, Value)>,
    on_conflict: Option<Vec<&'static str>>,
    do_update: Option<Vec<&'static str>>,
    nested: Option<NestedInsert<'db, M>>,
    m2m: Option<Box<dyn AnyM2mWrite<M> + 'db>>,
    nested_writes: Vec<Arc<dyn crate::nested::AnyNestedWrite<M>>>,
    _marker: PhantomData<fn() -> M>,
}

impl<'db, M: Model> std::fmt::Debug for InsertQuery<'db, M> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("InsertQuery")
            .field("values", &self.values)
            .field("on_conflict", &self.on_conflict)
            .field("do_update", &self.do_update)
            .field("nested", &self.nested.is_some())
            .field("m2m", &self.m2m.is_some())
            .field("nested_writes", &self.nested_writes.len())
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
            m2m: None,
            nested_writes: Vec::new(),
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

    /// Alias for [`set_optional`](InsertQuery::set_optional).
    pub fn set_if<V: Encodable>(self, col: Column<M, V>, value: Option<impl Into<V>>) -> Self {
        self.set_optional(col, value)
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

    /// Attaches a many-to-many nested write to this insert.
    ///
    /// The write runs in the same transaction as the parent insert. The parent
    /// is returned with the relation loaded.
    pub fn with_m2m<C: Model + Unpin, J: Model>(mut self, m2m: M2mWrite<'db, M, C, J>) -> Self {
        self.m2m = Some(Box::new(m2m) as Box<dyn AnyM2mWrite<M> + 'db>);
        self
    }

    /// Attaches an atomic nested relation write (e.g. nested create, connect, set) to this insert.
    pub fn with_nested_write<W: crate::nested::AnyNestedWrite<M> + 'static>(
        mut self,
        write: W,
    ) -> Self {
        self.nested_writes.push(Arc::new(write));
        self
    }

    /// Alias for [`exec`](InsertQuery::exec) to provide an ergonomic builder `.save()` endpoint.
    pub async fn save(self) -> Result<M, Error> {
        self.exec().await
    }

    /// Compiles the query to SQL and binds.
    pub fn to_sql(&self) -> CompiledSql {
        let dialect = dialect_for_pool(self.pool);
        let compiled = if let Some(ref conflict) = self.on_conflict {
            let do_update = self.do_update.as_deref().unwrap_or(&[]);
            upsert::<M>(dialect, M::TABLE, &self.values, conflict, do_update, &[])
        } else {
            insert::<M>(dialect, M::TABLE, &self.values, &[])
        };
        query_manifest::record(
            compiled.sql.clone().into_owned(),
            Some(file!()),
            Some(line!()),
            dialect.name(),
        );
        compiled
    }

    /// Executes the insert and returns the inserted row.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Sqlx`] for database errors.
    pub async fn exec(mut self) -> Result<M, Error> {
        let nested = self.nested.take();
        let m2m = self.m2m.take();
        let nested_writes = std::mem::take(&mut self.nested_writes);

        if !nested_writes.is_empty() {
            let tx = crate::tx::Tx::begin(self.pool).await?;
            let mut parent = self.insert_parent(&tx).await?;
            for nw in &nested_writes {
                let pk = nw.parent_pk(&parent);
                nw.execute(&tx, pk).await?;
            }
            if let Some(nested) = nested {
                let pk_value = (nested.get_parent_pk)(&parent);
                Self::insert_nested_child_batch(self.pool, &tx, &mut parent, nested, pk_value)
                    .await?;
            }
            if let Some(m2m) = m2m {
                m2m.execute_insert(&tx, &mut parent).await?;
            }
            tx.commit().await?;
            return Ok(parent);
        }

        match (nested, m2m) {
            (None, None) => self.exec_single().await,
            (Some(nested), None) => self.exec_nested(nested, None).await,
            (None, Some(m2m)) => self.exec_m2m(m2m).await,
            (Some(nested), Some(m2m)) => self.exec_nested(nested, Some(m2m)).await,
        }
    }

    async fn insert_parent(&self, exec: &dyn Executor) -> Result<M, Error> {
        let dialect = exec.dialect();
        let returning: &[&str] = if M::COLUMNS.is_empty() {
            &["*"]
        } else {
            M::COLUMNS
        };
        let compiled = if let Some(ref conflict) = self.on_conflict {
            let do_update = self.do_update.as_deref().unwrap_or(&[]);
            upsert::<M>(
                dialect,
                M::TABLE,
                &self.values,
                conflict,
                do_update,
                returning,
            )
        } else {
            insert::<M>(dialect, M::TABLE, &self.values, returning)
        };

        if dialect.returning_supported() {
            let batch = exec.fetch_all_raw(compiled.sql, compiled.binds).await?;
            let mut rows = crate::executor::decode_rows::<M>(batch)?;
            rows.pop()
                .ok_or_else(|| Error::Message("INSERT RETURNING returned no row".into()))
        } else {
            exec.execute_raw(compiled.sql, compiled.binds).await?;
            fetch_inserted_row(exec, dialect, &self.values).await
        }
    }

    async fn exec_single(self) -> Result<M, Error> {
        self.insert_parent(self.pool).await
    }

    async fn exec_m2m(self, m2m: Box<dyn AnyM2mWrite<M> + 'db>) -> Result<M, Error> {
        let tx = crate::tx::Tx::begin(self.pool).await?;
        let mut parent = self.insert_parent(&tx).await?;
        m2m.execute_insert(&tx, &mut parent).await?;
        tx.commit().await?;
        Ok(parent)
    }

    async fn exec_nested(
        self,
        nested: NestedInsert<'db, M>,
        m2m: Option<Box<dyn AnyM2mWrite<M> + 'db>>,
    ) -> Result<M, Error> {
        let tx = crate::tx::Tx::begin(self.pool).await?;
        let mut parent = self.insert_parent(&tx).await?;
        let pk_value = (nested.get_parent_pk)(&parent);
        Self::insert_nested_child_batch(self.pool, &tx, &mut parent, nested, pk_value).await?;

        if let Some(m2m) = m2m {
            m2m.execute_insert(&tx, &mut parent).await?;
        }

        tx.commit().await?;
        Ok(parent)
    }

    async fn insert_nested_child_batch(
        pool: &'db Pool,
        tx: &crate::tx::Tx,
        parent: &mut M,
        nested: NestedInsert<'db, M>,
        pk_value: Value,
    ) -> Result<(), Error> {
        let dialect = dialect_for_pool(pool);
        if !nested.child_rows.is_empty() {
            let rows: Vec<Vec<(&'static str, Value)>> = nested.child_rows;
            if rows.iter().any(|r| r.is_empty()) {
                return Err(Error::Message("insert row has no columns".into()));
            }
            validate_row_shape(&rows)?;

            let mut rows = rows;
            for row in &mut rows {
                row.push((nested.child_fk_col, pk_value.clone()));
            }

            let max = dialect.capabilities().max_query_params;
            let cols_per_row = rows.first().map(|r| r.len()).unwrap_or(0) as u32;
            let chunk_size = (max / cols_per_row.max(1)).max(1) as usize;

            let mut child_rows: Option<crate::executor::RowBatch> = None;
            for chunk in rows.chunks(chunk_size) {
                let returning: &[&str] = if M::COLUMNS.is_empty() {
                    &["*"]
                } else {
                    M::COLUMNS
                };
                let compiled = insert_many::<M>(dialect, nested.child_table, chunk, returning);
                if dialect.returning_supported() {
                    let chunk = tx.fetch_all_raw(compiled.sql, compiled.binds).await?;
                    match child_rows.as_mut() {
                        Some(acc) => acc.merge(chunk)?,
                        None => child_rows = Some(chunk),
                    }
                } else {
                    tx.execute_raw(compiled.sql, compiled.binds).await?;
                }
            }

            if !dialect.returning_supported() {
                let sql = format!(
                    "SELECT * FROM {} WHERE {} = {}",
                    dialect.quote_ident(nested.child_table),
                    dialect.quote_ident(nested.child_fk_col),
                    dialect.placeholder(0)
                );
                child_rows = Some(tx.fetch_all_raw(sql.into(), vec![pk_value.clone()]).await?);
            }

            nested.setter.set(
                parent,
                child_rows.unwrap_or(crate::executor::RowBatch::Any(Vec::new())),
            );
        } else {
            nested
                .setter
                .set(parent, crate::executor::RowBatch::Any(Vec::new()));
        }
        Ok(())
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

        if self.rows.iter().any(|r| r.is_empty()) {
            return Err(Error::Message("insert row has no columns".into()));
        }
        validate_row_shape(&self.rows)?;

        let dialect = dialect_for_pool(self.pool);
        let max = dialect.capabilities().max_query_params;
        let cols_per_row = self.rows.first().map(|r| r.len()).unwrap_or(0) as u32;
        let chunk_size = (max / cols_per_row.max(1)).max(1) as usize;

        let mut out = Vec::new();
        for chunk in self.rows.chunks(chunk_size) {
            let returning: &[&str] = if M::COLUMNS.is_empty() {
                &["*"]
            } else {
                M::COLUMNS
            };
            let compiled = insert_many::<M>(dialect, M::TABLE, chunk, returning);
            let batch = self
                .pool
                .fetch_all_raw(compiled.sql, compiled.binds)
                .await?;
            let mut rows = crate::executor::decode_rows::<M>(batch)?;
            out.append(&mut rows);
        }
        Ok(out)
    }
}

fn validate_row_shape(rows: &[Vec<(&'static str, Value)>]) -> Result<(), Error> {
    let Some(first) = rows.first() else {
        return Ok(());
    };
    if rows.len() < 2 {
        return Ok(());
    }
    for (idx, row) in rows.iter().enumerate().skip(1) {
        if row.len() != first.len() {
            return Err(Error::Message(format!(
                "insert row {idx} has {} columns, expected {} from row 0",
                row.len(),
                first.len()
            )));
        }
        for (col_idx, (a, b)) in first.iter().zip(row.iter()).enumerate() {
            if a.0 != b.0 {
                return Err(Error::Message(format!(
                    "insert row {idx} column {col_idx} differs from row 0: expected '{}', found '{}'",
                    a.0, b.0
                )));
            }
        }
    }
    Ok(())
}

/// A typed `UPDATE` query.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct UpdateQuery<'db, M: Model> {
    pool: &'db Pool,
    sets: Vec<SetExpr>,
    filter: Filter<M>,
    all_rows: bool,
    m2m: Option<Arc<dyn AnyM2mWrite<M> + 'db>>,
    rel: Vec<Arc<dyn AnyRelWrite<M>>>,
    nested_writes: Vec<Arc<dyn crate::nested::AnyNestedWrite<M>>>,
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
            m2m: None,
            rel: Vec::new(),
            nested_writes: Vec::new(),
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

    /// Adds a filter only if `f` is `Some`.
    ///
    /// If `f` is `None` the query is marked as `.all_rows()` so it can still be
    /// executed. This is the conditional-building equivalent of "filter, or
    /// update every row".
    pub fn filter_if(mut self, f: Option<Filter<M>>) -> Self {
        match f {
            Some(f) => self.filter(f),
            None => {
                self.all_rows = true;
                self
            }
        }
    }

    /// Sets an explicit value.
    pub fn set<V: Encodable>(mut self, col: Column<M, V>, value: impl Into<V>) -> Self {
        self.sets.push(SetExpr::Column {
            column: col.column,
            value: value.into().to_value(),
        });
        self
    }

    /// Sets a column to `NULL`.
    pub fn set_null<V: Encodable>(mut self, col: Column<M, V>) -> Self {
        self.sets.push(SetExpr::Column {
            column: col.column,
            value: Value::Null,
        });
        self
    }

    /// Applies a JSON set expression.
    pub fn json_set(mut self, set: JsonSet) -> Self {
        self.sets.push(SetExpr::JsonSet {
            column: set.column,
            path: set.path,
            value: set.value,
        });
        self
    }

    /// Convenience: set a single key inside a JSON column.
    pub fn jsonb_set(
        self,
        col: Column<M, serde_json::Value>,
        key: &'static str,
        value: serde_json::Value,
    ) -> Self {
        self.json_set(col.json_set(key, value))
    }

    /// Sets a column only if `value` is `Some`.
    pub fn set_if<V: Encodable>(self, col: Column<M, V>, value: Option<impl Into<V>>) -> Self {
        match value {
            Some(value) => self.set(col, value),
            None => self,
        }
    }

    /// Applies a JSON set expression only if `set` is `Some`.
    pub fn json_set_if(self, set: Option<JsonSet>) -> Self {
        match set {
            Some(set) => self.json_set(set),
            None => self,
        }
    }

    /// Convenience: set a single key inside a JSON column only if `value` is `Some`.
    pub fn jsonb_set_if(
        self,
        col: Column<M, serde_json::Value>,
        key: &'static str,
        value: Option<serde_json::Value>,
    ) -> Self {
        match value {
            Some(value) => self.jsonb_set(col, key, value),
            None => self,
        }
    }

    /// Allows updating all rows. Without this, `exec` returns an error if no
    /// filter was supplied.
    pub fn all_rows(mut self) -> Self {
        self.all_rows = true;
        self
    }

    /// Sets the model's soft-delete column (`@deletedAt`) to the current timestamp (`now()`).
    ///
    /// # Errors
    /// Returns an error if the model does not have a `@deletedAt` column configured.
    pub fn soft_delete(self) -> Result<Self, Error> {
        let col = M::DELETED_AT_COLUMN.ok_or_else(|| {
            Error::Message(format!(
                "Model '{}' does not have a @deletedAt column configured",
                M::TABLE
            ))
        })?;
        let now = chrono::Utc::now().to_rfc3339();
        Ok(self.set(Column::<M, String>::new(M::TABLE, col), now))
    }

    /// Attaches a many-to-many nested write to this update.
    ///
    /// The write runs in the same transaction as the parent update.
    pub fn with_m2m<C: Model + Unpin, J: Model>(mut self, m2m: M2mWrite<'db, M, C, J>) -> Self {
        self.m2m = Some(Arc::new(m2m) as Arc<dyn AnyM2mWrite<M> + 'db>);
        self
    }

    /// Attaches a one-to-many nested write to this update.
    ///
    /// This is the low-level building block; use [`connect`](UpdateQuery::connect),
    /// [`disconnect`](UpdateQuery::disconnect), or [`set_related`](UpdateQuery::set_related) for
    /// the common cases.
    pub fn with_related<C: Model>(mut self, write: RelWrite<M, C>) -> Self {
        self.rel.push(Arc::new(write) as Arc<dyn AnyRelWrite<M>>);
        self
    }

    /// Connects existing child rows to the parent updated by this query.
    ///
    /// `get_parent_pk` extracts the parent primary key from the updated row;
    /// `child_fk_col` is the child's foreign-key column; `child_pk_col` is the
    /// child's primary-key column; and `pks` are the primary keys of the child
    /// rows to connect. The query must match exactly one parent row.
    pub fn connect<C: Model, P: IntoIterator<Item = V>, V: Encodable>(
        self,
        get_parent_pk: fn(&M) -> Value,
        child_fk_col: &'static str,
        child_pk_col: &'static str,
        pks: P,
    ) -> Self {
        let pks = pks.into_iter().map(|v| v.to_value()).collect();
        self.with_related(RelWrite::<M, C>::new(
            RelAction::Connect,
            child_fk_col,
            child_pk_col,
            pks,
            get_parent_pk,
        ))
    }

    /// Disconnects the given child rows from the parent updated by this query.
    pub fn disconnect<C: Model, P: IntoIterator<Item = V>, V: Encodable>(
        self,
        get_parent_pk: fn(&M) -> Value,
        child_fk_col: &'static str,
        child_pk_col: &'static str,
        pks: P,
    ) -> Self {
        let pks = pks.into_iter().map(|v| v.to_value()).collect();
        self.with_related(RelWrite::<M, C>::new(
            RelAction::Disconnect,
            child_fk_col,
            child_pk_col,
            pks,
            get_parent_pk,
        ))
    }

    /// Replaces the parent's connected child rows with the given set.
    pub fn set_related<C: Model, P: IntoIterator<Item = V>, V: Encodable>(
        self,
        get_parent_pk: fn(&M) -> Value,
        child_fk_col: &'static str,
        child_pk_col: &'static str,
        pks: P,
    ) -> Self {
        let pks = pks.into_iter().map(|v| v.to_value()).collect();
        self.with_related(RelWrite::<M, C>::new(
            RelAction::Set,
            child_fk_col,
            child_pk_col,
            pks,
            get_parent_pk,
        ))
    }

    /// Attaches an atomic nested relation write to this update.
    pub fn with_nested_write<W: crate::nested::AnyNestedWrite<M> + 'static>(
        mut self,
        write: W,
    ) -> Self {
        self.nested_writes.push(Arc::new(write));
        self
    }

    /// Sets the filter condition using `.where(...)`.
    pub fn r#where(self, f: Filter<M>) -> Self {
        self.filter(f)
    }

    /// Alias for [`where`](UpdateQuery::where).
    pub fn where_clause(self, f: Filter<M>) -> Self {
        self.filter(f)
    }

    /// Alias for [`exec`](UpdateQuery::exec) to provide an ergonomic builder `.save()` endpoint.
    pub async fn save(self) -> Result<u64, Error> {
        self.exec().await
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
        let compiled = update_with_sets(dialect, M::TABLE, &self.sets, &self.filter.node, &[]);
        query_manifest::record(
            compiled.sql.clone().into_owned(),
            Some(file!()),
            Some(line!()),
            dialect.name(),
        );
        Ok(compiled)
    }

    /// Executes the update and returns the number of rows affected.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Sqlx`] for database errors.
    pub async fn exec(self) -> Result<u64, Error> {
        let has_nested =
            self.m2m.is_some() || !self.rel.is_empty() || !self.nested_writes.is_empty();

        if !has_nested {
            let compiled = self.to_sql()?;
            return self.pool.execute_raw(compiled.sql, compiled.binds).await;
        }

        if !self.all_rows && matches!(self.filter.node, FilterNode::And(ref v) if v.is_empty()) {
            return Err(Error::Message(
                "nested update has no filter; call .all_rows() to target every row".into(),
            ));
        }

        let tx = crate::tx::Tx::begin(self.pool).await?;

        let mut total = 0u64;
        if !self.sets.is_empty() {
            let compiled = self.to_sql()?;
            total += tx.execute_raw(compiled.sql, compiled.binds).await?;
        }

        let parents = self.fetch_parents(&tx).await?;
        if (!self.rel.is_empty() || !self.nested_writes.is_empty()) && parents.len() != 1 {
            return Err(Error::Message(
                "nested writes require exactly one parent row".into(),
            ));
        }

        if let Some(first_parent) = parents.first() {
            for rel in &self.rel {
                let parent_pk = rel.parent_pk(first_parent);
                total += rel.execute_update(&tx, parent_pk).await?;
            }

            for nw in &self.nested_writes {
                let parent_pk = nw.parent_pk(first_parent);
                nw.execute(&tx, parent_pk).await?;
                total += 1;
            }
        }

        if let Some(ref m2m) = self.m2m {
            for parent in &parents {
                let pk = m2m.parent_pk(parent);
                total += m2m.execute_update(&tx, pk).await?;
            }
        }

        tx.commit().await?;
        Ok(total)
    }

    async fn fetch_parents(&self, exec: &dyn Executor) -> Result<Vec<M>, Error> {
        let dialect = exec.dialect();
        let compiled = select::<M>(
            dialect,
            M::TABLE,
            M::COLUMNS,
            &self.filter.node,
            &[],
            None,
            None,
            false,
        );
        let batch = exec.fetch_all_raw(compiled.sql, compiled.binds).await?;
        crate::executor::decode_rows::<M>(batch)
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
    cascade: Vec<Arc<dyn AnyRelDelete<M>>>,
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
            cascade: Vec::new(),
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
            cascade: self.cascade,
            _state: PhantomData,
            _marker: PhantomData,
        }
    }

    /// Adds a filter only if `f` is `Some`, otherwise deletes all rows.
    pub fn filter_if(self, f: Option<Filter<M>>) -> DeleteQuery<'db, M, FilteredDelete> {
        match f {
            Some(f) => self.filter(f),
            None => self.all_rows(),
        }
    }

    /// Allows deleting all rows.
    pub fn all_rows(self) -> DeleteQuery<'db, M, FilteredDelete> {
        DeleteQuery {
            pool: self.pool,
            filter: self.filter,
            all_rows: true,
            cascade: self.cascade,
            _state: PhantomData,
            _marker: PhantomData,
        }
    }

    /// Cascades this delete to child rows according to the given referential
    /// action.
    ///
    /// `child_fk_col` is the child's foreign-key column that references this
    /// model's primary key. This must match the `onDelete` declared in the
    /// schema for the relation.
    pub fn cascade<C: Model>(mut self, child_fk_col: &'static str, action: DeleteAction) -> Self {
        self.cascade
            .push(Arc::new(DeleteCascade::<C>::new(child_fk_col, action)));
        self
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

    /// Adds an additional filter only if `f` is `Some`.
    pub fn filter_if(self, f: Option<Filter<M>>) -> Self {
        match f {
            Some(f) => self.filter(f),
            None => self,
        }
    }

    /// Cascades this delete to child rows according to the given referential
    /// action.
    pub fn cascade<C: Model>(mut self, child_fk_col: &'static str, action: DeleteAction) -> Self {
        self.cascade
            .push(Arc::new(DeleteCascade::<C>::new(child_fk_col, action)));
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
        let compiled = delete::<M>(dialect, M::TABLE, &self.filter.node, &[]);
        query_manifest::record(
            compiled.sql.clone().into_owned(),
            Some(file!()),
            Some(line!()),
            dialect.name(),
        );
        Ok(compiled)
    }

    /// Executes the delete and returns the number of rows removed.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Sqlx`] for database errors.
    pub async fn exec(self) -> Result<u64, Error> {
        let compiled = self.to_sql()?;
        if self.cascade.is_empty() {
            return self.pool.execute_raw(compiled.sql, compiled.binds).await;
        }

        let tx = crate::tx::Tx::begin(self.pool).await?;
        let parent_subquery = self.parent_subquery()?;

        let mut total = 0u64;
        for cascade in &self.cascade {
            total += cascade.execute_delete(&tx, parent_subquery.clone()).await?;
        }
        total += tx.execute_raw(compiled.sql, compiled.binds).await?;
        tx.commit().await?;
        Ok(total)
    }

    fn parent_subquery(&self) -> Result<CompiledSql, Error> {
        let pk_col = Column::<M, Value>::new(M::TABLE, M::PRIMARY_KEY);
        SelectQuery::<M>::new(self.pool)
            .filter(self.filter.clone())
            .columns(pk_col)
            .to_sql()
    }
}

/// A grouped `SELECT` query, created by [`SelectQuery::group_by`].
///
/// Add `HAVING` filters with [`having`](GroupedQuery::having), then call
/// [`aggregate`](GroupedQuery::aggregate) to produce the final result set.
#[derive(Clone)]
#[allow(dead_code)]
pub struct GroupedQuery<'db, M: Model, Out = M, I = ()> {
    inner: SelectQuery<'db, M, Out, I>,
    group_by: Vec<&'static str>,
    having: Filter<M>,
    _marker: PhantomData<fn() -> M>,
}

impl<'db, M, Out, I> GroupedQuery<'db, M, Out, I>
where
    M: Model,
{
    /// Adds a `HAVING` filter (`AND`).
    ///
    /// `HAVING` is applied after grouping; the filter is the same `Filter<M>`
    /// used for `WHERE`, so group columns can be referenced directly.
    pub fn having(self, f: Filter<M>) -> Self {
        Self {
            having: self.having.and(f),
            ..self
        }
    }

    /// Adds an additional `HAVING` filter (`OR`).
    pub fn or_having(self, f: Filter<M>) -> Self {
        Self {
            having: self.having.or(f),
            ..self
        }
    }

    /// Adds a `HAVING` filter (`AND`) only if `f` is `Some`.
    pub fn having_if(self, f: Option<Filter<M>>) -> Self {
        match f {
            Some(f) => self.having(f),
            None => self,
        }
    }

    /// Adds a `HAVING` filter (`OR`) only if `f` is `Some`.
    pub fn or_having_if(self, f: Option<Filter<M>>) -> Self {
        match f {
            Some(f) => self.or_having(f),
            None => self,
        }
    }

    /// Switches the grouped query to return aggregate results.
    #[must_use]
    pub fn aggregate<R, A>(self, set: A) -> AggregateQuery<'db, M, R>
    where
        R: RowDecode,
        A: AggregateSet<M, R>,
    {
        let mut entries = Vec::new();
        set.push_entries(&mut entries);
        AggregateQuery {
            exec: self.inner.exec,
            filter: self.inner.filter,
            aggregates: entries,
            group_by: self.group_by,
            having: self.having,
            order: self.inner.order,
            limit: self.inner.limit,
            offset: self.inner.offset,
            ctes: self.inner.ctes,
            _out: PhantomData,
        }
    }
}

/// A `SELECT` query whose projection is a set of aggregate expressions.
///
/// The output type `R` is a Rust tuple such as `(Option<i64>, i64)`, one element
/// per aggregate expression. Generated per-model structs for named fields are
/// added in Step 4.
#[derive(Clone)]
#[allow(dead_code)]
pub struct AggregateQuery<'db, M: Model, R: RowDecode> {
    exec: &'db dyn Executor,
    filter: Filter<M>,
    aggregates: Vec<AggregateEntry>,
    group_by: Vec<&'static str>,
    having: Filter<M>,
    order: Vec<OrderBy<M>>,
    limit: Option<u64>,
    offset: Option<u64>,
    ctes: Vec<Cte>,
    _out: PhantomData<fn() -> R>,
}

impl<'db, M, R> AggregateQuery<'db, M, R>
where
    M: Model,
    R: RowDecode,
{
    /// Adds a `WHERE` filter (`AND`).
    pub fn filter(self, f: Filter<M>) -> Self {
        Self {
            filter: self.filter.and(f),
            ..self
        }
    }

    /// Adds a `WHERE` filter (`OR`).
    pub fn or_filter(self, f: Filter<M>) -> Self {
        Self {
            filter: self.filter.or(f),
            ..self
        }
    }

    /// Adds a `WHERE` filter (`AND`) only if `f` is `Some`.
    pub fn filter_if(self, f: Option<Filter<M>>) -> Self {
        match f {
            Some(f) => self.filter(f),
            None => self,
        }
    }

    /// Adds a `WHERE` filter (`OR`) only if `f` is `Some`.
    pub fn or_filter_if(self, f: Option<Filter<M>>) -> Self {
        match f {
            Some(f) => self.or_filter(f),
            None => self,
        }
    }

    /// Adds a `HAVING` filter (`AND`).
    pub fn having(self, f: Filter<M>) -> Self {
        Self {
            having: self.having.and(f),
            ..self
        }
    }

    /// Adds a `HAVING` filter (`OR`).
    pub fn or_having(self, f: Filter<M>) -> Self {
        Self {
            having: self.having.or(f),
            ..self
        }
    }

    /// Adds a `HAVING` filter (`AND`) only if `f` is `Some`.
    pub fn having_if(self, f: Option<Filter<M>>) -> Self {
        match f {
            Some(f) => self.having(f),
            None => self,
        }
    }

    /// Adds a `HAVING` filter (`OR`) only if `f` is `Some`.
    pub fn or_having_if(self, f: Option<Filter<M>>) -> Self {
        match f {
            Some(f) => self.or_having(f),
            None => self,
        }
    }

    /// Adds an ordering.
    pub fn order_by(self, o: OrderBy<M>) -> Self {
        let mut order = self.order;
        order.push(o);
        Self { order, ..self }
    }

    /// Adds an ordering only if `o` is `Some`.
    pub fn order_by_if(self, o: Option<OrderBy<M>>) -> Self {
        match o {
            Some(o) => self.order_by(o),
            None => self,
        }
    }

    /// Sets the limit.
    pub fn limit(self, n: u64) -> Self {
        Self {
            limit: Some(n),
            ..self
        }
    }

    /// Sets the limit only if `n` is `Some`.
    pub fn limit_if(self, n: Option<u64>) -> Self {
        match n {
            Some(n) => self.limit(n),
            None => self,
        }
    }

    /// Sets the offset.
    pub fn offset(self, n: u64) -> Self {
        Self {
            offset: Some(n),
            ..self
        }
    }

    /// Sets the offset only if `n` is `Some`.
    pub fn offset_if(self, n: Option<u64>) -> Self {
        match n {
            Some(n) => self.offset(n),
            None => self,
        }
    }

    /// Compiles the query to SQL and binds.
    pub fn to_sql(&self) -> CompiledSql {
        let dialect = self.exec.dialect();
        let main = crate::compile::aggregate_select::<M>(
            dialect,
            M::TABLE,
            &self.aggregates,
            &self.filter.node,
            &self.group_by,
            &self.having.node,
            &self.order,
            self.limit,
            self.offset,
        );
        let compiled = crate::compile::with_cte_prefix(dialect, &self.ctes, main);
        query_manifest::record(
            compiled.sql.clone().into_owned(),
            Some(file!()),
            Some(line!()),
            dialect.name(),
        );
        compiled
    }

    /// Executes the query and returns all matching aggregate rows.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Sqlx`] for database errors.
    pub async fn fetch_all(self) -> Result<Vec<R>, Error>
    where
        R: Send + Unpin,
    {
        let compiled = self.to_sql();

        #[cfg(feature = "sqlite-rusqlite")]
        if let Some(pool) = self.exec.as_rusqlite() {
            self.exec.on_query();
            return pool
                .fetch_all_sync_decoded::<R>(compiled.sql, compiled.binds)
                .await;
        }

        let batch = self
            .exec
            .fetch_all_raw(compiled.sql, compiled.binds)
            .await?;
        crate::executor::decode_rows(batch)
    }

    /// Executes the query and returns the first aggregate row, if any.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Sqlx`] for database errors.
    pub async fn fetch_optional(mut self) -> Result<Option<R>, Error>
    where
        R: Send + Unpin,
    {
        self.limit = Some(1);
        let compiled = self.to_sql();

        #[cfg(feature = "sqlite-rusqlite")]
        let v: Vec<R> = if let Some(pool) = self.exec.as_rusqlite() {
            self.exec.on_query();
            pool.fetch_all_sync_decoded::<R>(compiled.sql, compiled.binds)
                .await?
        } else {
            let batch = self
                .exec
                .fetch_all_raw(compiled.sql, compiled.binds)
                .await?;
            crate::executor::decode_rows(batch)?
        };

        #[cfg(not(feature = "sqlite-rusqlite"))]
        let v: Vec<R> = {
            let batch = self
                .exec
                .fetch_all_raw(compiled.sql, compiled.binds)
                .await?;
            crate::executor::decode_rows(batch)?
        };

        Ok(v.into_iter().next())
    }

    /// Executes the query and returns exactly one aggregate row.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Sqlx`] for database errors, including the case where no
    /// row matches.
    pub async fn fetch_one(self) -> Result<R, Error>
    where
        R: Send + Unpin,
    {
        self.fetch_optional()
            .await?
            .ok_or_else(|| Error::Message("no row found for aggregate query".into()))
    }
}
