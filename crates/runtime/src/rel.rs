//! One-to-many relation mutations: connect, disconnect, set, and cascade.
//!
//! These complement [`crate::m2m`] (many-to-many attach/set/detach) with the
//! corresponding operations for one-to-many relations, where the child table
//! stores the foreign key directly.

use std::marker::PhantomData;

use ruprizzle_core::ir::ReferentialAction;

use crate::BoxFuture;
use crate::compile::{CompiledSql, delete, select, update};
use crate::error::Error;
use crate::executor::Executor;
use crate::filter::FilterNode;
use crate::model::Model;
use crate::value::Value;

/// Action to take on the child rows of a one-to-many relation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RelAction {
    /// Connect existing child rows to a parent by setting their foreign key.
    Connect,
    /// Disconnect the given child rows from a parent by nulling their foreign
    /// key.
    Disconnect,
    /// Replace the parent’s current children with the given child rows.
    Set,
}

/// Alias for the referential action used when deleting a parent.
pub type DeleteAction = ReferentialAction;

/// A one-to-many nested write for a single parent model `M`.
///
/// `C` is the child model on the other side of the relation.
pub struct RelWrite<M: Model, C: Model> {
    action: RelAction,
    child_fk_col: &'static str,
    child_pk_col: &'static str,
    pks: Vec<Value>,
    get_parent_pk: fn(&M) -> Value,
    _marker: PhantomData<fn() -> C>,
}

impl<M: Model, C: Model> RelWrite<M, C> {
    /// Creates a new one-to-many nested write.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        action: RelAction,
        child_fk_col: &'static str,
        child_pk_col: &'static str,
        pks: Vec<Value>,
        get_parent_pk: fn(&M) -> Value,
    ) -> Self {
        Self {
            action,
            child_fk_col,
            child_pk_col,
            pks,
            get_parent_pk,
            _marker: PhantomData,
        }
    }

    /// Extracts the parent primary key from a parent row.
    pub fn parent_pk(&self, parent: &M) -> Value {
        (self.get_parent_pk)(parent)
    }

    /// Runs this write inside an `UPDATE` for a single parent primary key,
    /// returning the number of child rows affected.
    pub async fn execute_update<'a>(
        &'a self,
        exec: &'a dyn Executor,
        parent_pk: Value,
    ) -> Result<u64, Error> {
        match self.action {
            RelAction::Connect => self.connect(exec, parent_pk).await,
            RelAction::Disconnect => self.disconnect(exec, parent_pk).await,
            RelAction::Set => self.set(exec, parent_pk).await,
        }
    }

    async fn connect(&self, exec: &dyn Executor, parent_pk: Value) -> Result<u64, Error> {
        let dialect = exec.dialect();
        let filter = FilterNode::In {
            table: C::TABLE,
            column: self.child_pk_col,
            values: self.pks.clone(),
            negated: false,
        };
        let compiled = update::<M>(
            dialect,
            C::TABLE,
            &[(self.child_fk_col, parent_pk)],
            &filter,
            &[],
        );
        exec.execute_raw(compiled.sql, compiled.binds).await
    }

    async fn disconnect(&self, exec: &dyn Executor, parent_pk: Value) -> Result<u64, Error> {
        let dialect = exec.dialect();
        let filter = FilterNode::And(vec![
            FilterNode::In {
                table: C::TABLE,
                column: self.child_pk_col,
                values: self.pks.clone(),
                negated: false,
            },
            FilterNode::Cmp {
                table: C::TABLE,
                column: self.child_fk_col,
                op: crate::filter::CmpOp::Eq,
                value: parent_pk,
            },
        ]);
        let compiled = update::<M>(
            dialect,
            C::TABLE,
            &[(self.child_fk_col, Value::Null)],
            &filter,
            &[],
        );
        exec.execute_raw(compiled.sql, compiled.binds).await
    }

    async fn set(&self, exec: &dyn Executor, parent_pk: Value) -> Result<u64, Error> {
        let mut total = self.disconnect_all(exec, parent_pk.clone()).await?;
        total += self.connect(exec, parent_pk).await?;
        Ok(total)
    }

    async fn disconnect_all(&self, exec: &dyn Executor, parent_pk: Value) -> Result<u64, Error> {
        let dialect = exec.dialect();
        let filter = FilterNode::Cmp {
            table: C::TABLE,
            column: self.child_fk_col,
            op: crate::filter::CmpOp::Eq,
            value: parent_pk,
        };
        let compiled = update::<M>(
            dialect,
            C::TABLE,
            &[(self.child_fk_col, Value::Null)],
            &filter,
            &[],
        );
        exec.execute_raw(compiled.sql, compiled.binds).await
    }
}

impl<M: Model, C: Model> std::fmt::Debug for RelWrite<M, C> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RelWrite")
            .field("action", &self.action)
            .field("child_table", &C::TABLE)
            .field("child_fk_col", &self.child_fk_col)
            .field("child_pk_col", &self.child_pk_col)
            .field("pks", &self.pks)
            .finish_non_exhaustive()
    }
}

