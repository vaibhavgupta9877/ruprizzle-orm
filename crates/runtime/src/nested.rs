//! Nested relational mutation engine for atomic multi-entity operations.
//!
//! Supports `create`, `connect`, `connect_or_create`, `disconnect`, `set`, and `delete`
//! within a single unified transaction.

use std::marker::PhantomData;

use crate::BoxFuture;
use crate::col::Column;
use crate::compile::{delete, insert, select, update};
use crate::error::Error;
use crate::executor::Executor;
use crate::filter::{Filter, FilterNode};
use crate::model::Model;
use crate::value::{Encodable, Value};

/// A builder for creating a nested child entity.
#[derive(Debug, Clone)]
pub struct NestedCreate<C: Model> {
    /// Column values for the child entity.
    pub values: Vec<(&'static str, Value)>,
    _marker: PhantomData<fn() -> C>,
}

impl<C: Model> Default for NestedCreate<C> {
    fn default() -> Self {
        Self::new()
    }
}

impl<C: Model> NestedCreate<C> {
    /// Creates a new empty nested create builder.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            values: Vec::new(),
            _marker: PhantomData,
        }
    }

    /// Sets a column value on the child.
    pub fn set<V: Encodable>(mut self, col: Column<C, V>, value: impl Into<V>) -> Self {
        self.values.push((col.column, value.into().to_value()));
        self
    }

    /// Sets a column value if `Some`.
    pub fn set_if<V: Encodable>(self, col: Column<C, V>, value: Option<impl Into<V>>) -> Self {
        match value {
            Some(v) => self.set(col, v),
            None => self,
        }
    }
}

/// A specification for nested `connect_or_create` operations.
#[derive(Debug, Clone)]
pub struct NestedConnectOrCreate<C: Model> {
    /// Filter used to check if the child entity already exists.
    pub filter: Filter<C>,
    /// Fallback values to create the child entity if not found.
    pub create: NestedCreate<C>,
}

impl<C: Model> NestedConnectOrCreate<C> {
    /// Creates a new `NestedConnectOrCreate`.
    #[must_use]
    pub const fn new(filter: Filter<C>, create: NestedCreate<C>) -> Self {
        Self { filter, create }
    }
}

/// An atomic operation on a one-to-many or one-to-one child relation.
pub enum RelNestedOp<C: Model> {
    /// Create child rows with given values.
    Create(Vec<NestedCreate<C>>),
    /// Connect existing child rows by primary key.
    Connect(Vec<Value>),
    /// Connect matching child or create if not found.
    ConnectOrCreate(Vec<NestedConnectOrCreate<C>>),
    /// Disconnect child rows by primary key (sets FK to NULL).
    Disconnect(Vec<Value>),
    /// Replace all children with the given list of primary keys.
    Set(Vec<Value>),
    /// Delete child rows by primary key.
    Delete(Vec<Value>),
}

/// A nested write specification on model `M` targeting child relation `C`.
pub struct NestedRelWrite<M: Model, C: Model> {
    get_parent_pk: fn(&M) -> Value,
    child_table: &'static str,
    child_fk_col: &'static str,
    child_pk_col: &'static str,
    ops: Vec<RelNestedOp<C>>,
    _marker: PhantomData<fn() -> (M, C)>,
}

impl<M: Model, C: Model> NestedRelWrite<M, C> {
    /// Creates a new nested relation write.
    pub const fn new(
        get_parent_pk: fn(&M) -> Value,
        child_table: &'static str,
        child_fk_col: &'static str,
        child_pk_col: &'static str,
        ops: Vec<RelNestedOp<C>>,
    ) -> Self {
        Self {
            get_parent_pk,
            child_table,
            child_fk_col,
            child_pk_col,
            ops,
            _marker: PhantomData,
        }
    }

