//! Many-to-many relation mutations: attach, set, and detach target rows
//! through an explicit join model.

use std::marker::PhantomData;

use crate::BoxFuture;
use crate::col::Column;
use crate::compile::{delete, insert_many, select};
use crate::error::Error;
use crate::executor::{Executor, decode_rows};
use crate::filter::{Filter, FilterNode};
use crate::model::Model;
use crate::value::Value;

/// What to do with the join rows for a many-to-many relation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum M2mAction {
    /// Insert new join rows, leaving existing rows in place.
    Attach,
    /// Replace all existing join rows with the given target IDs.
    Set,
    /// Remove the given target IDs from the join rows.
    Detach,
}

/// A nested many-to-many write operation for a parent model `M`.
///
/// `C` is the target model on the other side of the relation; `J` is the join
/// model that stores the `(parent_id, target_id)` pairs.
pub struct M2mWrite<'db, M, C, J> {
    action: M2mAction,
    get_parent_pk: fn(&M) -> Value,
    join_table: &'static str,
    join_owner_col: &'static str,
    join_target_col: &'static str,
    target_table: &'static str,
    target_pk_col: &'static str,
    target_ids: Vec<Value>,
    setter: fn(&mut M, Vec<C>),
    _marker: PhantomData<fn() -> (&'db (), J)>,
}

impl<'db, M, C, J> M2mWrite<'db, M, C, J> {
    /// Creates a new many-to-many write.
    #[allow(clippy::too_many_arguments)]
    pub const fn new(
        action: M2mAction,
        get_parent_pk: fn(&M) -> Value,
        join_table: &'static str,
        join_owner_col: &'static str,
        join_target_col: &'static str,
        target_table: &'static str,
        target_pk_col: &'static str,
        target_ids: Vec<Value>,
        setter: fn(&mut M, Vec<C>),
    ) -> Self {
        Self {
            action,
            get_parent_pk,
            join_table,
            join_owner_col,
            join_target_col,
            target_table,
            target_pk_col,
            target_ids,
            setter,
            _marker: PhantomData,
        }
    }
}

impl<'db, M, C, J> M2mWrite<'db, M, C, J>
where
    M: Model,
    C: Model + Unpin,
    J: Model,
{
    /// Runs this write inside an `INSERT` of a single parent, returning the
    /// parent with the loaded relation attached.
    pub(crate) async fn execute_insert<'a>(
        &'a self,
        exec: &'a dyn Executor,
        parent: &'a mut M,
    ) -> Result<(), Error> {
        let pk = (self.get_parent_pk)(parent);

        let mut affected = 0u64;
        if self.action == M2mAction::Set {
            affected += self.clear_joins(exec, &pk).await?;
        }

        if self.action != M2mAction::Detach {
            affected += self.insert_joins(exec, &pk).await?;
        } else {
            affected += self.delete_specific_joins(exec, &pk).await?;
        }

        let children = if self.action == M2mAction::Detach {
            Vec::new()
        } else if affected > 0 && !self.target_ids.is_empty() {
            self.load_targets(exec).await?
        } else {
            Vec::new()
        };

        (self.setter)(parent, children);
        Ok(())
    }

    /// Runs this write inside an `UPDATE` for a single parent primary key,
    /// returning the number of join rows affected.
    pub(crate) async fn execute_update<'a>(
        &'a self,
        exec: &'a dyn Executor,
        parent_pk: Value,
    ) -> Result<u64, Error> {
        let mut affected = 0u64;
        if self.action == M2mAction::Set {
            affected += self.clear_joins(exec, &parent_pk).await?;
        }

        if self.action != M2mAction::Detach {
            affected += self.insert_joins(exec, &parent_pk).await?;
        } else {
            affected += self.delete_specific_joins(exec, &parent_pk).await?;
        }

        Ok(affected)
    }

    pub(crate) fn parent_pk(&self, parent: &M) -> Value {
        (self.get_parent_pk)(parent)
    }

    fn owner_col(&self) -> Column<J, Value> {
        Column::new(self.join_table, self.join_owner_col)
    }

    fn target_col(&self) -> Column<J, Value> {
        Column::new(self.join_table, self.join_target_col)
    }

    fn target_pk(&self) -> Column<C, Value> {
        Column::new(self.target_table, self.target_pk_col)
    }

    async fn clear_joins(&self, exec: &dyn Executor, pk: &Value) -> Result<u64, Error> {
        let filter =
            Filter::<J>::new(FilterNode::And(Vec::new())).and(self.owner_col().eq(pk.clone()));
        let compiled = delete::<J>(exec.dialect(), self.join_table, &filter.node, &[]);
        exec.execute_raw(compiled.sql, compiled.binds).await
    }

    async fn delete_specific_joins(&self, exec: &dyn Executor, pk: &Value) -> Result<u64, Error> {
        let filter = Filter::<J>::new(FilterNode::And(Vec::new()))
            .and(self.owner_col().eq(pk.clone()))
            .and(self.target_col().in_set(self.target_ids.clone()));
        let compiled = delete::<J>(exec.dialect(), self.join_table, &filter.node, &[]);
        exec.execute_raw(compiled.sql, compiled.binds).await
    }

    async fn insert_joins(&self, exec: &dyn Executor, pk: &Value) -> Result<u64, Error> {
        if self.target_ids.is_empty() {
            return Ok(0);
        }

        let rows: Vec<Vec<(&'static str, Value)>> = self
            .target_ids
            .iter()
            .map(|tid| {
                vec![
                    (self.join_owner_col, pk.clone()),
                    (self.join_target_col, tid.clone()),
                ]
            })
            .collect();

        let compiled = insert_many::<J>(exec.dialect(), self.join_table, &rows, &[]);
        exec.execute_raw(compiled.sql, compiled.binds).await
    }

    async fn load_targets(&self, exec: &dyn Executor) -> Result<Vec<C>, Error> {
        let filter = Filter::<C>::new(FilterNode::And(Vec::new()))
            .and(self.target_pk().in_set(self.target_ids.clone()));
        let compiled = select::<C>(
            exec.dialect(),
            self.target_table,
            C::COLUMNS,
            &filter.node,
            &[],
            None,
            None,
            false,
        );
        let batch = exec.fetch_all_raw(compiled.sql, compiled.binds).await?;
        decode_rows::<C>(batch)
    }
}

