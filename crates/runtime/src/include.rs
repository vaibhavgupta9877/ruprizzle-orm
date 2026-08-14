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
    keys: &[Key],
    full_table: bool,
) -> Result<Vec<C>, Error>
where
    C: Model + Send + Unpin,
    Key: Encodable + Eq + Hash + Clone + Send + Sync + 'static,
{
    // Fast path: if the parent set is the whole parent table and the child
    // include has no extra filter, order or per-parent limit, we can avoid
    // parsing and binding a large `IN` list by loading the whole child table.
    // To prevent unbounded materialisation when the child table is huge, we
    // first COUNT(*) it and fall back to chunked `IN` above the executor's
    // limit. `0` disables the fast path entirely.
    let full_table_limit = exec.full_table_include_limit();
    if full_table
        && full_table_limit > 0
        && filter.node == crate::filter::FilterNode::And(Vec::new())
        && order.is_empty()
        && limit.is_none()
    {
        let child_count = SelectQuery::<C>::new(exec).count().await?;
        if child_count <= full_table_limit as i64 {
            return SelectQuery::<C>::new(exec).fetch_all().await;
        }
    }

    // The parent set repeats keys whenever several parents point at the same
    // child (every many-to-one relation does). Sending the duplicates would
    // inflate the `IN` list and burn the parameter budget for nothing. The
    // dedup set borrows the keys so we avoid cloning every parent key just to
    // spot duplicates.
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
            let combined = filter
                .clone()
                .and(child_key.in_set(chunk.iter().copied().cloned().collect::<Vec<_>>()));
            let compiled = select_partitioned::<C>(
                dialect,
                C::TABLE,
                child_key.column,
                &combined.node,
                order,
                n,
            );
            let batch = exec.fetch_all_raw(compiled.sql, compiled.binds).await?;
            all.extend(crate::executor::decode_rows::<C>(batch)?);
        }
        return Ok(all);
    }

    let mut all = Vec::new();
    for chunk in keys.chunks(chunk_size) {
        let mut q = SelectQuery::<C>::new(exec)
            .filter(child_key.in_set(chunk.iter().copied().cloned().collect::<Vec<_>>()));
        if filter.node != crate::filter::FilterNode::And(Vec::new()) {
            q = q.filter(filter.clone());
        }
        for o in order {
            q = q.order_by(o.clone());
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
    keys: &[&Key],
) -> Result<Vec<C>, Error>
where
    C: Model + Send + Unpin,
    Key: Encodable + Clone + Send + Sync + 'static,
{
    let mut all = Vec::new();
    for key in keys {
        let mut q = SelectQuery::<C>::new(exec).filter(child_key.eq((*key).clone()));
        if filter.node != crate::filter::FilterNode::And(Vec::new()) {
            q = q.filter(filter.clone());
        }
        for o in order {
            q = q.order_by(o.clone());
        }
        all.extend(q.limit(limit).fetch_all().await?);
    }
    Ok(all)
}

/// Removes duplicates while keeping first-seen order.
///
/// Borrows from `keys` so the deduplication set does not clone every key; the
/// caller clones only the keys it actually uses in `IN`/`=` binds.
fn dedup<Key: Eq + Hash>(keys: &[Key]) -> Vec<&Key> {
    let mut seen = HashSet::with_capacity(keys.len());
    keys.iter().filter(|k| seen.insert(*k)).collect()
}

/// A set of includes to attach to a parent model.
///
/// This is a type-level "list": a single relation implements it, and nested
/// relations are chained through the relation builder's own `.include()` method.
pub trait IncludeSet<M: Model> {
    /// Loads the related data and attaches it to `parents` in place.
    ///
    /// `full_table` is `true` when the parent query is known to load every row
    /// of the parent table with no filter, limit, offset or distinct. Loaders
    /// can use this to fetch the whole child table instead of building an `IN`
    /// list of parent keys.
    fn load<'a>(
        &'a self,
        exec: &'a dyn Executor,
        parents: &'a mut [M],
        full_table: bool,
    ) -> BoxFuture<'a, Result<(), Error>>;
}

