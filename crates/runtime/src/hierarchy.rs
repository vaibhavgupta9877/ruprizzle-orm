//! Tree hierarchy queries and data structures powered by recursive CTEs.
//!
//! Provides [`HierarchyQuery`] for `.ancestors()` and `.descendants()` queries,
//! and [`HierarchyNode`] for in-memory nested tree reconstruction.

use std::marker::PhantomData;

use serde::{Deserialize, Serialize};

use crate::compile::CompiledSql;
use crate::error::Error;
use crate::executor::{Executor, decode_rows};
use crate::model::Model;
use crate::pool::Pool;
use crate::value::{Encodable, Value};

/// The direction of hierarchy traversal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HierarchyDirection {
    /// Walk up the hierarchy from child to root (`c.id = h.parent_id`).
    Ancestors,
    /// Walk down the hierarchy from parent to descendants (`c.parent_id = h.id`).
    Descendants,
}

/// A node in an in-memory reconstructed tree hierarchy.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HierarchyNode<M> {
    /// The entity model at this node.
    pub item: M,
    /// The distance from the queried root node (root is depth 0).
    pub depth: usize,
    /// Children of this node in the hierarchy.
    pub children: Vec<HierarchyNode<M>>,
}

impl<M> HierarchyNode<M> {
    /// Creates a new hierarchy node.
    pub const fn new(item: M, depth: usize) -> Self {
        Self {
            item,
            depth,
            children: Vec::new(),
        }
    }

    /// Total number of nodes in this subtree (including self).
    pub fn count(&self) -> usize {
        1 + self.children.iter().map(Self::count).sum::<usize>()
    }

    /// Maximum depth of any descendant relative to this node (self is 0).
    pub fn max_subtree_depth(&self) -> usize {
        self.children
            .iter()
            .map(|c| 1 + c.max_subtree_depth())
            .max()
            .unwrap_or(0)
    }

    /// Flattens this tree into a `Vec` of references in pre-order traversal.
    pub fn flatten(&self) -> Vec<&M> {
        let mut out = Vec::new();
        self.collect_flat(&mut out);
        out
    }

    fn collect_flat<'a>(&'a self, out: &mut Vec<&'a M>) {
        out.push(&self.item);
        for child in &self.children {
            child.collect_flat(out);
        }
    }

    /// Reconstructs a full nested `HierarchyNode<M>` from a root item and flat descendants.
    pub fn from_flat(
        root: M,
        descendants: Vec<M>,
        get_id: fn(&M) -> Value,
        get_parent_id: fn(&M) -> Option<Value>,
    ) -> Self {
        let mut by_parent: Vec<(Value, Vec<M>)> = Vec::new();
        for d in descendants {
            if let Some(pid) = get_parent_id(&d) {
                if let Some((_, list)) = by_parent.iter_mut().find(|(k, _)| *k == pid) {
                    list.push(d);
                } else {
                    by_parent.push((pid, vec![d]));
                }
            }
        }

        Self::build_node(root, 0, &mut by_parent, get_id)
    }

    fn build_node(
        item: M,
        depth: usize,
        by_parent: &mut Vec<(Value, Vec<M>)>,
        get_id: fn(&M) -> Value,
    ) -> Self {
        let id = get_id(&item);
        let raw_children = if let Some(idx) = by_parent.iter().position(|(k, _)| *k == id) {
            by_parent.swap_remove(idx).1
        } else {
            Vec::new()
        };
        let children = raw_children
            .into_iter()
            .map(|child| Self::build_node(child, depth + 1, by_parent, get_id))
            .collect();

        HierarchyNode {
            item,
            depth,
            children,
        }
    }
}

/// Depth limit applied while cycle protection is on and no `max_depth` is set.
const DEFAULT_DEPTH_CAP: usize = 100;

