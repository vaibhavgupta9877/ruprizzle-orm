//! SQL compiler: turns `Filter`/`Order`/`Value` trees into parameterised SQL.
//!
//! Every runtime value is pushed as a placeholder; there is no string
//! interpolation of user data. The compiler is dialect-aware so it can quote
//! identifiers and produce the correct parameter markers.

use std::borrow::Cow;
use std::fmt::Write as _;

use ruprizzle_dialect::{DbDialect, dialect_for};

use crate::aggregate::{AggregateEntry, AggregateKind};
use crate::filter::{CmpOp, Cte, FilterNode};
use crate::join::JoinKind;
use crate::model::Model;
use crate::order::OrderBy;
use crate::query::SetOp;
use crate::value::Value;

/// A compiled SQL statement and its bound values.
#[derive(Debug, Clone, PartialEq)]
pub struct CompiledSql {
    /// The SQL string with placeholders.
    pub sql: Cow<'static, str>,
    /// The values bound to the placeholders, in order.
    pub binds: Vec<Value>,
}

impl CompiledSql {
    /// Returns a copy of this SQL with every `$n` placeholder shifted by `offset`.
    ///
    /// This is used when a compiled subquery is embedded in a larger statement
    /// and its placeholders must continue the outer statement's numbering.
    /// `?`-style placeholders are left unchanged because they are not numbered.
    #[must_use]
    pub fn renumbered(&self, offset: usize) -> Self {
        if offset == 0 || self.binds.is_empty() || !self.sql.contains('$') {
            return self.clone();
        }
        let mut sql = String::with_capacity(self.sql.len());
        let bytes = self.sql.as_bytes();
        let mut i = 0;
        while i < bytes.len() {
            if bytes[i] == b'$' {
                let mut j = i + 1;
                while j < bytes.len() && bytes[j].is_ascii_digit() {
                    j += 1;
                }
                if j > i + 1 {
                    let n = std::str::from_utf8(&bytes[i + 1..j])
                        .unwrap()
                        .parse::<usize>()
                        .unwrap();
                    let _ = write!(sql, "${}", n + offset);
                    i = j;
                    continue;
                }
            }
            sql.push(bytes[i] as char);
            i += 1;
        }
        Self {
            sql: Cow::Owned(sql),
            binds: self.binds.clone(),
        }
    }
}

/// Compile a `SELECT` for `M`.
#[must_use]
#[allow(clippy::too_many_arguments)]
pub fn select<M: Model>(
    dialect: &dyn DbDialect,
    table: &str,
    projection: &[&str],
    filter: &FilterNode,
    order: &[OrderBy<M>],
    limit: Option<u64>,
    offset: Option<u64>,
    distinct: bool,
) -> CompiledSql {
    let mut c = Compiler::new(dialect);

    c.push_str("SELECT ");
    if distinct {
        c.push_str("DISTINCT ");
    }
    // Fall back to `SELECT *` only when the user did not request an explicit
    // projection and the model does not declare its columns. Generated models
    // always declare `COLUMNS`, so this compiles to an explicit, narrow list
    // and lets `FromRow` decode by ordinal instead of by name.
    let projection = if projection.is_empty() && !M::COLUMNS.is_empty() {
        M::COLUMNS
    } else {
        projection
    };
    if projection.is_empty() {
        c.push('*');
    } else {
        for (i, col) in projection.iter().enumerate() {
            if i > 0 {
                c.push_str(", ");
            }
            c.push_quoted(table);
            c.push('.');
            c.push_quoted(col);
        }
    }

    c.push_str(" FROM ");
    c.push_quoted(table);

    if !matches!(filter, FilterNode::And(v) if v.is_empty()) {
        c.push_str(" WHERE ");
        c.push_filter(filter);
    }

    if !order.is_empty() {
        c.push_str(" ORDER BY ");
        c.push_order(order);
    }

    if let Some(n) = limit {
        c.push_str(" LIMIT ");
        c.push_str(&n.to_string());
    }

    if let Some(n) = offset {
        c.push_str(" OFFSET ");
        c.push_str(&n.to_string());
    }

    c.finish()
}

/// Prepends a `WITH` (or `WITH RECURSIVE`) clause to `main` using `ctes`.
///
/// Placeholders are renumbered so that the CTE binds come first and the main
/// query's placeholders follow. For `?`-style dialects the SQL is concatenated
/// unchanged and the binds are appended in order.
pub(crate) fn with_cte_prefix(
    dialect: &dyn DbDialect,
    ctes: &[Cte],
    main: CompiledSql,
) -> CompiledSql {
    if ctes.is_empty() {
        return main;
    }
    let recursive = ctes.iter().any(|c| c.recursive);
    let mut binds = Vec::with_capacity(
        ctes.iter().map(|c| c.compiled.binds.len()).sum::<usize>() + main.binds.len(),
    );
    let mut parts = Vec::with_capacity(ctes.len());
    for cte in ctes {
        let shifted = cte.compiled.renumbered(binds.len());
        binds.extend(shifted.binds);
        parts.push(format!(
            "{} AS ({})",
            dialect.quote_ident(cte.name),
            shifted.sql.as_ref()
        ));
    }
    let main_shifted = main.renumbered(binds.len());
    binds.extend(main_shifted.binds);
    let keyword = if recursive {
        "WITH RECURSIVE "
    } else {
        "WITH "
    };
    let sql = format!(
        "{}{} {}",
        keyword,
        parts.join(", "),
        main_shifted.sql.as_ref()
    );
    CompiledSql {
        sql: Cow::Owned(sql),
        binds,
    }
}

/// Combines two compiled `SELECT`s with a set operation and prepends CTEs.
///
/// Right-hand placeholders are renumbered so that the combined statement uses
/// one contiguous parameter sequence. For `?`-style dialects the placeholders
/// are left unchanged and the binds are concatenated in order.
#[must_use]
pub fn set_op(
    dialect: &dyn DbDialect,
    op: SetOp,
    ctes: &[Cte],
    left: CompiledSql,
    right: CompiledSql,
) -> CompiledSql {
    let right = right.renumbered(left.binds.len());
    let mut binds = left.binds;
    binds.extend(right.binds);

    // SQLite does not accept a parenthesized SELECT as a top-level compound
    // operand, so wrap each side in a derived table instead.
    let sql = if dialect.name() == "sqlite" {
        format!(
            "SELECT * FROM ({}) AS __rz_l {} SELECT * FROM ({}) AS __rz_r",
            left.sql.as_ref(),
            op.sql(),
            right.sql.as_ref()
        )
    } else {
        format!(
            "({}) {} ({})",
            left.sql.as_ref(),
            op.sql(),
            right.sql.as_ref()
        )
    };
    with_cte_prefix(
        dialect,
        ctes,
        CompiledSql {
            sql: Cow::Owned(sql),
            binds,
        },
    )
}

/// Compile a `SELECT` for a join between `M` and `J`.
///
/// This is the public, typed entry point; the query builder uses
/// [`join_select_with_columns`] so it does not need to keep `J` in the
/// `SelectQuery` type.
#[must_use]
#[allow(clippy::too_many_arguments)]
pub fn join_select<M: Model, J: Model>(
    dialect: &dyn DbDialect,
    left_table: &str,
    right_table: &str,
    right_alias: Option<&str>,
    join_kind: JoinKind,
    on: &FilterNode,
    filter: &FilterNode,
    order: &[OrderBy<M>],
    limit: Option<u64>,
    offset: Option<u64>,
    distinct: bool,
) -> CompiledSql {
    join_select_with_columns(
        dialect,
        left_table,
        M::COLUMNS,
        right_table,
        J::COLUMNS,
        right_alias,
        join_kind,
        on,
        filter,
        order,
        limit,
        offset,
        distinct,
    )
}

