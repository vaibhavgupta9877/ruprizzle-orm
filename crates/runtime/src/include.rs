//! Batched relation `include` loading.

use std::collections::{HashMap, HashSet};
use std::fmt;
use std::hash::Hash;
use std::marker::PhantomData;

use crate::BoxFuture;
use crate::col::Column;
use crate::compile::select_partitioned;
use crate::error::Error;
use crate::executor::Executor;
use crate::filter::Filter;
use crate::model::Model;
use crate::order::OrderBy;
use crate::query::SelectQuery;
use crate::related::Related;
use crate::value::Encodable;

/// Loads every child row belonging to any of `keys`, in a bounded number of
/// queries.
///
/// `limit` is a **per-parent** limit, not a limit on the batch: `take(5)` means
/// five children for each parent, which is what the caller asked for and what a
/// plain `LIMIT` would silently fail to deliver. It is compiled to a
/// `ROW_NUMBER() OVER (PARTITION BY ...)` window; only if the dialect cannot do
/// windows does this degrade to one query per parent.
async fn fetch_children<C, Key>(
    exec: &dyn Executor,
    child_key: Column<C, Key>,
    filter: &Filter<C>,
    order: &[OrderBy<C>],
    limit: Option<u64>,
    keys: Vec<Key>,
) -> Result<Vec<C>, Error>
where
    C: Model + Send + Unpin + for<'r> sqlx::FromRow<'r, sqlx::any::AnyRow>,
    Key: Encodable + Eq + Hash + Clone + Send + Sync + 'static,
{
    // The parent set repeats keys whenever several parents point at the same
    // child (every many-to-one relation does). Sending the duplicates would
    // inflate the `IN` list and burn the parameter budget for nothing.
    let keys = dedup(keys);
    if keys.is_empty() {
        return Ok(Vec::new());
    }

    let dialect = exec.dialect();
    let caps = dialect.capabilities();
    // Leave headroom for the binds the relation filter itself contributes.
    let chunk_size = (caps.max_query_params as usize).saturating_sub(10).max(1);

    if let Some(n) = limit {
        if !caps.window_functions {
            return fetch_children_per_parent(exec, child_key, filter, order, n, &keys).await;
        }

        let mut all = Vec::new();
        for chunk in keys.chunks(chunk_size) {
            let combined = filter.clone().and(child_key.in_set(chunk.to_vec()));
            let compiled = select_partitioned::<C>(
                dialect.as_ref(),
                C::TABLE,
                child_key.column,
                &combined.node,
                order,
                n,
            );
            let rows = exec.fetch_all_raw(compiled.sql, compiled.binds).await?;
            all.extend(crate::executor::decode_rows::<C>(rows)?);
        }
        return Ok(all);
    }

    let mut all = Vec::new();
    for chunk in keys.chunks(chunk_size) {
        let mut q = SelectQuery::<C>::new(exec).filter(child_key.in_set(chunk.to_vec()));
        if filter.node != crate::filter::FilterNode::And(Vec::new()) {
            q = q.filter(filter.clone());
        }
        for o in order {
            q = q.order_by(*o);
        }
        all.extend(q.fetch_all().await?);
    }
    Ok(all)
}

/// The no-window-function fallback: one `LIMIT`ed query per parent key.
///
/// Correct, but linear in the parent count — the very shape the batched loader
/// exists to avoid. No dialect shipped today takes this path.
async fn fetch_children_per_parent<C, Key>(
    exec: &dyn Executor,
    child_key: Column<C, Key>,
    filter: &Filter<C>,
    order: &[OrderBy<C>],
    limit: u64,
    keys: &[Key],
) -> Result<Vec<C>, Error>
where
    C: Model + Send + Unpin + for<'r> sqlx::FromRow<'r, sqlx::any::AnyRow>,
    Key: Encodable + Clone + Send + Sync + 'static,
{
    let mut all = Vec::new();
    for key in keys {
        let mut q = SelectQuery::<C>::new(exec).filter(child_key.eq(key.clone()));
        if filter.node != crate::filter::FilterNode::And(Vec::new()) {
            q = q.filter(filter.clone());
        }
        for o in order {
            q = q.order_by(*o);
        }
        all.extend(q.limit(limit).fetch_all().await?);
    }
    Ok(all)
}

/// Removes duplicates while keeping first-seen order.
fn dedup<Key: Eq + Hash + Clone>(keys: Vec<Key>) -> Vec<Key> {
    let mut seen = HashSet::with_capacity(keys.len());
    keys.into_iter()
        .filter(|k| seen.insert(k.clone()))
        .collect()
}

/// A set of includes to attach to a parent model.
///
/// This is a type-level "list": a single relation implements it, and nested
/// relations are chained through the relation builder's own `.include()` method.
pub trait IncludeSet<M: Model> {
    /// Loads the related data and attaches it to `parents` in place.
    fn load<'a>(
        &'a self,
        exec: &'a dyn Executor,
        parents: &'a mut [M],
    ) -> BoxFuture<'a, Result<(), Error>>;
}

