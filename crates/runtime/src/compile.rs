//! SQL compiler: turns `Filter`/`Order`/`Value` trees into parameterised SQL.
//!
//! Every runtime value is pushed as a placeholder; there is no string
//! interpolation of user data. The compiler is dialect-aware so it can quote
//! identifiers and produce the correct parameter markers.

use ruprizzle_dialect::{DbDialect, dialect_for};

use crate::filter::{CmpOp, FilterNode};
use crate::model::Model;
use crate::order::OrderBy;
use crate::value::Value;

/// A compiled SQL statement and its bound values.
#[derive(Debug, Clone, PartialEq)]
pub struct CompiledSql {
    /// The SQL string with placeholders.
    pub sql: String,
    /// The values bound to the placeholders, in order.
    pub binds: Vec<Value>,
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

/// Compile a `SELECT COUNT(*)` for `M`.
///
/// Skips `ORDER BY`, `LIMIT` and `OFFSET` because an aggregate count over an
/// unordered, unlimited relation is what callers of `SelectQuery::count` want.
#[must_use]
pub fn count<M: Model>(
    dialect: &dyn DbDialect,
    table: &str,
    filter: &FilterNode,
) -> CompiledSql {
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
pub fn exists<M: Model>(
    dialect: &dyn DbDialect,
    table: &str,
    filter: &FilterNode,
) -> CompiledSql {
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

/// Build a `Box<dyn DbDialect>` from a `Pool`.
pub fn dialect_for_pool(pool: &crate::Pool) -> Box<dyn DbDialect> {
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
            sql: self.sql,
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
            FilterNode::And(nodes) => {
                if nodes.is_empty() {
                    self.push_str("TRUE");
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
    use crate::filter::{Filter, RawFragment, all, any};
    use crate::order::OrderBy;
    use crate::value::Value;
    use ruprizzle_core::ir::Provider;
    use ruprizzle_dialect::dialect_for;

    struct User;
    impl Model for User {
        const TABLE: &'static str = "users";
    }

    const ID: Column<User, i64> = Column::new("users", "id");
    const EMAIL: Column<User, Option<String>> = Column::new("users", "email");
    const AGE: Column<User, i32> = Column::new("users", "age");

    fn pg() -> Box<dyn DbDialect> {
        dialect_for(Provider::Postgres)
    }

    fn sqlite() -> Box<dyn DbDialect> {
        dialect_for(Provider::Sqlite)
    }

    #[test]
    fn select_no_filter() {
        let c = select::<User>(
            pg().as_ref(),
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
            pg().as_ref(),
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
        let c = select::<User>(pg().as_ref(), "users", &[], &f.node, &[], None, None, false);
        assert_eq!(c.sql, r#"SELECT * FROM "users" WHERE "users"."id" = $1"#);
        assert_eq!(c.binds, vec![Value::I64(1)]);
    }

    #[test]
    fn placeholders_are_sqlite_q() {
        let f = ID.eq(1).and(EMAIL.eq("a".to_string()));
        let c = select::<User>(
            sqlite().as_ref(),
            "users",
            &[],
            &f.node,
            &[],
            None,
            None,
            false,
        );
        assert_eq!(
            c.sql,
            r#"SELECT * FROM `users` WHERE (`users`.`id` = ? AND `users`.`email` = ?)"#
        );
        assert_eq!(c.binds, vec![Value::I64(1), Value::Str("a".into())]);
    }

    #[test]
    fn ordering_and_pagination() {
        let c = select::<User>(
            pg().as_ref(),
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
        let c = select::<User>(pg().as_ref(), "users", &[], &a.node, &[], None, None, false);
        assert_eq!(c.sql, r#"SELECT * FROM "users""#);

        let a: Filter<User> = any(Vec::<Filter<User>>::new());
        let c = select::<User>(pg().as_ref(), "users", &[], &a.node, &[], None, None, false);
        assert_eq!(c.sql, r#"SELECT * FROM "users" WHERE FALSE"#);
    }

    #[test]
    fn filter_flattening_and_combinators() {
        let f = ID.eq(1).and(AGE.gt(0)).and(EMAIL.eq("a".to_string()));
        let c = select::<User>(pg().as_ref(), "users", &[], &f.node, &[], None, None, false);
        assert_eq!(
            c.sql,
            r#"SELECT * FROM "users" WHERE ("users"."id" = $1 AND "users"."age" > $2 AND "users"."email" = $3)"#
        );

        let f = ID.eq(1).or(AGE.gt(0)).or(EMAIL.eq("a".to_string()));
        let c = select::<User>(pg().as_ref(), "users", &[], &f.node, &[], None, None, false);
        assert_eq!(
            c.sql,
            r#"SELECT * FROM "users" WHERE ("users"."id" = $1 OR "users"."age" > $2 OR "users"."email" = $3)"#
        );

        let f = (!ID.eq(1)).and(AGE.in_(vec![1, 2, 3]));
        let c = select::<User>(pg().as_ref(), "users", &[], &f.node, &[], None, None, false);
        assert_eq!(
            c.sql,
            r#"SELECT * FROM "users" WHERE (NOT ("users"."id" = $1) AND "users"."age" IN ($2, $3, $4))"#
        );
        assert_eq!(
            c.binds,
            vec![Value::I64(1), Value::I32(1), Value::I32(2), Value::I32(3)]
        );
    }

    #[test]
    fn relation_exists_filter() {
        struct Post;
        impl Model for Post {
            const TABLE: &'static str = "posts";
        }

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
        let c = select::<User>(pg().as_ref(), "users", &[], &f.node, &[], None, None, false);
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
        let c = select::<User>(pg().as_ref(), "users", &[], &f.node, &[], None, None, false);
        assert_eq!(
            c.sql,
            r#"SELECT * FROM "users" WHERE NOT EXISTS (SELECT 1 FROM "posts" WHERE "posts"."author_id" = "users"."id" AND "posts"."published" = $1)"#
        );
    }

    #[test]
    fn null_and_between_and_like() {
        let f = EMAIL.is_null().and(AGE.between(0, 120));
        let c = select::<User>(pg().as_ref(), "users", &[], &f.node, &[], None, None, false);
        assert_eq!(
            c.sql,
            r#"SELECT * FROM "users" WHERE ("users"."email" IS NULL AND "users"."age" BETWEEN $1 AND $2)"#
        );
    }

    #[test]
    fn insert_sql() {
        let c = insert::<User>(
            pg().as_ref(),
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
            pg().as_ref(),
            "users",
            &[("age", Value::I32(30))],
            &FilterNode::And(vec![]),
            &[],
        );
        assert!(u.sql.starts_with(r#"UPDATE "users" SET "age" = $1"#));

        let d = delete::<User>(pg().as_ref(), "users", &FilterNode::And(vec![]), &[]);
        assert!(d.sql.starts_with(r#"DELETE FROM "users""#));
    }

    #[test]
    fn upsert_sql() {
        let c = upsert::<User>(
            pg().as_ref(),
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
        let c = select::<User>(pg().as_ref(), "users", &[], &f.node, &[], None, None, false);
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
        let c = select::<User>(
            sqlite().as_ref(),
            "users",
            &[],
            &f.node,
            &[],
            None,
            None,
            false,
        );
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
        let c = count::<User>(pg().as_ref(), "users", &f.node);
        assert_eq!(
            c.sql,
            r#"SELECT COUNT(*) FROM "users" WHERE "users"."age" > $1"#
        );
        assert_eq!(c.binds, vec![Value::I32(0)]);
    }

    #[test]
    fn exists_adds_limit_1() {
        let f = AGE.gt(0);
        let c = exists::<User>(pg().as_ref(), "users", &f.node);
        assert_eq!(
            c.sql,
            r#"SELECT 1 FROM "users" WHERE "users"."age" > $1 LIMIT 1"#
        );
        assert_eq!(c.binds, vec![Value::I32(0)]);
    }
}