impl<M: Model> IncludeSet<M> for () {
    fn load<'a>(
        &'a self,
        _exec: &'a dyn Executor,
        _parents: &'a mut [M],
        _full_table: bool,
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
    M: Model + Send + Unpin,
    // `Clone` because a one-to-many relation is one-to-many: when several parents
    // share a join key, the same child row must appear in each parent's `Vec`.
    C: Model + Clone + Send + Unpin,
    Key: Encodable + Eq + Hash + Clone + Send + Sync + 'static,
    NI: IncludeSet<C> + Sync,
{
    fn load<'a>(
        &'a self,
        exec: &'a dyn Executor,
        parents: &'a mut [M],
        full_table: bool,
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
                &keys,
                full_table,
            )
            .await?;

            // Propagate the full-table hint to nested includes if this child
            // query was itself an unconstrained full-table fetch.
            let child_full_table = full_table
                && self.filter.node == crate::filter::FilterNode::And(Vec::new())
                && self.order.is_empty()
                && self.limit.is_none();
            self.nested
                .load(exec, &mut children, child_full_table)
                .await?;

            // Group children into pre-sized buckets indexed by parent position.
            // A parent key can map to more than one parent when the join key is not
            // unique, so the index keeps a list of parent positions and a child is
            // cloned into every matching bucket. The single-parent case avoids a
            // clone.
            let bucket_hint = children.len() / parents.len();
            let mut parent_index: HashMap<Key, Vec<usize>> = HashMap::with_capacity(parents.len());
            for (i, parent) in parents.iter().enumerate() {
                parent_index.entry((self.get)(parent)).or_default().push(i);
            }

            let mut buckets: Vec<Vec<C>> =
                std::iter::repeat_with(|| Vec::with_capacity(bucket_hint))
                    .take(parents.len())
                    .collect();

            for child in children {
                if let Some(indices) = parent_index.get(&(self.child_key_get)(&child)) {
                    let mut iter = indices.iter();
                    if let Some(&first) = iter.next() {
                        // Clone only for additional parents; the first gets the
                        // original child.
                        for &idx in iter {
                            if let Some(bucket) = buckets.get_mut(idx) {
                                bucket.push(child.clone());
                            }
                        }
                        if let Some(bucket) = buckets.get_mut(first) {
                            bucket.push(child);
                        }
                    }
                }
            }