/// Compile a `SELECT` for a join with the right-hand columns supplied as a slice.
///
/// This is the internal entry point used by the query builder, which does not
/// have the right-hand model type available at the call site.
#[must_use]
#[allow(clippy::too_many_arguments)]
pub(crate) fn join_select_with_columns<M: Model>(
    dialect: &dyn DbDialect,
    left_table: &str,
    left_columns: &[&str],
    right_table: &str,
    right_columns: &[&str],
    right_alias: Option<&str>,
    join_kind: JoinKind,
    on: &FilterNode,
    filter: &FilterNode,
    order: &[OrderBy<M>],
    limit: Option<u64>,
    offset: Option<u64>,
    distinct: bool,
) -> CompiledSql {
    let mut c = Compiler::new(dialect);

    c.push_str("SELECT ");
    if distinct {
        c.push_str("DISTINCT ");
    }

    let right_qualifier = right_alias.unwrap_or(right_table);

    if left_columns.is_empty() {
        c.push_quoted(left_table);
        c.push_str(".*");
    } else {
        for (i, col) in left_columns.iter().enumerate() {
            if i > 0 {
                c.push_str(", ");
            }
            c.push_quoted(left_table);
            c.push('.');
            c.push_quoted(col);
        }
    }

    if right_columns.is_empty() {
        c.push_str(", ");
        c.push_quoted(right_qualifier);
        c.push_str(".*");
    } else {
        for col in right_columns {
            c.push_str(", ");
            c.push_quoted(right_qualifier);
            c.push('.');
            c.push_quoted(col);
        }
    }

    c.push_str(" FROM ");
    c.push_quoted(left_table);

    c.push(' ');
    c.push_str(join_kind_sql(dialect, join_kind));
    c.push(' ');
    c.push_quoted(right_table);

    if let Some(alias) = right_alias {
        c.push_str(" AS ");
        c.push_quoted(alias);
    }

    c.push_str(" ON ");
    c.push_filter(on);

    if !matches!(filter, FilterNode::And(v) if v.is_empty()) {
        c.push_str(" WHERE ");
        c.push_filter(filter);
    }

    if !order.is_empty() {
        c.push_str(" ORDER BY ");
        c.push_order(order);
    }

    if let Some(n) = limit {
        c.push_str(" LIMIT ");
        c.push_str(&n.to_string());
    }

    if let Some(n) = offset {
        c.push_str(" OFFSET ");
        c.push_str(&n.to_string());
    }

    c.finish()
}

fn join_kind_sql(dialect: &dyn DbDialect, kind: JoinKind) -> &'static str {
    match kind {
        JoinKind::Inner => "INNER JOIN",
        JoinKind::Left => "LEFT JOIN",
        JoinKind::Right => "RIGHT JOIN",
        JoinKind::Full => {
            if dialect.name() == "postgres" {
                "FULL OUTER JOIN"
            } else {
                "FULL JOIN"
            }
        }
    }
}

/// Compile an aggregate `SELECT` for `M`.
///
/// The projection is a list of aggregate expressions; `GROUP BY` and `HAVING`
/// are included when their inputs are non-empty.
#[must_use]
#[allow(clippy::too_many_arguments)]
pub fn aggregate_select<M: Model>(
    dialect: &dyn DbDialect,
    table: &str,
    aggregates: &[AggregateEntry],
    filter: &FilterNode,
    group_by: &[&str],
    having: &FilterNode,
    order: &[OrderBy<M>],
    limit: Option<u64>,
    offset: Option<u64>,
) -> CompiledSql {
    let mut c = Compiler::new(dialect);

    c.push_str("SELECT ");
    for (i, agg) in aggregates.iter().enumerate() {
        if i > 0 {
            c.push_str(", ");
        }
        c.push_aggregate(agg);
    }

    c.push_str(" FROM ");
    c.push_quoted(table);

    if !matches!(filter, FilterNode::And(v) if v.is_empty()) {
        c.push_str(" WHERE ");
        c.push_filter(filter);
    }

    if !group_by.is_empty() {
        c.push_str(" GROUP BY ");
        for (i, col) in group_by.iter().enumerate() {
            if i > 0 {
                c.push_str(", ");
            }
            c.push_quoted(table);
            c.push('.');
            c.push_quoted(col);
        }
    }

    if !matches!(having, FilterNode::And(v) if v.is_empty()) {
        c.push_str(" HAVING ");
        c.push_filter(having);
    }

    if !order.is_empty() {
        c.push_str(" ORDER BY ");
        c.push_order(order);
    }

    if let Some(n) = limit {
        c.push_str(" LIMIT ");
        c.push_str(&n.to_string());
    }

    if let Some(n) = offset {
        c.push_str(" OFFSET ");
        c.push_str(&n.to_string());
    }

    c.finish()
}

/// Compile a `SELECT COUNT(*)` for `M`.
///
/// Skips `ORDER BY`, `LIMIT` and `OFFSET` because an aggregate count over an
/// unordered, unlimited relation is what callers of `SelectQuery::count` want.
#[must_use]
pub fn count<M: Model>(dialect: &dyn DbDialect, table: &str, filter: &FilterNode) -> CompiledSql {
    let mut c = Compiler::new(dialect);

    c.push_str("SELECT COUNT(*) FROM ");
    c.push_quoted(table);

    if !matches!(filter, FilterNode::And(v) if v.is_empty()) {
        c.push_str(" WHERE ");
        c.push_filter(filter);
    }

    c.finish()
}

/// Compile an `EXISTS`-style `SELECT 1 ... LIMIT 1` for `M`.
#[must_use]
pub fn exists<M: Model>(dialect: &dyn DbDialect, table: &str, filter: &FilterNode) -> CompiledSql {
    let mut c = Compiler::new(dialect);

    c.push_str("SELECT 1 FROM ");
    c.push_quoted(table);

    if !matches!(filter, FilterNode::And(v) if v.is_empty()) {
        c.push_str(" WHERE ");
        c.push_filter(filter);
    }

    c.push_str(" LIMIT 1");

    c.finish()
}

/// The alias the partitioned select gives its `ROW_NUMBER()` column.
///
/// It is selected through to the outer query, so it appears in the result set.
/// `FromRow` implementations match by name and ignore the extra column.
pub const ROW_NUMBER_ALIAS: &str = "__rz_rn";

/// Compile a `SELECT` for `M` that keeps only the first `take` rows *per group*.
///
/// This is the per-parent `take` of a relation `include`. A plain `LIMIT` would
/// cap the whole batch rather than each parent's children, which is silently
/// wrong once more than one parent is loaded — hence the window function.
///
/// Requires `capabilities().window_functions`; callers must check first.
#[must_use]
pub fn select_partitioned<M: Model>(
    dialect: &dyn DbDialect,
    table: &str,
    partition_by: &str,
    filter: &FilterNode,
    order: &[OrderBy<M>],
    take: u64,
) -> CompiledSql {
    let mut c = Compiler::new(dialect);

    c.push_str("SELECT * FROM (SELECT ");
    if M::COLUMNS.is_empty() {
        c.push_quoted(table);
        c.push_str(".*");
    } else {
        for (i, col) in M::COLUMNS.iter().enumerate() {
            if i > 0 {
                c.push_str(", ");
            }
            c.push_quoted(table);
            c.push('.');
            c.push_quoted(col);
        }
    }
    c.push_str(", ROW_NUMBER() OVER (PARTITION BY ");
    c.push_quoted(table);
    c.push('.');
    c.push_quoted(partition_by);
    c.push_str(" ORDER BY ");
    if order.is_empty() {
        // A window needs a deterministic order or the rows kept by `take` vary
        // between runs. The primary key is the one column always available.
        c.push_quoted(table);
        c.push('.');
        c.push_quoted(M::PRIMARY_KEY);
        c.push_str(" ASC");
    } else {
        c.push_order(order);
    }
    c.push_str(") AS ");
    c.push_quoted(ROW_NUMBER_ALIAS);
    c.push_str(" FROM ");
    c.push_quoted(table);

    if !matches!(filter, FilterNode::And(v) if v.is_empty()) {
        c.push_str(" WHERE ");
        c.push_filter(filter);
    }

    c.push_str(") AS ");
    c.push_quoted("__rz_partitioned");
    c.push_str(" WHERE ");
    c.push_quoted(ROW_NUMBER_ALIAS);
    c.push_str(" <= ");
    c.push_str(&take.to_string());
    // Restores the per-partition ordering the outer query would otherwise lose.
    c.push_str(" ORDER BY ");
    c.push_quoted(ROW_NUMBER_ALIAS);
    c.push_str(" ASC");

    c.finish()
}