/// SQL fragments that carry the visited keys through the recursive CTE: the
/// anchor's path column, the step's extended path, and the step's condition
/// that the next node is not already on the path.
fn cycle_guard(dialect: &str, q_pk: &str) -> (String, String, String) {
    match dialect {
        // Text rather than an array: a model without `COLUMNS` selects `*` from
        // the CTE, and the sqlx `Any` driver cannot decode a Postgres array.
        "postgres" => (
            format!(", ',' || CAST({q_pk} AS TEXT) || ',' AS __path"),
            format!(", h.__path || CAST(c.{q_pk} AS TEXT) || ',' AS __path"),
            format!(" AND strpos(h.__path, ',' || CAST(c.{q_pk} AS TEXT) || ',') = 0"),
        ),
        // MySQL types a recursive CTE column from the anchor alone, so the path
        // is widened there or a deep path would be truncated.
        "mysql" => (
            format!(", CAST(CONCAT(',', {q_pk}, ',') AS CHAR(16383)) AS __path"),
            format!(", CONCAT(h.__path, c.{q_pk}, ',') AS __path"),
            format!(" AND LOCATE(CONCAT(',', c.{q_pk}, ','), h.__path) = 0"),
        ),
        _ => (
            format!(", ',' || {q_pk} || ',' AS __path"),
            format!(", h.__path || c.{q_pk} || ',' AS __path"),
            format!(" AND instr(h.__path, ',' || c.{q_pk} || ',') = 0"),
        ),
    }
}

/// A query builder for recursive hierarchy queries (`.ancestors()` and `.descendants()`).
pub struct HierarchyQuery<'db, M: Model> {
    pool: &'db Pool,
    table: &'static str,
    pk_col: &'static str,
    parent_fk_col: &'static str,
    start_id: Value,
    direction: HierarchyDirection,
    max_depth: Option<usize>,
    order_by_depth: Option<ruprizzle_core::ir::SortOrder>,
    cycle_protection: bool,
    with_deleted: bool,
    _marker: PhantomData<fn() -> M>,
}

impl<'db, M: Model> HierarchyQuery<'db, M> {
    /// Creates a new hierarchy query.
    pub const fn new(
        pool: &'db Pool,
        table: &'static str,
        pk_col: &'static str,
        parent_fk_col: &'static str,
        start_id: Value,
        direction: HierarchyDirection,
    ) -> Self {
        Self {
            pool,
            table,
            pk_col,
            parent_fk_col,
            start_id,
            direction,
            max_depth: None,
            order_by_depth: None,
            cycle_protection: true,
            with_deleted: false,
            _marker: PhantomData,
        }
    }

    /// Creates an ancestor hierarchy query (traversing up to the root).
    pub fn ancestors(
        pool: &'db Pool,
        table: &'static str,
        pk_col: &'static str,
        parent_fk_col: &'static str,
        start_id: impl Encodable,
    ) -> Self {
        Self::new(
            pool,
            table,
            pk_col,
            parent_fk_col,
            start_id.to_value(),
            HierarchyDirection::Ancestors,
        )
    }

    /// Creates a descendant hierarchy query (traversing down through all subtrees).
    pub fn descendants(
        pool: &'db Pool,
        table: &'static str,
        pk_col: &'static str,
        parent_fk_col: &'static str,
        start_id: impl Encodable,
    ) -> Self {
        Self::new(
            pool,
            table,
            pk_col,
            parent_fk_col,
            start_id.to_value(),
            HierarchyDirection::Descendants,
        )
    }

    /// Sets the maximum recursive depth to search.
    #[must_use]
    pub const fn max_depth(mut self, depth: usize) -> Self {
        self.max_depth = Some(depth);
        self
    }

    /// Orders results by depth ascending (closest to start node first).
    #[must_use]
    pub const fn order_by_depth_asc(mut self) -> Self {
        self.order_by_depth = Some(ruprizzle_core::ir::SortOrder::Asc);
        self
    }

    /// Orders results by depth descending (furthest from start node first).
    #[must_use]
    pub const fn order_by_depth_desc(mut self) -> Self {
        self.order_by_depth = Some(ruprizzle_core::ir::SortOrder::Desc);
        self
    }

    /// Configures cycle and infinite loop protection (default: true).
    ///
    /// When on, the query tracks the keys on each path and never steps onto a
    /// node already on it, so cyclic parent links return each node once. It
    /// also caps the depth at 100 levels unless [`max_depth`](Self::max_depth)
    /// is set. The path is a comma-separated string, so a text key that itself
    /// contains a comma can be mistaken for a visited node.
    ///
    /// When off, there is neither a path check nor a depth cap: on cyclic data
    /// Postgres and SQLite recurse until the statement is cancelled, and MySQL
    /// fails at `cte_max_recursion_depth`. Turn it off only for data known to be
    /// acyclic, or together with `max_depth`.
    #[must_use]
    pub const fn cycle_protection(mut self, enabled: bool) -> Self {
        self.cycle_protection = enabled;
        self
    }