            for (parent, bucket) in parents.iter_mut().zip(buckets) {
                (self.set)(parent, Related::Loaded(bucket));
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

/// Include a many-to-many relation that traverses an explicit join model.
///
/// Loads the join rows and the target rows in two batched queries, then
/// distributes target rows back to each parent by matching the join keys.
pub struct IncludeMany<'db, M, C, J, Key, CKey, NI = ()> {
    /// Extract the parent key from the parent row.
    pub get: fn(&M) -> Key,
    /// Attach the loaded children to the parent.
    pub set: fn(&mut M, Related<Vec<C>>),
    /// Extract the parent key from a join row.
    pub join_owner_get: fn(&J) -> Key,
    /// Extract the target key from a join row.
    pub join_target_get: fn(&J) -> CKey,
    /// Extract the target key from a child row.
    pub child_key_get: fn(&C) -> CKey,
    /// The join column that stores the parent key.
    pub join_owner_col: Column<J, Key>,
    /// The join column that stores the target key.
    pub join_target_col: Column<J, CKey>,
    /// The target primary key column.
    pub target_pk: Column<C, CKey>,
    /// Optional extra filter on the target rows.
    pub filter: Filter<C>,
    /// Ordering for the target rows.
    pub order: Vec<OrderBy<C>>,
    /// Optional per-parent limit (`take`).
    pub limit: Option<u64>,
    /// Nested includes on the target.
    pub nested: NI,
    _marker: PhantomData<fn() -> &'db ()>,
}

impl<'db, M, C, J, Key, CKey, NI> Clone for IncludeMany<'db, M, C, J, Key, CKey, NI>
where
    NI: Clone,
{
    fn clone(&self) -> Self {
        Self {
            get: self.get,
            set: self.set,
            join_owner_get: self.join_owner_get,
            join_target_get: self.join_target_get,
            child_key_get: self.child_key_get,
            join_owner_col: self.join_owner_col,
            join_target_col: self.join_target_col,
            target_pk: self.target_pk,
            filter: self.filter.clone(),
            order: self.order.clone(),
            limit: self.limit,
            nested: self.nested.clone(),
            _marker: PhantomData,
        }
    }
}

impl<'db, M, C, J, Key, CKey> IncludeMany<'db, M, C, J, Key, CKey, ()>
where
    M: Model,
    C: Model,
    J: Model,
{
    /// Creates a new many-to-many include.
    #[allow(clippy::too_many_arguments)]
    pub const fn new(
        get: fn(&M) -> Key,
        set: fn(&mut M, Related<Vec<C>>),
        join_owner_get: fn(&J) -> Key,
        join_target_get: fn(&J) -> CKey,
        child_key_get: fn(&C) -> CKey,
        join_owner_col: Column<J, Key>,
        join_target_col: Column<J, CKey>,
        target_pk: Column<C, CKey>,
    ) -> Self {
        Self {
            get,
            set,
            join_owner_get,
            join_target_get,
            child_key_get,
            join_owner_col,
            join_target_col,
            target_pk,
            filter: Filter::new(crate::filter::FilterNode::And(Vec::new())),
            order: Vec::new(),
            limit: None,
            nested: (),
            _marker: PhantomData,
        }
    }
}

impl<'db, M, C, J, Key, CKey, NI> IncludeMany<'db, M, C, J, Key, CKey, NI>
where
    M: Model,
    C: Model,
    J: Model,
{
    /// Adds an extra filter on the target rows.
    pub fn filter(mut self, f: Filter<C>) -> Self {
        self.filter = self.filter.and(f);
        self
    }

    /// Adds a target ordering.
    pub fn order_by(mut self, o: OrderBy<C>) -> Self {
        self.order.push(o);
        self
    }

    /// Limits the number of target rows per parent.
    pub fn take(mut self, n: u64) -> Self {
        self.limit = Some(n);
        self
    }

    /// Adds a nested include on the target.
    pub fn include<Next: IncludeSet<C>>(self, include: Next) -> IncludeMany<'db, M, C, J, Key, CKey, Next> {
        IncludeMany {
            get: self.get,
            set: self.set,
            join_owner_get: self.join_owner_get,
            join_target_get: self.join_target_get,
            child_key_get: self.child_key_get,
            join_owner_col: self.join_owner_col,
            join_target_col: self.join_target_col,
            target_pk: self.target_pk,
            filter: self.filter,
            order: self.order,
            limit: self.limit,
            nested: include,
            _marker: PhantomData,
        }
    }
}