impl<M: Model> IncludeSet<M> for () {
    fn load<'a>(
        &'a self,
        _exec: &'a dyn Executor,
        _parents: &'a mut [M],
    ) -> BoxFuture<'a, Result<(), Error>> {
        Box::pin(async { Ok(()) })
    }
}

/// Include a one-to-many relation (parent has many children).
pub struct IncludeList<'db, M, C, Key, NI = ()> {
    /// Extract the join key from the parent.
    pub get: fn(&M) -> Key,
    /// Attach the loaded children to the parent.
    pub set: fn(&mut M, Related<Vec<C>>),
    /// The child column that matches the parent key.
    pub child_key: Column<C, Key>,
    /// Extract the join key from a child row.
    pub child_key_get: fn(&C) -> Key,
    /// Optional extra filter on the child rows.
    pub filter: Filter<C>,
    /// Ordering for the child rows.
    pub order: Vec<OrderBy<C>>,
    /// Optional per-child limit (`take`).
    pub limit: Option<u64>,
    /// Nested includes on the child.
    pub nested: NI,
    _marker: PhantomData<fn() -> &'db ()>,
}

impl<'db, M, C, Key, NI> Clone for IncludeList<'db, M, C, Key, NI>
where
    NI: Clone,
{
    fn clone(&self) -> Self {
        Self {
            get: self.get,
            set: self.set,
            child_key: self.child_key,
            child_key_get: self.child_key_get,
            filter: self.filter.clone(),
            order: self.order.clone(),
            limit: self.limit,
            nested: self.nested.clone(),
            _marker: PhantomData,
        }
    }
}

impl<'db, M, C, Key, NI> fmt::Debug for IncludeList<'db, M, C, Key, NI>
where
    NI: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("IncludeList")
            .field("child_key", &self.child_key)
            .field("filter", &self.filter)
            .field("order", &self.order)
            .field("limit", &self.limit)
            .field("nested", &self.nested)
            .finish()
    }
}

impl<'db, M, C, Key> IncludeList<'db, M, C, Key, ()>
where
    M: Model,
    C: Model,
{
    /// Creates a new one-to-many include.
    pub const fn new(
        get: fn(&M) -> Key,
        set: fn(&mut M, Related<Vec<C>>),
        child_key: Column<C, Key>,
        child_key_get: fn(&C) -> Key,
    ) -> Self {
        Self {
            get,
            set,
            child_key,
            child_key_get,
            filter: Filter::new(crate::filter::FilterNode::And(Vec::new())),
            order: Vec::new(),
            limit: None,
            nested: (),
            _marker: PhantomData,
        }
    }
}

impl<'db, M, C, Key, NI> IncludeList<'db, M, C, Key, NI>
where
    M: Model,
    C: Model,
{
    /// Adds an extra filter on the child rows.
    pub fn filter(mut self, f: Filter<C>) -> Self {
        self.filter = self.filter.and(f);
        self
    }

    /// Adds a child ordering.
    pub fn order_by(mut self, o: OrderBy<C>) -> Self {
        self.order.push(o);
        self
    }

    /// Limits the number of children per parent.
    pub fn take(mut self, n: u64) -> Self {
        self.limit = Some(n);
        self
    }

    /// Adds a nested include on the child.
    pub fn include<J: IncludeSet<C>>(self, include: J) -> IncludeList<'db, M, C, Key, J> {
        IncludeList {
            get: self.get,
            set: self.set,
            child_key: self.child_key,
            child_key_get: self.child_key_get,
            filter: self.filter,
            order: self.order,
            limit: self.limit,
            nested: include,
            _marker: PhantomData,
        }
    }
}

impl<'db, M, C, Key, NI> IncludeSet<M> for IncludeList<'db, M, C, Key, NI>
where
    M: Model + Send + Unpin + for<'r> sqlx::FromRow<'r, sqlx::any::AnyRow>,
    C: Model + Send + Unpin + for<'r> sqlx::FromRow<'r, sqlx::any::AnyRow>,
    Key: Encodable + Eq + Hash + Clone + Send + Sync + 'static,
    NI: IncludeSet<C> + Sync,
{
    fn load<'a>(
        &'a self,
        exec: &'a dyn Executor,
        parents: &'a mut [M],
    ) -> BoxFuture<'a, Result<(), Error>> {
        Box::pin(async move {
            if parents.is_empty() {
                return Ok(());
            }

            let keys: Vec<Key> = parents.iter().map(self.get).collect();
            let mut children = fetch_children(
                exec,
                self.child_key,
                &self.filter,
                &self.order,
                self.limit,
                keys,
            )
            .await?;
            self.nested.load(exec, &mut children).await?;

            let mut map: HashMap<Key, Vec<C>> = HashMap::new();
            for child in children {
                let key = (self.child_key_get)(&child);
                map.entry(key).or_default().push(child);
            }

            for parent in parents.iter_mut() {
                let key = (self.get)(parent);
                let related = map.remove(&key).unwrap_or_default();
                (self.set)(parent, Related::Loaded(related));
            }

            Ok(())
        })
    }
}

