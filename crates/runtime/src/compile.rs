//! SQL compiler: turns `Filter`/`Order`/`Value` trees into parameterised SQL.
//!
//! Every runtime value is pushed as a placeholder; there is no string
//! interpolation of user data. The compiler is dialect-aware so it can quote
//! identifiers and produce the correct parameter markers.

use ruprizzle_core::ir::Provider;
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
        for (i, o) in order.iter().enumerate() {
            if i > 0 {
                c.push_str(", ");
            }
            c.push_quoted(o.table);
            c.push('.');
            c.push_quoted(o.column);
            if o.desc {
                c.push_str(" DESC");
            } else {
                c.push_str(" ASC");
            }
        }
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

/// Build a `Box<dyn DbDialect>` from an `Any` pool URL scheme.
pub fn dialect_for_pool(pool: &crate::Pool) -> Box<dyn DbDialect> {
    let opts = pool.connect_options();
    let scheme = opts.database_url.scheme();
    Provider::parse(scheme)
        .map(dialect_for)
        .unwrap_or_else(|| dialect_for(Provider::Postgres))
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
    use crate::filter::{Filter, all, any};
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
}