    /// Runs all operations within this nested write for a single parent primary key.
    pub async fn execute<'a>(
        &'a self,
        exec: &'a dyn Executor,
        parent_pk: Value,
    ) -> Result<(), Error>
    where
        C: Unpin,
    {
        let dialect = exec.dialect();

        for op in &self.ops {
            match op {
                RelNestedOp::Create(creates) => {
                    for c in creates {
                        let mut row_vals = c.values.clone();
                        row_vals.push((self.child_fk_col, parent_pk.clone()));
                        let compiled = insert::<C>(dialect, self.child_table, &row_vals, &[]);
                        exec.execute_raw(compiled.sql, compiled.binds).await?;
                    }
                }
                RelNestedOp::Connect(pks) => {
                    if !pks.is_empty() {
                        let filter = FilterNode::In {
                            table: self.child_table,
                            column: self.child_pk_col,
                            values: pks.clone(),
                            negated: false,
                        };
                        let compiled = update::<C>(
                            dialect,
                            self.child_table,
                            &[(self.child_fk_col, parent_pk.clone())],
                            &filter,
                            &[],
                        );
                        exec.execute_raw(compiled.sql, compiled.binds).await?;
                    }
                }
                RelNestedOp::ConnectOrCreate(items) => {
                    for item in items {
                        let compiled_sel = select::<C>(
                            dialect,
                            self.child_table,
                            &[self.child_pk_col],
                            &item.filter.node,
                            &[],
                            Some(1),
                            None,
                            false,
                        );
                        let found = exec
                            .fetch_all_raw(compiled_sel.sql, compiled_sel.binds)
                            .await?;
                        if !found.is_empty() {
                            // Update FK on existing row
                            let compiled_up = update::<C>(
                                dialect,
                                self.child_table,
                                &[(self.child_fk_col, parent_pk.clone())],
                                &item.filter.node,
                                &[],
                            );
                            exec.execute_raw(compiled_up.sql, compiled_up.binds).await?;
                        } else {
                            // Insert new row
                            let mut row_vals = item.create.values.clone();
                            row_vals.push((self.child_fk_col, parent_pk.clone()));
                            let compiled_ins =
                                insert::<C>(dialect, self.child_table, &row_vals, &[]);
                            exec.execute_raw(compiled_ins.sql, compiled_ins.binds)
                                .await?;
                        }
                    }
                }
                RelNestedOp::Disconnect(pks) => {
                    if !pks.is_empty() {
                        let filter = FilterNode::And(vec![
                            FilterNode::In {
                                table: self.child_table,
                                column: self.child_pk_col,
                                values: pks.clone(),
                                negated: false,
                            },
                            FilterNode::Cmp {
                                table: self.child_table,
                                column: self.child_fk_col,
                                op: crate::filter::CmpOp::Eq,
                                value: parent_pk.clone(),
                            },
                        ]);
                        let compiled = update::<C>(
                            dialect,
                            self.child_table,
                            &[(self.child_fk_col, Value::Null)],
                            &filter,
                            &[],
                        );
                        exec.execute_raw(compiled.sql, compiled.binds).await?;
                    }
                }
                RelNestedOp::Set(pks) => {
                    // 1. Disconnect all existing children
                    let disconnect_filter = FilterNode::Cmp {
                        table: self.child_table,
                        column: self.child_fk_col,
                        op: crate::filter::CmpOp::Eq,
                        value: parent_pk.clone(),
                    };
                    let compiled_disc = update::<C>(
                        dialect,
                        self.child_table,
                        &[(self.child_fk_col, Value::Null)],
                        &disconnect_filter,
                        &[],
                    );
                    exec.execute_raw(compiled_disc.sql, compiled_disc.binds)
                        .await?;

                    // 2. Connect specified children
                    if !pks.is_empty() {
                        let filter = FilterNode::In {
                            table: self.child_table,
                            column: self.child_pk_col,
                            values: pks.clone(),
                            negated: false,
                        };
                        let compiled_conn = update::<C>(
                            dialect,
                            self.child_table,
                            &[(self.child_fk_col, parent_pk.clone())],
                            &filter,
                            &[],
                        );
                        exec.execute_raw(compiled_conn.sql, compiled_conn.binds)
                            .await?;
                    }
                }
                RelNestedOp::Delete(pks) => {
                    if !pks.is_empty() {
                        let filter = FilterNode::And(vec![
                            FilterNode::In {
                                table: self.child_table,
                                column: self.child_pk_col,
                                values: pks.clone(),
                                negated: false,
                            },
                            FilterNode::Cmp {
                                table: self.child_table,
                                column: self.child_fk_col,
                                op: crate::filter::CmpOp::Eq,
                                value: parent_pk.clone(),
                            },
                        ]);
                        let compiled = delete::<C>(dialect, self.child_table, &filter, &[]);
                        exec.execute_raw(compiled.sql, compiled.binds).await?;
                    }
                }
            }
        }

        Ok(())
    }
}

/// Object-safe interface for any nested write attached to a parent model `M`.
pub trait AnyNestedWrite<M: Model>: Send + Sync + std::fmt::Debug {
    /// Extracts the parent primary key from the parent model instance.
    fn parent_pk(&self, parent: &M) -> Value;

    /// Runs all operations within this nested write for a single parent primary key.
    fn execute<'a>(
        &'a self,
        exec: &'a dyn Executor,
        parent_pk: Value,
    ) -> BoxFuture<'a, Result<(), Error>>;
}

impl<M: Model, C: Model + Unpin> AnyNestedWrite<M> for NestedRelWrite<M, C> {
    fn parent_pk(&self, parent: &M) -> Value {
        (self.get_parent_pk)(parent)
    }

    fn execute<'a>(
        &'a self,
        exec: &'a dyn Executor,
        parent_pk: Value,
    ) -> BoxFuture<'a, Result<(), Error>> {
        Box::pin(self.execute(exec, parent_pk))
    }
}

impl<M: Model, C: Model> std::fmt::Debug for NestedRelWrite<M, C> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NestedRelWrite")
            .field("child_table", &self.child_table)
            .field("child_fk_col", &self.child_fk_col)
            .field("child_pk_col", &self.child_pk_col)
            .finish_non_exhaustive()
    }
}
