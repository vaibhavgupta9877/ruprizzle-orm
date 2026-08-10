//! Batched relation `include` loading.

use std::collections::HashMap;
use std::fmt;
use std::hash::Hash;
use std::marker::PhantomData;

use crate::BoxFuture;
use crate::col::Column;
use crate::compile::dialect_for_pool;
use crate::error::Error;
use crate::filter::Filter;
use crate::model::Model;
use crate::order::OrderBy;
use crate::pool::Pool;
use crate::query::SelectQuery;
use crate::related::Related;
use crate::value::Encodable;

async fn fetch_children<C, Key>(
    pool: &Pool,
    child_key: Column<C, Key>,
    filter: &Filter<C>,
    order: &[OrderBy<C>],
    limit: Option<u64>,
    keys: Vec<Key>,
) -> Result<Vec<C>, Error>
where
    C: Model + Send + Unpin + for<'r> sqlx::FromRow<'r, sqlx::any::AnyRow>,
    Key: Encodable + Clone + Send + Sync + 'static,
{
    if keys.is_empty() {
        return Ok(Vec::new());
    }

    let cap = dialect_for_pool(pool).capabilities().max_query_params as usize;
    let chunk_size = cap.saturating_sub(10).max(1);

    let should_chunk = limit.is_none() && keys.len() > chunk_size;

    if !should_chunk {
        let mut q = SelectQuery::<C>::new(pool).filter(child_key.in_set(keys));
        if filter.node != crate::filter::FilterNode::And(Vec::new()) {
            q = q.filter(filter.clone());
        }
        for o in order {
            q = q.order_by(*o);
        }
        if let Some(n) = limit {
            q = q.limit(n);
        }
        q.fetch_all().await
    } else {
        let mut all = Vec::new();
        for chunk in keys.chunks(chunk_size) {
            let chunk = chunk.to_vec();
            let mut q = SelectQuery::<C>::new(pool).filter(child_key.in_set(chunk));
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
}

/// A set of includes to attach to a parent model.
///
/// This is a type-level "list": a single relation implements it, and nested
/// relations are chained through the relation builder's own `.include()` method.
pub trait IncludeSet<M: Model> {
    /// Loads the related data and attaches it to `parents` in place.
    fn load<'a>(&'a self, pool: &'a Pool, parents: &'a mut [M])
    -> BoxFuture<'a, Result<(), Error>>;
}

impl<M: Model> IncludeSet<M> for () {
    fn load<'a>(
        &'a self,
        _pool: &'a Pool,
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
        pool: &'a Pool,
        parents: &'a mut [M],
    ) -> BoxFuture<'a, Result<(), Error>> {
        Box::pin(async move {
            if parents.is_empty() {
                return Ok(());
            }

            let keys: Vec<Key> = parents.iter().map(self.get).collect();
            let mut children = fetch_children(
                pool,
                self.child_key,
                &self.filter,
                &self.order,
                self.limit,
                keys,
            )
            .await?;
            self.nested.load(pool, &mut children).await?;

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
    C: Model + Send + Unpin + for<'r> sqlx::FromRow<'r, sqlx::any::AnyRow>,
    Key: Encodable + Eq + Hash + Clone + Send + Sync + 'static,
    NI: IncludeSet<C> + Sync,
{
    fn load<'a>(
        &'a self,
        pool: &'a Pool,
        parents: &'a mut [M],
    ) -> BoxFuture<'a, Result<(), Error>> {
        Box::pin(async move {
            if parents.is_empty() {
                return Ok(());
            }

            let keys: Vec<Key> = parents.iter().map(self.get).collect();
            let mut children = fetch_children(
                pool,
                self.child_key,
                &self.filter,
                &self.order,
                self.limit,
                keys,
            )
            .await?;
            self.nested.load(pool, &mut children).await?;

            let mut map: HashMap<Key, Vec<C>> = HashMap::new();
            for child in children {
                let key = (self.child_key_get)(&child);
                map.entry(key).or_default().push(child);
            }

            for parent in parents.iter_mut() {
                let key = (self.get)(parent);
                let related = map
                    .remove(&key)
                    .and_then(|mut v| v.pop())
                    .map(|c| Related::Loaded(Some(c)))
                    .unwrap_or(Related::Loaded(None));
                (self.set)(parent, related);
            }

            Ok(())
        })
    }
}