/// Object-safe interface for [`M2mWrite`] so `InsertQuery` and `UpdateQuery`
/// can store a concrete write without becoming generic over the join/target
/// models.
pub(crate) trait AnyM2mWrite<M: Model>: Send + Sync + std::fmt::Debug {
    /// Extract the parent primary key from a parent row.
    fn parent_pk(&self, parent: &M) -> Value;

    /// Runs the write after a parent has just been inserted, and attaches the
    /// loaded target rows to the parent.
    fn execute_insert<'a>(
        &'a self,
        exec: &'a dyn Executor,
        parent: &'a mut M,
    ) -> BoxFuture<'a, Result<(), Error>>;

    /// Runs the write for an existing parent primary key, returning the number
    /// of join rows affected.
    fn execute_update<'a>(
        &'a self,
        exec: &'a dyn Executor,
        parent_pk: Value,
    ) -> BoxFuture<'a, Result<u64, Error>>;
}

impl<M, C, J> std::fmt::Debug for M2mWrite<'_, M, C, J> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("M2mWrite")
            .field("action", &self.action)
            .field("join_table", &self.join_table)
            .field("join_owner_col", &self.join_owner_col)
            .field("join_target_col", &self.join_target_col)
            .field("target_table", &self.target_table)
            .field("target_pk_col", &self.target_pk_col)
            .field("target_ids", &self.target_ids)
            .finish_non_exhaustive()
    }
}

impl<M, C, J> AnyM2mWrite<M> for M2mWrite<'_, M, C, J>
where
    M: Model,
    C: Model + Unpin,
    J: Model,
{
    fn parent_pk(&self, parent: &M) -> Value {
        self.parent_pk(parent)
    }

    fn execute_insert<'a>(
        &'a self,
        exec: &'a dyn Executor,
        parent: &'a mut M,
    ) -> BoxFuture<'a, Result<(), Error>> {
        Box::pin(self.execute_insert(exec, parent))
    }

    fn execute_update<'a>(
        &'a self,
        exec: &'a dyn Executor,
        parent_pk: Value,
    ) -> BoxFuture<'a, Result<u64, Error>> {
        Box::pin(self.execute_update(exec, parent_pk))
    }
}