/// Object-safe interface for [`RelWrite`] so `UpdateQuery` can store concrete
/// one-to-many writes without becoming generic over the child model.
pub(crate) trait AnyRelWrite<M: Model>: Send + Sync + std::fmt::Debug {
    /// Extracts the parent primary key from a parent row.
    fn parent_pk(&self, parent: &M) -> Value;

    /// Runs this write for a single parent primary key, returning the number of
    /// child rows affected.
    fn execute_update<'a>(
        &'a self,
        exec: &'a dyn Executor,
        parent_pk: Value,
    ) -> BoxFuture<'a, Result<u64, Error>>;
}

impl<M: Model, C: Model> AnyRelWrite<M> for RelWrite<M, C> {
    fn parent_pk(&self, parent: &M) -> Value {
        RelWrite::parent_pk(self, parent)
    }

    fn execute_update<'a>(
        &'a self,
        exec: &'a dyn Executor,
        parent_pk: Value,
    ) -> BoxFuture<'a, Result<u64, Error>> {
        Box::pin(self.execute_update(exec, parent_pk))
    }
}

/// Specification for cascading a parent `DELETE` to child rows.
pub struct DeleteCascade<C: Model> {
    child_fk_col: &'static str,
    action: DeleteAction,
    _marker: PhantomData<C>,
}

impl<C: Model> DeleteCascade<C> {
    /// Creates a new cascade specification.
    pub fn new(child_fk_col: &'static str, action: DeleteAction) -> Self {
        Self {
            child_fk_col,
            action,
            _marker: PhantomData,
        }
    }

    /// Runs this cascade before the parent rows are deleted, using the parent
    /// primary-key subquery compiled from the delete filter.
    pub async fn execute_delete<'a, M: Model>(
        &'a self,
        exec: &'a dyn Executor,
        parent_subquery: CompiledSql,
    ) -> Result<u64, Error> {
        match self.action {
            DeleteAction::NoAction => Ok(0),
            DeleteAction::Cascade => self.cascade::<M>(exec, parent_subquery).await,
            DeleteAction::SetNull => self.set_null::<M>(exec, parent_subquery).await,
            DeleteAction::Restrict => self.restrict(exec, parent_subquery).await,
            DeleteAction::SetDefault => Err(Error::Message(
                "SET DEFAULT is not supported for cascaded deletes".into(),
            )),
        }
    }

    fn child_filter(&self, parent_subquery: CompiledSql) -> FilterNode {
        FilterNode::InSubquery {
            table: C::TABLE,
            column: self.child_fk_col,
            subquery: parent_subquery,
            negated: false,
        }
    }

    async fn cascade<M: Model>(
        &self,
        exec: &dyn Executor,
        parent_subquery: CompiledSql,
    ) -> Result<u64, Error> {
        let dialect = exec.dialect();
        let filter = self.child_filter(parent_subquery);
        let compiled = delete::<M>(dialect, C::TABLE, &filter, &[]);
        exec.execute_raw(compiled.sql, compiled.binds).await
    }

    async fn set_null<M: Model>(
        &self,
        exec: &dyn Executor,
        parent_subquery: CompiledSql,
    ) -> Result<u64, Error> {
        let dialect = exec.dialect();
        let filter = self.child_filter(parent_subquery);
        let compiled = update::<M>(
            dialect,
            C::TABLE,
            &[(self.child_fk_col, Value::Null)],
            &filter,
            &[],
        );
        exec.execute_raw(compiled.sql, compiled.binds).await
    }

    async fn restrict(
        &self,
        exec: &dyn Executor,
        parent_subquery: CompiledSql,
    ) -> Result<u64, Error> {
        let dialect = exec.dialect();
        let filter = self.child_filter(parent_subquery);
        let compiled = select::<C>(dialect, C::TABLE, &[], &filter, &[], Some(1), None, false);
        let batch = exec.fetch_all_raw(compiled.sql, compiled.binds).await?;
        if !batch.is_empty() {
            return Err(Error::Message(format!(
                "cannot delete parent: child rows exist in {}",
                C::TABLE
            )));
        }
        Ok(0)
    }
}

impl<C: Model> std::fmt::Debug for DeleteCascade<C> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DeleteCascade")
            .field("child_table", &C::TABLE)
            .field("child_fk_col", &self.child_fk_col)
            .field("action", &self.action)
            .finish()
    }
}

/// Object-safe interface for [`DeleteCascade`] so `DeleteQuery` can store
/// cascades without becoming generic over the child model.
pub(crate) trait AnyRelDelete<M: Model>: Send + Sync + std::fmt::Debug {
    /// Runs this cascade before the parent rows are deleted.
    fn execute_delete<'a>(
        &'a self,
        exec: &'a dyn Executor,
        parent_subquery: CompiledSql,
    ) -> BoxFuture<'a, Result<u64, Error>>;
}

impl<M: Model, C: Model> AnyRelDelete<M> for DeleteCascade<C> {
    fn execute_delete<'a>(
        &'a self,
        exec: &'a dyn Executor,
        parent_subquery: CompiledSql,
    ) -> BoxFuture<'a, Result<u64, Error>> {
        Box::pin(self.execute_delete::<M>(exec, parent_subquery))
    }
}