/// Include a many-to-one / one-to-one relation (parent has one child).
pub struct IncludeOne<'db, M, C, Key, NI = ()> {
    /// Extract the join key from the parent.
    pub get: fn(&M) -> Key,
    /// Attach the loaded child to the parent.
    pub set: fn(&mut M, Related<Option<C>>),
    /// The child column that matches the parent key.
    pub child_key: Column<C, Key>,
    /// Extract the join key from a child row.
    pub child_key_get: fn(&C) -> Key,
    /// Optional extra filter on the child rows.
    pub filter: Filter<C>,
    /// Ordering for the child rows.
    pub order: Vec<OrderBy<C>>,
    /// Optional per-child limit (`take`).
    pub limit: Option<u64>,
    /// Nested includes on the child.
    pub nested: NI,
    _marker: PhantomData<fn() -> &'db ()>,
}

impl<'db, M, C, Key, NI> Clone for IncludeOne<'db, M, C, Key, NI>
where
    NI: Clone,
{
    fn clone(&self) -> Self {
        Self {
            get: self.get,
            set: self.set,
            child_key: self.child_key,
            child_key_get: self.child_key_get,
            filter: self.filter.clone(),
            order: self.order.clone(),
            limit: self.limit,
            nested: self.nested.clone(),
            _marker: PhantomData,
        }
    }
}

impl<'db, M, C, Key, NI> fmt::Debug for IncludeOne<'db, M, C, Key, NI>
where
    NI: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("IncludeOne")
            .field("child_key", &self.child_key)
            .field("filter", &self.filter)
            .field("order", &self.order)
            .field("limit", &self.limit)
            .field("nested", &self.nested)
            .finish()
    }
}

impl<'db, M, C, Key> IncludeOne<'db, M, C, Key, ()>
where
    M: Model,
    C: Model,
{
    /// Creates a new many-to-one / one-to-one include.
    pub const fn new(
        get: fn(&M) -> Key,
        set: fn(&mut M, Related<Option<C>>),
        child_key: Column<C, Key>,
        child_key_get: fn(&C) -> Key,
    ) -> Self {
        Self {
            get,
            set,
            child_key,
            child_key_get,
            filter: Filter::new(crate::filter::FilterNode::And(Vec::new())),
            order: Vec::new(),
            limit: None,
            nested: (),
            _marker: PhantomData,
        }
    }
}

impl<'db, M, C, Key, NI> IncludeOne<'db, M, C, Key, NI>
where
    M: Model,
    C: Model,
{
    /// Adds an extra filter on the child rows.
    pub fn filter(mut self, f: Filter<C>) -> Self {
        self.filter = self.filter.and(f);
        self
    }

    /// Adds a child ordering.
    pub fn order_by(mut self, o: OrderBy<C>) -> Self {
        self.order.push(o);
        self
    }

    /// Limits the number of children (normally 1 for a single relation).
    pub fn take(mut self, n: u64) -> Self {
        self.limit = Some(n);
        self
    }

    /// Adds a nested include on the child.
    pub fn include<J: IncludeSet<C>>(self, include: J) -> IncludeOne<'db, M, C, Key, J> {
        IncludeOne {
            get: self.get,
            set: self.set,
            child_key: self.child_key,
            child_key_get: self.child_key_get,
            filter: self.filter,
            order: self.order,
            limit: self.limit,
            nested: include,
            _marker: PhantomData,
        }
    }
}

impl<'db, M, C, Key, NI> IncludeSet<M> for IncludeOne<'db, M, C, Key, NI>
where
    M: Model + Send + Unpin + for<'r> sqlx::FromRow<'r, sqlx::any::AnyRow>,
    // `Clone` because a many-to-one relation is many-to-one: several parents
    // routinely share one child row, and each parent owns its own copy.
    C: Model + Clone + Send + Unpin + for<'r> sqlx::FromRow<'r, sqlx::any::AnyRow>,
    Key: Encodable + Eq + Hash + Clone + Send + Sync + 'static,
    NI: IncludeSet<C> + Sync,
{
    fn load<'a>(
        &'a self,
        exec: &'a dyn Executor,
        parents: &'a mut [M],
    ) -> BoxFuture<'a, Result<(), Error>> {
        Box::pin(async move {
            if parents.is_empty() {
                return Ok(());
            }

            let keys: Vec<Key> = parents.iter().map(self.get).collect();
            let mut children = fetch_children(
                exec,
                self.child_key,
                &self.filter,
                &self.order,
                self.limit,
                keys,
            )
            .await?;
            self.nested.load(exec, &mut children).await?;

            // Only the first child per key can be attached, so keep just that
            // one rather than a `Vec` that is always length 1 in practice.
            let mut map: HashMap<Key, C> = HashMap::new();
            for child in children {
                // First, not last: rows arrive in the relation's `ORDER BY`, so
                // the first match is the one the ordering selected.
                map.entry((self.child_key_get)(&child)).or_insert(child);
            }

            for parent in parents.iter_mut() {
                let key = (self.get)(parent);
                let related = Related::Loaded(map.get(&key).cloned());
                (self.set)(parent, related);
            }

            Ok(())
        })
    }
}