impl<'db, M, C, J, Key, CKey, NI> IncludeSet<M> for IncludeMany<'db, M, C, J, Key, CKey, NI>
where
    M: Model + Send + Unpin,
    C: Model + Clone + Send + Unpin,
    J: Model + Send + Unpin,
    Key: Encodable + Eq + Hash + Clone + Send + Sync + 'static,
    CKey: Encodable + Eq + Hash + Clone + Send + Sync + 'static,
    NI: IncludeSet<C> + Sync,
{
    fn load<'a>(
        &'a self,
        exec: &'a dyn Executor,
        parents: &'a mut [M],
        _full_table: bool,
    ) -> BoxFuture<'a, Result<(), Error>> {
        Box::pin(async move {
            if parents.is_empty() {
                return Ok(());
            }

            let keys: Vec<Key> = parents.iter().map(self.get).collect();
            let deduped = dedup(&keys);
            let caps = exec.dialect().capabilities();
            let chunk_size = (caps.max_query_params as usize).saturating_sub(10).max(1);

            // 1. Load the join rows for all parents in chunked `IN` queries.
            let mut join_rows: Vec<J> = Vec::new();
            for chunk in deduped.chunks(chunk_size) {
                let values: Vec<Key> = chunk.iter().copied().cloned().collect();
                let q = SelectQuery::<J>::new(exec)
                    .filter(self.join_owner_col.in_set(values))
                    .order_by(self.join_owner_col.asc())
                    .order_by(self.join_target_col.asc());
                join_rows.extend(q.fetch_all().await?);
            }

            if join_rows.is_empty() {
                for parent in parents.iter_mut() {
                    (self.set)(parent, Related::Loaded(Vec::new()));
                }
                return Ok(());
            }

            // 2. Collect the unique target keys referenced by the join rows.
            let child_keys: Vec<CKey> = join_rows
                .iter()
                .map(|j| (self.join_target_get)(j))
                .collect();
            let child_keys = dedup(&child_keys)
                .into_iter()
                .cloned()
                .collect::<Vec<_>>();

            // 3. Load the target rows in one (possibly chunked) query,
            //    applying the user filter and order.
            let mut children: Vec<C> = Vec::new();
            for chunk in child_keys.chunks(chunk_size) {
                let values: Vec<CKey> = chunk.to_vec();
                let mut q = SelectQuery::<C>::new(exec).filter(self.target_pk.in_set(values));
                if self.filter.node != crate::filter::FilterNode::And(Vec::new()) {
                    q = q.filter(self.filter.clone());
                }
                for o in &self.order {
                    q = q.order_by(o.clone());
                }
                children.extend(q.fetch_all().await?);
            }

            // 4. Recursively load nested includes on the target rows.
            self.nested.load(exec, &mut children, false).await?;

            // 5. Build a parent key -> [parent index] map.
            let mut parent_index: HashMap<Key, Vec<usize>> = HashMap::with_capacity(parents.len());
            for (i, parent) in parents.iter().enumerate() {
                parent_index.entry((self.get)(parent)).or_default().push(i);
            }

            // 6. Map each target key to the parent indices that reference it.
            let mut child_to_parents: HashMap<CKey, Vec<usize>> =
                HashMap::with_capacity(join_rows.len());
            for j in &join_rows {
                let owner = (self.join_owner_get)(j);
                if let Some(indices) = parent_index.get(&owner) {
                    let target = (self.join_target_get)(j);
                    child_to_parents
                        .entry(target)
                        .or_default()
                        .extend(indices.iter().copied());
                }
            }

            // 7. Walk the target rows in the requested order and distribute
            //    them to the matching parent buckets.
            let mut buckets: Vec<Vec<C>> =
                std::iter::repeat_with(Vec::new).take(parents.len()).collect();
            for child in children {
                let target_key = (self.child_key_get)(&child);
                if let Some(parents) = child_to_parents.get(&target_key) {
                    for &idx in parents {
                        buckets[idx].push(child.clone());
                    }
                }
            }

            // 8. Apply per-parent take and attach.
            for (parent, mut bucket) in parents.iter_mut().zip(buckets) {
                if let Some(n) = self.limit {
                    let n = n as usize;
                    if bucket.len() > n {
                        bucket.truncate(n);
                    }
                }
                (self.set)(parent, Related::Loaded(bucket));
            }

            Ok(())
        })
    }
}

impl<'db, M, C, Key, NI> IncludeSet<M> for IncludeOne<'db, M, C, Key, NI>
where
    M: Model + Send + Unpin,
    C: Model + Clone + Send + Unpin,
    Key: Encodable + Eq + Hash + Clone + Send + Sync + 'static,
    NI: IncludeSet<C> + Sync,
{
    fn load<'a>(
        &'a self,
        exec: &'a dyn Executor,
        parents: &'a mut [M],
        full_table: bool,
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
                &keys,
                full_table,
            )
            .await?;

            let child_full_table = full_table
                && self.filter.node == crate::filter::FilterNode::And(Vec::new())
                && self.order.is_empty()
                && self.limit.is_none();
            self.nested
                .load(exec, &mut children, child_full_table)
                .await?;

            let mut map: HashMap<Key, C> = HashMap::with_capacity(parents.len());
            for child in children {
                let key = (self.child_key_get)(&child);
                if let std::collections::hash_map::Entry::Vacant(e) = map.entry(key) {
                    e.insert(child);
                }
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