/// Compile an `INSERT` for `M`.
#[must_use]
pub fn insert<M: Model>(
    dialect: &dyn DbDialect,
    table: &str,
    values: &[(&'static str, Value)],
    returning: &[&str],
) -> CompiledSql {
    let mut c = Compiler::new(dialect);

    c.push_str("INSERT INTO ");
    c.push_quoted(table);
    c.push_str(" (");
    for (i, (col, _)) in values.iter().enumerate() {
        if i > 0 {
            c.push_str(", ");
        }
        c.push_quoted(col);
    }
    c.push_str(") VALUES (");
    for (i, (_, val)) in values.iter().enumerate() {
        if i > 0 {
            c.push_str(", ");
        }
        c.push_bind(val.clone());
    }
    c.push(')');

    if dialect.returning_supported() && !returning.is_empty() {
        c.push_str(" RETURNING ");
        for (i, col) in returning.iter().enumerate() {
            if i > 0 {
                c.push_str(", ");
            }
            if *col == "*" {
                c.push('*');
            } else {
                c.push_quoted(col);
            }
        }
    }

    c.finish()
}

/// Compile a multi-row `INSERT` for `M`.
#[must_use]
pub fn insert_many<M: Model>(
    dialect: &dyn DbDialect,
    table: &str,
    rows: &[Vec<(&'static str, Value)>],
    returning: &[&str],
) -> CompiledSql {
    let mut c = Compiler::new(dialect);
    let Some(first) = rows.first() else {
        return c.finish();
    };
    let columns: Vec<_> = first.iter().map(|(col, _)| *col).collect();

    c.push_str("INSERT INTO ");
    c.push_quoted(table);
    c.push_str(" (");
    for (i, col) in columns.iter().enumerate() {
        if i > 0 {
            c.push_str(", ");
        }
        c.push_quoted(col);
    }
    c.push_str(") VALUES ");

    for (r, row) in rows.iter().enumerate() {
        if r > 0 {
            c.push_str(", ");
        }
        c.push('(');
        for (i, (_, val)) in row.iter().enumerate() {
            if i > 0 {
                c.push_str(", ");
            }
            c.push_bind(val.clone());
        }
        c.push(')');
    }

    if dialect.returning_supported() && !returning.is_empty() {
        c.push_str(" RETURNING ");
        for (i, col) in returning.iter().enumerate() {
            if i > 0 {
                c.push_str(", ");
            }
            if *col == "*" {
                c.push('*');
            } else {
                c.push_quoted(col);
            }
        }
    }

    c.finish()
}

/// Compile an `INSERT ... ON CONFLICT` (upsert) for `M`.
#[must_use]
#[allow(clippy::too_many_arguments)]
pub fn upsert<M: Model>(
    dialect: &dyn DbDialect,
    table: &str,
    values: &[(&'static str, Value)],
    conflict: &[&str],
    update: &[&str],
    returning: &[&str],
) -> CompiledSql {
    let mut c = Compiler::new(dialect);

    c.push_str("INSERT INTO ");
    c.push_quoted(table);
    c.push_str(" (");
    for (i, (col, _)) in values.iter().enumerate() {
        if i > 0 {
            c.push_str(", ");
        }
        c.push_quoted(col);
    }
    c.push_str(") VALUES (");
    for (i, (_, val)) in values.iter().enumerate() {
        if i > 0 {
            c.push_str(", ");
        }
        c.push_bind(val.clone());
    }
    c.push_str(") ");

    let conflict = conflict.iter().map(|s| (*s).to_owned()).collect::<Vec<_>>();
    let update = update.iter().map(|s| (*s).to_owned()).collect::<Vec<_>>();
    c.push_str(&dialect.upsert_clause(&conflict, &update));

    if dialect.returning_supported() && !returning.is_empty() {
        c.push_str(" RETURNING ");
        for (i, col) in returning.iter().enumerate() {
            if i > 0 {
                c.push_str(", ");
            }
            if *col == "*" {
                c.push('*');
            } else {
                c.push_quoted(col);
            }
        }
    }

    c.finish()
}

/// Compile an `UPDATE` for `M`.
#[must_use]
pub fn update<M: Model>(
    dialect: &dyn DbDialect,
    table: &str,
    sets: &[(&'static str, Value)],
    filter: &FilterNode,
    returning: &[&str],
) -> CompiledSql {
    let mut c = Compiler::new(dialect);

    c.push_str("UPDATE ");
    c.push_quoted(table);
    c.push_str(" SET ");
    for (i, (col, val)) in sets.iter().enumerate() {
        if i > 0 {
            c.push_str(", ");
        }
        c.push_quoted(col);
        c.push_str(" = ");
        c.push_bind(val.clone());
    }

    if !matches!(filter, FilterNode::And(v) if v.is_empty()) {
        c.push_str(" WHERE ");
        c.push_filter(filter);
    }

    if dialect.returning_supported() && !returning.is_empty() {
        c.push_str(" RETURNING ");
        for (i, col) in returning.iter().enumerate() {
            if i > 0 {
                c.push_str(", ");
            }
            if *col == "*" {
                c.push('*');
            } else {
                c.push_quoted(col);
            }
        }
    }

    c.finish()
}

/// Compile a `DELETE` for `M`.
#[must_use]
pub fn delete<M: Model>(
    dialect: &dyn DbDialect,
    table: &str,
    filter: &FilterNode,
    returning: &[&str],
) -> CompiledSql {
    let mut c = Compiler::new(dialect);

    c.push_str("DELETE FROM ");
    c.push_quoted(table);

    if !matches!(filter, FilterNode::And(v) if v.is_empty()) {
        c.push_str(" WHERE ");
        c.push_filter(filter);
    }

    if dialect.returning_supported() && !returning.is_empty() {
        c.push_str(" RETURNING ");
        for (i, col) in returning.iter().enumerate() {
            if i > 0 {
                c.push_str(", ");
            }
            if *col == "*" {
                c.push('*');
            } else {
                c.push_quoted(col);
            }
        }
    }

    c.finish()
}

/// Build a `&'static dyn DbDialect` from a `Pool`.
pub fn dialect_for_pool(pool: &crate::Pool) -> &'static dyn DbDialect {
    dialect_for(pool.provider())
}

struct Compiler<'d> {
    dialect: &'d dyn DbDialect,
    sql: String,
    binds: Vec<Value>,
}

impl<'d> Compiler<'d> {
    fn new(dialect: &'d dyn DbDialect) -> Self {
        Self {
            dialect,
            sql: String::new(),
            binds: Vec::new(),
        }
    }

    fn finish(self) -> CompiledSql {
        CompiledSql {
            sql: Cow::Owned(self.sql),
            binds: self.binds,
        }
    }

    fn push_str(&mut self, s: &str) {
        self.sql.push_str(s);
    }

    fn push(&mut self, c: char) {
        self.sql.push(c);
    }

    fn push_quoted(&mut self, s: &str) {
        self.sql.push_str(&self.dialect.quote_ident(s));
    }

    fn push_aggregate(&mut self, agg: &AggregateEntry) {
        self.push_str(agg.kind.sql_fn());
        self.push('(');
        if agg.kind == AggregateKind::CountDistinct {
            self.push_str("DISTINCT ");
        }

        // AVG over an integer column returns NUMERIC/DECIMAL on Postgres and
        // MySQL, which sqlx::Any cannot decode as f64. Cast the argument to a
        // floating-point type so the aggregate result matches the Rust type.
        if agg.kind == AggregateKind::Avg {
            match self.dialect.name() {
                "postgres" => {
                    self.push_quoted(agg.table);
                    self.push('.');
                    self.push_quoted(agg.column);
                    self.push_str("::double precision");
                }
                "mysql" => {
                    self.push_str("CAST(");
                    self.push_quoted(agg.table);
                    self.push('.');
                    self.push_quoted(agg.column);
                    self.push_str(" AS DOUBLE)");
                }
                _ => {
                    self.push_str("CAST(");
                    self.push_quoted(agg.table);
                    self.push('.');
                    self.push_quoted(agg.column);
                    self.push_str(" AS REAL)");
                }
            }
        } else {
            self.push_quoted(agg.table);
            self.push('.');
            self.push_quoted(agg.column);
        }

        self.push(')');
        self.push_str(" AS ");
        self.push_quoted(&agg.alias);
    }

    fn push_order<M: Model>(&mut self, order: &[OrderBy<M>]) {
        for (i, o) in order.iter().enumerate() {
            if i > 0 {
                self.push_str(", ");
            }
            self.push_quoted(o.table);
            self.push('.');
            self.push_quoted(o.column);
            if o.desc {
                self.push_str(" DESC");
            } else {
                self.push_str(" ASC");
            }
        }
    }

    fn push_bind(&mut self, value: Value) {
        self.binds.push(value);
        // `placeholder` is 0-indexed and adds one internally.
        let idx = self.binds.len() - 1;
        self.sql.push_str(&self.dialect.placeholder(idx));
    }

    fn push_subquery(&mut self, subquery: &CompiledSql) {
        let sql = subquery.sql.as_ref();
        if self.dialect.placeholder(0).starts_with('?') {
            self.sql.push_str(sql);
            self.binds.extend(subquery.binds.iter().cloned());
            return;
        }

        let offset = self.binds.len();
        let bytes = sql.as_bytes();
        let mut i = 0;
        while i < bytes.len() {
            if bytes[i] == b'$' {
                let mut j = i + 1;
                while j < bytes.len() && bytes[j].is_ascii_digit() {
                    j += 1;
                }
                if j > i + 1 {
                    let n = std::str::from_utf8(&bytes[i + 1..j])
                        .unwrap()
                        .parse::<usize>()
                        .unwrap();
                    self.sql.push_str(&self.dialect.placeholder(offset + n - 1));
                    i = j;
                    continue;
                }
            }
            self.sql.push(bytes[i] as char);
            i += 1;
        }
        self.binds.extend(subquery.binds.iter().cloned());
    }

    fn push_filter(&mut self, node: &FilterNode) {
        match node {
            FilterNode::Cmp {
                table,
                column,
                op,
                value,
            } => {
                self.push_quoted(table);
                self.push('.');
                self.push_quoted(column);
                self.push(' ');
                self.push_op(*op);
                self.push(' ');
                if value.is_null() && *op == CmpOp::Eq {
                    self.push_str("IS NULL");
                } else if value.is_null() && *op == CmpOp::Ne {
                    self.push_str("IS NOT NULL");
                } else {
                    self.push_bind(value.clone());
                }
            }
            FilterNode::Between {
                table,
                column,
                lo,
                hi,
            } => {
                self.push_quoted(table);
                self.push('.');
                self.push_quoted(column);
                self.push_str(" BETWEEN ");
                self.push_bind(lo.clone());
                self.push_str(" AND ");
                self.push_bind(hi.clone());
            }
            FilterNode::Null {
                table,
                column,
                negated,
            } => {
                self.push_quoted(table);
                self.push('.');
                self.push_quoted(column);
                if *negated {
                    self.push_str(" IS NOT NULL");
                } else {
                    self.push_str(" IS NULL");
                }
            }
            FilterNode::In {
                table,
                column,
                values,
                negated,
            } => {
                self.push_quoted(table);
                self.push('.');
                self.push_quoted(column);
                if *negated {
                    self.push_str(" NOT IN (");
                } else {
                    self.push_str(" IN (");
                }
                for (i, v) in values.iter().enumerate() {
                    if i > 0 {
                        self.push_str(", ");
                    }
                    self.push_bind(v.clone());
                }
                self.push(')');
            }
            FilterNode::InSubquery {
                table,
                column,
                subquery,
                negated,
            } => {
                self.push_quoted(table);
                self.push('.');
                self.push_quoted(column);
                if *negated {
                    self.push_str(" NOT IN (");
                } else {
                    self.push_str(" IN (");
                }
                self.push_subquery(subquery);
                self.push(')');
            }
            FilterNode::ColumnCmp {
                left_table,
                left_col,
                op,
                right_table,
                right_col,
            } => {
                self.push_quoted(left_table);
                self.push('.');
                self.push_quoted(left_col);
                self.push(' ');
                self.push_op(*op);
                self.push(' ');
                self.push_quoted(right_table);
                self.push('.');
                self.push_quoted(right_col);
            }
            FilterNode::And(nodes) => {
                if nodes.is_empty() {
                    self.push_str("TRUE");
                } else if nodes.len() == 1 {
                    self.push_filter(&nodes[0]);
                } else {
                    self.push('(');
                    for (i, n) in nodes.iter().enumerate() {
                        if i > 0 {
                            self.push_str(" AND ");
                        }
                        self.push_filter(n);
                    }
                    self.push(')');
                }
            }
            FilterNode::Or(nodes) => {
                if nodes.is_empty() {
                    self.push_str("FALSE");
                } else if nodes.len() == 1 {
                    self.push_filter(&nodes[0]);
                } else {
                    self.push('(');
                    for (i, n) in nodes.iter().enumerate() {
                        if i > 0 {
                            self.push_str(" OR ");
                        }
                        self.push_filter(n);
                    }
                    self.push(')');
                }
            }
            FilterNode::Not(n) => {
                self.push_str("NOT (");
                self.push_filter(n);
                self.push(')');
            }
            FilterNode::Exists {
                child_table,
                child_col,
                parent_table,
                parent_col,
                filter,
                negated,
            } => {
                if *negated {
                    self.push_str("NOT ");
                }
                self.push_str("EXISTS (SELECT 1 FROM ");
                self.push_quoted(child_table);
                self.push_str(" WHERE ");
                self.push_quoted(child_table);
                self.push('.');
                self.push_quoted(child_col);
                self.push_str(" = ");
                self.push_quoted(parent_table);
                self.push('.');
                self.push_quoted(parent_col);
                if !matches!(filter.as_ref(), FilterNode::And(v) if v.is_empty()) {
                    self.push_str(" AND ");
                    self.push_filter(filter);
                }
                self.push(')');
            }
            FilterNode::ExistsSubquery { subquery, negated } => {
                if *negated {
                    self.push_str("NOT ");
                }
                self.push_str("EXISTS (");
                self.push_subquery(subquery);
                self.push(')');
            }
            FilterNode::Raw(raw) => {
                for (i, part) in raw.parts.iter().enumerate() {
                    self.push_str(part);
                    if let Some(bind) = raw.binds.get(i) {
                        self.push_bind(bind.clone());
                    }
                }
            }
        }
    }

    fn push_op(&mut self, op: CmpOp) {
        let s = match op {
            CmpOp::Eq => "=",
            CmpOp::Ne => "!=",
            CmpOp::Gt => ">",
            CmpOp::Gte => ">=",
            CmpOp::Lt => "<",
            CmpOp::Lte => "<=",
            CmpOp::Like => "LIKE",
            CmpOp::Ilike => "ILIKE",
        };
        self.push_str(s);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::col::Column;
    use crate::error::Error;
    use crate::executor::{BoxRowStream, Executor, RawRow, RowBatch};
    use crate::filter::{Filter, RawFragment, all, any};
    use crate::order::OrderBy;
    use crate::query::{SelectQuery, SetOp, SetOpQuery};
    use crate::value::Value;
    use ruprizzle_core::ir::Provider;
    use ruprizzle_dialect::dialect_for;
    use std::borrow::Cow;

    macro_rules! unit_row_decode {
        ($t:ty) => {
            impl<'r> sqlx::FromRow<'r, sqlx::any::AnyRow> for $t {
                fn from_row(_: &'r sqlx::any::AnyRow) -> Result<Self, sqlx::Error> {
                    let v: $t = Default::default();
                    Ok(v)
                }
            }
            impl<'r> sqlx::FromRow<'r, sqlx::postgres::PgRow> for $t {
                fn from_row(_: &'r sqlx::postgres::PgRow) -> Result<Self, sqlx::Error> {
                    let v: $t = Default::default();
                    Ok(v)
                }
            }
            impl<'r> sqlx::FromRow<'r, sqlx::sqlite::SqliteRow> for $t {
                fn from_row(_: &'r sqlx::sqlite::SqliteRow) -> Result<Self, sqlx::Error> {
                    let v: $t = Default::default();
                    Ok(v)
                }
            }
            impl<'r> sqlx::FromRow<'r, sqlx::mysql::MySqlRow> for $t {
                fn from_row(_: &'r sqlx::mysql::MySqlRow) -> Result<Self, sqlx::Error> {
                    let v: $t = Default::default();
                    Ok(v)
                }
            }
            #[cfg(feature = "sqlite-rusqlite")]
            impl crate::rusqlite::FromRusqliteRow for $t {
                fn from_rusqlite_row(
                    _: &crate::rusqlite::RusqliteRow,
                ) -> Result<Self, crate::Error> {
                    let v: $t = Default::default();
                    Ok(v)
                }
            }
            #[cfg(feature = "sqlite-rusqlite")]
            impl crate::rusqlite::FromOwnedRow for $t {
                fn from_owned_row(_: &crate::rusqlite::Row) -> Result<Self, crate::Error> {
                    let v: $t = Default::default();
                    Ok(v)
                }
            }
            #[cfg(feature = "postgres-tokio-postgres")]
            impl crate::tokio_postgres::FromTokioPostgresRow for $t {
                fn from_tokio_postgres_row(
                    _: &crate::tokio_postgres::Row,
                ) -> Result<Self, crate::Error> {
                    let v: $t = Default::default();
                    Ok(v)
                }
            }
        };
    }

    #[derive(Default)]
    struct User;
    impl Model for User {
        const TABLE: &'static str = "users";
    }
    unit_row_decode!(User);

    #[derive(Default)]
    struct JoinUser;
    impl Model for JoinUser {
        const TABLE: &'static str = "users";
        const PRIMARY_KEY: &'static str = "id";
        const COLUMNS: &'static [&'static str] = &["id", "name"];
    }
    unit_row_decode!(JoinUser);

    #[derive(Default)]
    struct JoinPost;
    impl Model for JoinPost {
        const TABLE: &'static str = "posts";
        const PRIMARY_KEY: &'static str = "id";
        const COLUMNS: &'static [&'static str] = &["id", "title", "user_id"];
    }
    unit_row_decode!(JoinPost);

    #[derive(Default)]
    struct SelfJoinUser;
    impl Model for SelfJoinUser {
        const TABLE: &'static str = "employees";
        const PRIMARY_KEY: &'static str = "id";
        const COLUMNS: &'static [&'static str] = &["id", "name", "manager_id"];
    }
    unit_row_decode!(SelfJoinUser);

    const ID: Column<User, i64> = Column::new("users", "id");
    const EMAIL: Column<User, Option<String>> = Column::new("users", "email");
    const AGE: Column<User, i32> = Column::new("users", "age");

    const USER_ID: Column<JoinUser, i64> = Column::new("users", "id");
    const POST_USER_ID: Column<JoinPost, i64> = Column::new("posts", "user_id");

    const EMPLOYEE_ID: Column<SelfJoinUser, i64> = Column::new("employees", "id");
    const MANAGER_ID: Column<SelfJoinUser, i64> = Column::new("employees", "manager_id");
    const EMPLOYEE_NAME: Column<SelfJoinUser, String> = Column::new("employees", "name");
    const MANAGER_ID_AS_M: Column<SelfJoinUser, i64> = EMPLOYEE_ID.aliased("m");

    #[derive(Default)]
    struct Reports;
    impl Model for Reports {
        const TABLE: &'static str = "reports";
        const PRIMARY_KEY: &'static str = "id";
        const COLUMNS: &'static [&'static str] = &["id", "name", "manager_id"];
    }
    unit_row_decode!(Reports);

    const REPORTS_MANAGER_ID: Column<Reports, i64> = Column::new("reports", "manager_id");

    const NAME: Column<User, String> = Column::new("users", "name");
    const ROLE: Column<User, String> = Column::new("users", "role");

    fn pg() -> &'static dyn DbDialect {
        dialect_for(Provider::Postgres)
    }

    fn sqlite() -> &'static dyn DbDialect {
        dialect_for(Provider::Sqlite)
    }

    #[test]
    fn select_no_filter() {
        let c = select::<User>(
            pg(),
            "users",
            &[],
            &FilterNode::And(vec![]),
            &[],
            None,
            None,
            false,
        );
        assert_eq!(c.sql, r#"SELECT * FROM "users""#);
        assert!(c.binds.is_empty());
    }

    #[test]
    fn select_projection() {
        let c = select::<User>(
            pg(),
            "users",
            &["id", "email"],
            &FilterNode::And(vec![]),
            &[],
            None,
            None,
            false,
        );
        assert_eq!(
            c.sql,
            r#"SELECT "users"."id", "users"."email" FROM "users""#
        );
        assert!(c.binds.is_empty());
    }

    #[test]
    fn select_with_filter() {
        let f = ID.eq(1);
        let c = select::<User>(pg(), "users", &[], &f.node, &[], None, None, false);
        assert_eq!(c.sql, r#"SELECT * FROM "users" WHERE "users"."id" = $1"#);
        assert_eq!(c.binds, vec![Value::I64(1)]);
    }

    #[test]
    fn placeholders_are_sqlite_q() {
        let f = ID.eq(1).and(EMAIL.eq("a".to_string()));
        let c = select::<User>(sqlite(), "users", &[], &f.node, &[], None, None, false);
        assert_eq!(
            c.sql,
            r#"SELECT * FROM `users` WHERE (`users`.`id` = ? AND `users`.`email` = ?)"#
        );
        assert_eq!(c.binds, vec![Value::I64(1), Value::Str("a".into())]);
    }

    #[test]
    fn ordering_and_pagination() {
        let c = select::<User>(
            pg(),
            "users",
            &[],
            &FilterNode::And(vec![]),
            &[OrderBy::new("users", "id", true)],
            Some(10),
            Some(20),
            false,
        );
        assert_eq!(
            c.sql,
            r#"SELECT * FROM "users" ORDER BY "users"."id" DESC LIMIT 10 OFFSET 20"#
        );
    }

    #[test]
    fn all_empty_is_true_any_empty_is_false() {
        let a: Filter<User> = all(Vec::<Filter<User>>::new());
        let c = select::<User>(pg(), "users", &[], &a.node, &[], None, None, false);
        assert_eq!(c.sql, r#"SELECT * FROM "users""#);

        let a: Filter<User> = any(Vec::<Filter<User>>::new());
        let c = select::<User>(pg(), "users", &[], &a.node, &[], None, None, false);
        assert_eq!(c.sql, r#"SELECT * FROM "users" WHERE FALSE"#);
    }

    #[test]
    fn filter_flattening_and_combinators() {
        let f = ID.eq(1).and(AGE.gt(0)).and(EMAIL.eq("a".to_string()));
        let c = select::<User>(pg(), "users", &[], &f.node, &[], None, None, false);
        assert_eq!(
            c.sql,
            r#"SELECT * FROM "users" WHERE ("users"."id" = $1 AND "users"."age" > $2 AND "users"."email" = $3)"#
        );

        let f = ID.eq(1).or(AGE.gt(0)).or(EMAIL.eq("a".to_string()));
        let c = select::<User>(pg(), "users", &[], &f.node, &[], None, None, false);
        assert_eq!(
            c.sql,
            r#"SELECT * FROM "users" WHERE ("users"."id" = $1 OR "users"."age" > $2 OR "users"."email" = $3)"#
        );

        let f = (!ID.eq(1)).and(AGE.in_(vec![1, 2, 3]));
        let c = select::<User>(pg(), "users", &[], &f.node, &[], None, None, false);
        assert_eq!(
            c.sql,
            r#"SELECT * FROM "users" WHERE (NOT ("users"."id" = $1) AND "users"."age" IN ($2, $3, $4))"#
        );
        assert_eq!(
            c.binds,
            vec![Value::I64(1), Value::I32(1), Value::I32(2), Value::I32(3)]
        );
    }

    #[derive(Default)]
    struct Post;
    impl Model for Post {
        const TABLE: &'static str = "posts";
    }
    unit_row_decode!(Post);

    #[test]
    fn relation_exists_filter() {
        let child = Filter::<Post>::new(FilterNode::Cmp {
            table: "posts",
            column: "published",
            op: CmpOp::Eq,
            value: Value::Bool(true),
        });

        let f = Filter::<User>::new(FilterNode::Exists {
            child_table: "posts",
            child_col: "author_id",
            parent_table: "users",
            parent_col: "id",
            filter: Box::new(child.node.clone()),
            negated: false,
        });
        let c = select::<User>(pg(), "users", &[], &f.node, &[], None, None, false);
        assert_eq!(
            c.sql,
            r#"SELECT * FROM "users" WHERE EXISTS (SELECT 1 FROM "posts" WHERE "posts"."author_id" = "users"."id" AND "posts"."published" = $1)"#
        );
        assert_eq!(c.binds, vec![Value::Bool(true)]);

        let f = Filter::<User>::new(FilterNode::Exists {
            child_table: "posts",
            child_col: "author_id",
            parent_table: "users",
            parent_col: "id",
            filter: Box::new(child.node.clone()),
            negated: true,
        });
        let c = select::<User>(pg(), "users", &[], &f.node, &[], None, None, false);
        assert_eq!(
            c.sql,
            r#"SELECT * FROM "users" WHERE NOT EXISTS (SELECT 1 FROM "posts" WHERE "posts"."author_id" = "users"."id" AND "posts"."published" = $1)"#
        );
    }

    #[test]
    fn null_and_between_and_like() {
        let f = EMAIL.is_null().and(AGE.between(0, 120));
        let c = select::<User>(pg(), "users", &[], &f.node, &[], None, None, false);
        assert_eq!(
            c.sql,
            r#"SELECT * FROM "users" WHERE ("users"."email" IS NULL AND "users"."age" BETWEEN $1 AND $2)"#
        );
    }

    #[test]
    fn insert_sql() {
        let c = insert::<User>(
            pg(),
            "users",
            &[
                ("email", Value::Str("a@b.c".into())),
                ("age", Value::I32(30)),
            ],
            &[],
        );
        assert_eq!(
            c.sql,
            r#"INSERT INTO "users" ("email", "age") VALUES ($1, $2)"#
        );
    }

    #[test]
    fn update_and_delete_guards() {
        let u = update::<User>(
            pg(),
            "users",
            &[("age", Value::I32(30))],
            &FilterNode::And(vec![]),
            &[],
        );
        assert!(u.sql.starts_with(r#"UPDATE "users" SET "age" = $1"#));

        let d = delete::<User>(pg(), "users", &FilterNode::And(vec![]), &[]);
        assert!(d.sql.starts_with(r#"DELETE FROM "users""#));
    }

    #[test]
    fn upsert_sql() {
        let c = upsert::<User>(
            pg(),
            "users",
            &[
                ("email", Value::Str("a@b.c".into())),
                ("age", Value::I32(30)),
            ],
            &["email"],
            &["age"],
            &[],
        );
        assert_eq!(
            c.sql,
            r#"INSERT INTO "users" ("email", "age") VALUES ($1, $2) ON CONFLICT (email) DO UPDATE SET "age" = EXCLUDED."age""#
        );
        assert_eq!(c.binds, vec![Value::Str("a@b.c".into()), Value::I32(30)]);
    }

    #[test]
    fn raw_filter_postgres() {
        let fragment = RawFragment::new(
            vec!["email = ".to_string(), "".to_string()],
            vec![Value::Str("a@b.c".into())],
        );
        let f: Filter<User> = Filter::raw(fragment);
        let c = select::<User>(pg(), "users", &[], &f.node, &[], None, None, false);
        assert_eq!(c.sql, r#"SELECT * FROM "users" WHERE email = $1"#);
        assert_eq!(c.binds, vec![Value::Str("a@b.c".into())]);
    }

    #[test]
    fn raw_filter_sqlite() {
        let fragment = RawFragment::new(
            vec!["email = ".to_string(), "".to_string()],
            vec![Value::Str("a@b.c".into())],
        );
        let f: Filter<User> = Filter::raw(fragment);
        let c = select::<User>(sqlite(), "users", &[], &f.node, &[], None, None, false);
        assert_eq!(c.sql, r#"SELECT * FROM `users` WHERE email = ?"#);
        assert_eq!(c.binds, vec![Value::Str("a@b.c".into())]);
    }

    #[test]
    fn raw_fragment_sql() {
        let raw = RawFragment::new(
            vec!["x = ".to_string(), " AND y = ".to_string(), "".to_string()],
            vec![Value::I64(1), Value::I64(2)],
        );
        assert_eq!(raw.sql(), "x = $1 AND y = $2");
        assert_eq!(raw.binds(), &[Value::I64(1), Value::I64(2)]);
    }

    #[test]
    fn count_ignores_order_limit() {
        let f = AGE.gt(0);
        let c = count::<User>(pg(), "users", &f.node);
        assert_eq!(
            c.sql,
            r#"SELECT COUNT(*) FROM "users" WHERE "users"."age" > $1"#
        );
        assert_eq!(c.binds, vec![Value::I32(0)]);
    }

    #[test]
    fn exists_adds_limit_1() {
        let f = AGE.gt(0);
        let c = exists::<User>(pg(), "users", &f.node);
        assert_eq!(
            c.sql,
            r#"SELECT 1 FROM "users" WHERE "users"."age" > $1 LIMIT 1"#
        );
        assert_eq!(c.binds, vec![Value::I32(0)]);
    }

    #[test]
    fn inner_join_sql_postgres() {
        let c = join_select::<JoinUser, JoinPost>(
            pg(),
            "users",
            "posts",
            None,
            JoinKind::Inner,
            &USER_ID.on(POST_USER_ID).node,
            &FilterNode::And(vec![]),
            &[],
            None,
            None,
            false,
        );
        assert_eq!(
            c.sql,
            r#"SELECT "users"."id", "users"."name", "posts"."id", "posts"."title", "posts"."user_id" FROM "users" INNER JOIN "posts" ON "users"."id" = "posts"."user_id""#
        );
        assert!(c.binds.is_empty());
    }

    #[test]
    fn right_join_sql_postgres() {
        let c = join_select::<JoinUser, JoinPost>(
            pg(),
            "users",
            "posts",
            None,
            JoinKind::Right,
            &USER_ID.on(POST_USER_ID).node,
            &FilterNode::And(vec![]),
            &[],
            None,
            None,
            false,
        );
        assert_eq!(
            c.sql,
            r#"SELECT "users"."id", "users"."name", "posts"."id", "posts"."title", "posts"."user_id" FROM "users" RIGHT JOIN "posts" ON "users"."id" = "posts"."user_id""#
        );
        assert!(c.binds.is_empty());
    }

    #[test]
    fn full_join_sql_postgres() {
        let c = join_select::<JoinUser, JoinPost>(
            pg(),
            "users",
            "posts",
            None,
            JoinKind::Full,
            &USER_ID.on(POST_USER_ID).node,
            &FilterNode::And(vec![]),
            &[],
            None,
            None,
            false,
        );
        assert_eq!(
            c.sql,
            r#"SELECT "users"."id", "users"."name", "posts"."id", "posts"."title", "posts"."user_id" FROM "users" FULL OUTER JOIN "posts" ON "users"."id" = "posts"."user_id""#
        );
        assert!(c.binds.is_empty());
    }

    #[test]
    fn left_join_sql_sqlite() {
        let c = join_select::<JoinUser, JoinPost>(
            sqlite(),
            "users",
            "posts",
            None,
            JoinKind::Left,
            &USER_ID.on(POST_USER_ID).node,
            &FilterNode::And(vec![]),
            &[],
            None,
            None,
            false,
        );
        assert_eq!(
            c.sql,
            r#"SELECT `users`.`id`, `users`.`name`, `posts`.`id`, `posts`.`title`, `posts`.`user_id` FROM `users` LEFT JOIN `posts` ON `users`.`id` = `posts`.`user_id`"#
        );
        assert!(c.binds.is_empty());
    }

    #[test]
    fn self_join_sql_postgres() {
        let c = join_select::<SelfJoinUser, SelfJoinUser>(
            pg(),
            "employees",
            "employees",
            Some("m"),
            JoinKind::Inner,
            &MANAGER_ID.on(MANAGER_ID_AS_M).node,
            &FilterNode::And(vec![]),
            &[],
            None,
            None,
            false,
        );
        assert_eq!(
            c.sql,
            r#"SELECT "employees"."id", "employees"."name", "employees"."manager_id", "m"."id", "m"."name", "m"."manager_id" FROM "employees" INNER JOIN "employees" AS "m" ON "employees"."manager_id" = "m"."id""#
        );
        assert!(c.binds.is_empty());
    }

    #[test]
    fn in_subquery_postgres() {
        let subquery = CompiledSql {
            sql: std::borrow::Cow::Borrowed(
                r#"SELECT "posts"."author_id" FROM "posts" WHERE "posts"."published" = $1"#,
            ),
            binds: vec![Value::Bool(true)],
        };
        let f = Filter::<User>::new(FilterNode::InSubquery {
            table: "users",
            column: "id",
            subquery,
            negated: false,
        });
        let c = select::<User>(
            pg(),
            "users",
            &["id", "name"],
            &f.node,
            &[],
            None,
            None,
            false,
        );
        assert_eq!(
            c.sql,
            r#"SELECT "users"."id", "users"."name" FROM "users" WHERE "users"."id" IN (SELECT "posts"."author_id" FROM "posts" WHERE "posts"."published" = $1)"#
        );
        assert_eq!(c.binds, vec![Value::Bool(true)]);
    }

    #[test]
    fn in_subquery_sqlite() {
        let subquery = CompiledSql {
            sql: std::borrow::Cow::Borrowed(
                r#"SELECT `posts`.`author_id` FROM `posts` WHERE `posts`.`published` = ?"#,
            ),
            binds: vec![Value::Bool(true)],
        };
        let f = Filter::<User>::new(FilterNode::InSubquery {
            table: "users",
            column: "id",
            subquery,
            negated: false,
        });
        let c = select::<User>(
            sqlite(),
            "users",
            &["id", "name"],
            &f.node,
            &[],
            None,
            None,
            false,
        );
        assert_eq!(
            c.sql,
            r#"SELECT `users`.`id`, `users`.`name` FROM `users` WHERE `users`.`id` IN (SELECT `posts`.`author_id` FROM `posts` WHERE `posts`.`published` = ?)"#
        );
        assert_eq!(c.binds, vec![Value::Bool(true)]);
    }

    #[test]
    fn in_subquery_postgres_with_outer_binds() {
        let subquery = CompiledSql {
            sql: std::borrow::Cow::Borrowed(
                r#"SELECT "posts"."author_id" FROM "posts" WHERE "posts"."published" = $1 AND "posts"."title" = $2"#,
            ),
            binds: vec![Value::Bool(true), Value::Str("First".into())],
        };
        let f = Filter::<User>::new(FilterNode::And(vec![
            FilterNode::Cmp {
                table: "users",
                column: "age",
                op: CmpOp::Eq,
                value: Value::I32(30),
            },
            FilterNode::InSubquery {
                table: "users",
                column: "id",
                subquery,
                negated: true,
            },
        ]));
        let c = select::<User>(
            pg(),
            "users",
            &["id", "name"],
            &f.node,
            &[],
            None,
            None,
            false,
        );
        assert_eq!(
            c.sql,
            r#"SELECT "users"."id", "users"."name" FROM "users" WHERE ("users"."age" = $1 AND "users"."id" NOT IN (SELECT "posts"."author_id" FROM "posts" WHERE "posts"."published" = $2 AND "posts"."title" = $3))"#
        );
        assert_eq!(
            c.binds,
            vec![
                Value::I32(30),
                Value::Bool(true),
                Value::Str("First".into())
            ]
        );
    }

    #[test]
    fn exists_subquery_postgres() {
        let subquery = CompiledSql {
            sql: Cow::Borrowed(
                r#"SELECT "posts"."id" FROM "posts" WHERE "posts"."author_id" = "users"."id""#,
            ),
            binds: vec![],
        };
        let f = Filter::<User>::new(FilterNode::ExistsSubquery {
            subquery,
            negated: false,
        });
        let c = select::<User>(
            pg(),
            "users",
            &["id", "name"],
            &f.node,
            &[],
            None,
            None,
            false,
        );
        assert_eq!(
            c.sql,
            r#"SELECT "users"."id", "users"."name" FROM "users" WHERE EXISTS (SELECT "posts"."id" FROM "posts" WHERE "posts"."author_id" = "users"."id")"#
        );
        assert!(c.binds.is_empty());
    }

    #[test]
    fn not_exists_subquery_sqlite() {
        let subquery = CompiledSql {
            sql: Cow::Borrowed(
                r#"SELECT `posts`.`id` FROM `posts` WHERE `posts`.`author_id` = `users`.`id`"#,
            ),
            binds: vec![],
        };
        let f = Filter::<User>::new(FilterNode::ExistsSubquery {
            subquery,
            negated: true,
        });
        let c = select::<User>(
            sqlite(),
            "users",
            &["id", "name"],
            &f.node,
            &[],
            None,
            None,
            false,
        );
        assert_eq!(
            c.sql,
            r#"SELECT `users`.`id`, `users`.`name` FROM `users` WHERE NOT EXISTS (SELECT `posts`.`id` FROM `posts` WHERE `posts`.`author_id` = `users`.`id`)"#
        );
        assert!(c.binds.is_empty());
    }

    #[test]
    fn exists_subquery_postgres_with_outer_binds() {
        let subquery = CompiledSql {
            sql: Cow::Borrowed(
                r#"SELECT "posts"."id" FROM "posts" WHERE "posts"."author_id" = "users"."id" AND "posts"."published" = $1"#,
            ),
            binds: vec![Value::Bool(true)],
        };
        let f = Filter::<User>::new(FilterNode::And(vec![
            FilterNode::Cmp {
                table: "users",
                column: "age",
                op: CmpOp::Gt,
                value: Value::I32(30),
            },
            FilterNode::ExistsSubquery {
                subquery,
                negated: false,
            },
        ]));
        let c = select::<User>(
            pg(),
            "users",
            &["id", "name"],
            &f.node,
            &[],
            None,
            None,
            false,
        );
        assert_eq!(
            c.sql,
            r#"SELECT "users"."id", "users"."name" FROM "users" WHERE ("users"."age" > $1 AND EXISTS (SELECT "posts"."id" FROM "posts" WHERE "posts"."author_id" = "users"."id" AND "posts"."published" = $2))"#
        );
        assert_eq!(c.binds, vec![Value::I32(30), Value::Bool(true)]);
    }

    struct NoopExecutor(&'static dyn DbDialect);

    impl Executor for NoopExecutor {
        fn dialect(&self) -> &dyn DbDialect {
            self.0
        }

        fn fetch_all_raw(
            &self,
            _sql: Cow<'static, str>,
            _binds: Vec<Value>,
        ) -> crate::BoxFuture<'_, Result<RowBatch, Error>> {
            Box::pin(async { Ok(RowBatch::Any(Vec::new())) })
        }

        fn execute_raw(
            &self,
            _sql: Cow<'static, str>,
            _binds: Vec<Value>,
        ) -> crate::BoxFuture<'_, Result<u64, Error>> {
            Box::pin(async { Ok(0) })
        }

        fn stream_raw(&self, _sql: Cow<'static, str>, _binds: Vec<Value>) -> BoxRowStream<'_> {
            Box::pin(futures_util::stream::empty::<Result<RawRow, Error>>())
        }
    }

    #[test]
    fn cte_postgres() {
        let exec = NoopExecutor(pg());
        let body = SelectQuery::<User>::new(&exec)
            .filter(ROLE.eq("manager"))
            .columns(ID);
        let q = SelectQuery::<User>::new(&exec)
            .with("managers", body)
            .columns((ID, NAME));
        let c = q.to_sql();
        assert_eq!(
            c.sql,
            r#"WITH "managers" AS (SELECT "users"."id" FROM "users" WHERE "users"."role" = $1) SELECT "users"."id", "users"."name" FROM "users""#
        );
        assert_eq!(c.binds, vec![Value::Str("manager".into())]);
    }

    #[test]
    fn cte_sqlite() {
        let exec = NoopExecutor(sqlite());
        let body = SelectQuery::<User>::new(&exec)
            .filter(ROLE.eq("manager"))
            .columns(ID);
        let q = SelectQuery::<User>::new(&exec)
            .with("managers", body)
            .columns((ID, NAME));
        let c = q.to_sql();
        assert_eq!(
            c.sql,
            r#"WITH `managers` AS (SELECT `users`.`id` FROM `users` WHERE `users`.`role` = ?) SELECT `users`.`id`, `users`.`name` FROM `users`"#
        );
        assert_eq!(c.binds, vec![Value::Str("manager".into())]);
    }

    #[test]
    fn recursive_cte_postgres() {
        let exec = NoopExecutor(pg());
        let anchor = SelectQuery::<SelfJoinUser>::new(&exec).filter(MANAGER_ID.eq(1));
        let recursive = SelectQuery::<SelfJoinUser>::new(&exec).filter(
            Filter::<SelfJoinUser>::exists(
                SelectQuery::<Reports>::new(&exec)
                    .filter(REPORTS_MANAGER_ID.correlated_to(EMPLOYEE_ID)),
            )
            .and(EMPLOYEE_NAME.eq("x")),
        );
        let q = SelectQuery::<Reports>::new(&exec).with_recursive("reports", anchor, recursive);
        let c = q.to_sql();
        assert!(c.sql.starts_with(r#"WITH RECURSIVE "reports" AS ("#));
        assert!(c.sql.contains("UNION ALL"));
        assert!(c.sql.ends_with(
            r#"SELECT "reports"."id", "reports"."name", "reports"."manager_id" FROM "reports""#
        ));
        assert_eq!(c.binds, vec![Value::I64(1), Value::Str("x".into())]);
    }

    #[test]
    fn union_sql_postgres() {
        let exec = NoopExecutor(pg());
        let left = SelectQuery::<JoinUser>::new(&exec).columns(USER_ID);
        let right = SelectQuery::<JoinPost>::new(&exec).columns(POST_USER_ID);
        let q = SetOpQuery::new(&exec, SetOp::Union, left, right);
        let c = q.to_sql();
        assert_eq!(
            c.sql,
            r#"(SELECT "users"."id" FROM "users") UNION (SELECT "posts"."user_id" FROM "posts")"#
        );
        assert!(c.binds.is_empty());
    }

    #[test]
    fn union_all_sqlite() {
        let exec = NoopExecutor(sqlite());
        let left = SelectQuery::<JoinUser>::new(&exec)
            .filter(USER_ID.eq(1))
            .columns(USER_ID);
        let right = SelectQuery::<JoinPost>::new(&exec)
            .filter(POST_USER_ID.eq(2))
            .columns(POST_USER_ID);
        let q = SetOpQuery::new(&exec, SetOp::UnionAll, left, right);
        let c = q.to_sql();
        assert_eq!(
            c.sql,
            r#"SELECT * FROM (SELECT `users`.`id` FROM `users` WHERE `users`.`id` = ?) AS __rz_l UNION ALL SELECT * FROM (SELECT `posts`.`user_id` FROM `posts` WHERE `posts`.`user_id` = ?) AS __rz_r"#
        );
        assert_eq!(c.binds, vec![Value::I64(1), Value::I64(2)]);
    }

    #[test]
    fn set_op_with_binds_postgres() {
        let exec = NoopExecutor(pg());
        let left = SelectQuery::<JoinUser>::new(&exec)
            .filter(USER_ID.eq(1))
            .columns(USER_ID);
        let right = SelectQuery::<JoinPost>::new(&exec)
            .filter(POST_USER_ID.in_(vec![2_i64, 3_i64]))
            .columns(POST_USER_ID);
        let q = SetOpQuery::new(&exec, SetOp::Union, left, right);
        let c = q.to_sql();
        assert_eq!(
            c.sql,
            r#"(SELECT "users"."id" FROM "users" WHERE "users"."id" = $1) UNION (SELECT "posts"."user_id" FROM "posts" WHERE "posts"."user_id" IN ($2, $3))"#
        );
        assert_eq!(c.binds, vec![Value::I64(1), Value::I64(2), Value::I64(3)]);
    }

    #[test]
    fn intersect_sql_postgres() {
        let exec = NoopExecutor(pg());
        let left = SelectQuery::<JoinUser>::new(&exec).columns(USER_ID);
        let right = SelectQuery::<JoinPost>::new(&exec).columns(POST_USER_ID);
        let q = SetOpQuery::new(&exec, SetOp::Intersect, left, right);
        let c = q.to_sql();
        assert_eq!(
            c.sql,
            r#"(SELECT "users"."id" FROM "users") INTERSECT (SELECT "posts"."user_id" FROM "posts")"#
        );
        assert!(c.binds.is_empty());
    }

    #[test]
    fn except_sql_postgres() {
        let exec = NoopExecutor(pg());
        let left = SelectQuery::<JoinUser>::new(&exec).columns(USER_ID);
        let right = SelectQuery::<JoinPost>::new(&exec).columns(POST_USER_ID);
        let q = SetOpQuery::new(&exec, SetOp::Except, left, right);
        let c = q.to_sql();
        assert_eq!(
            c.sql,
            r#"(SELECT "users"."id" FROM "users") EXCEPT (SELECT "posts"."user_id" FROM "posts")"#
        );
        assert!(c.binds.is_empty());
    }
}