    /// Includes soft-deleted nodes, and the subtrees reached through them.
    ///
    /// By default a soft-deleted node is excluded and so is everything reached
    /// only through it. This removes the `deleted_at IS NULL` predicate from
    /// both halves of the recursive CTE. It has no effect on models without a
    /// soft-delete column.
    #[must_use]
    pub const fn with_deleted(mut self) -> Self {
        self.with_deleted = true;
        self
    }

    /// Compiles this hierarchy query to native SQL and binds.
    #[must_use]
    pub fn to_sql(&self) -> CompiledSql {
        let dialect = crate::dialect_for_pool(self.pool);
        let q_table = dialect.quote_ident(self.table);
        let q_pk = dialect.quote_ident(self.pk_col);
        let q_parent_fk = dialect.quote_ident(self.parent_fk_col);

        let cols: Vec<String> = if M::COLUMNS.is_empty() {
            vec!["*".to_owned()]
        } else {
            M::COLUMNS.iter().map(|c| dialect.quote_ident(c)).collect()
        };
        let cols_str = cols.join(", ");
        let c_cols_str: String = if M::COLUMNS.is_empty() {
            "c.*".to_owned()
        } else {
            cols.iter()
                .map(|c| format!("c.{c}"))
                .collect::<Vec<_>>()
                .join(", ")
        };

        let placeholder = dialect.placeholder(0);

        let join_cond = match self.direction {
            HierarchyDirection::Descendants => format!("c.{q_parent_fk} = h.{q_pk}"),
            HierarchyDirection::Ancestors => format!("c.{q_pk} = h.{q_parent_fk}"),
        };

        let depth_cond = match (self.max_depth, self.cycle_protection) {
            (Some(max), _) => format!("h.__depth < {max}"),
            (None, true) => format!("h.__depth < {DEFAULT_DEPTH_CAP}"),
            (None, false) => "1 = 1".to_owned(),
        };

        // Cycle protection carries the keys already on the path and refuses to
        // step onto one of them, so `A -> B -> A` yields A and B once each. A
        // depth cap alone would stop the recursion but emit every node of the
        // cycle once per lap.
        let (anchor_path, step_path, not_visited) = if self.cycle_protection {
            cycle_guard(dialect.name(), &q_pk)
        } else {
            (String::new(), String::new(), String::new())
        };

        // Soft-deleted nodes are excluded, and so is everything reached only
        // through one: a deleted folder hides its subtree, just as a normal
        // query hides the folder. The predicate goes on both halves of the CTE,
        // because filtering the final SELECT would drop the node but still walk
        // past it.
        let (anchor_live, step_live) = match M::DELETED_AT_COLUMN {
            Some(col) if !self.with_deleted => {
                let q_col = dialect.quote_ident(col);
                (
                    format!(" AND {q_col} IS NULL"),
                    format!(" AND c.{q_col} IS NULL"),
                )
            }
            _ => (String::new(), String::new()),
        };

        let order_clause = match self.order_by_depth {
            Some(ruprizzle_core::ir::SortOrder::Asc) => " ORDER BY __depth ASC",
            Some(ruprizzle_core::ir::SortOrder::Desc) => " ORDER BY __depth DESC",
            None => "",
        };

        let sql = format!(
            "WITH RECURSIVE __hierarchy AS (\n  \
               SELECT {cols_str}, 0 AS __depth{anchor_path} FROM {q_table} WHERE {q_pk} = {placeholder}{anchor_live}\n  \
               UNION ALL\n  \
               SELECT {c_cols_str}, h.__depth + 1 AS __depth{step_path} FROM {q_table} c \
               JOIN __hierarchy h ON {join_cond} WHERE {depth_cond}{step_live}{not_visited}\n\
             )\n\
             SELECT {cols_str} FROM __hierarchy{order_clause}"
        );

        CompiledSql {
            sql: sql.into(),
            binds: vec![self.start_id.clone()],
        }
    }

    /// Executes the query and returns all matching models.
    pub async fn all(self) -> Result<Vec<M>, Error>
    where
        M: Unpin,
    {
        let compiled = self.to_sql();
        let batch = self
            .pool
            .fetch_all_raw(compiled.sql, compiled.binds)
            .await?;
        decode_rows::<M>(batch)
    }
}
