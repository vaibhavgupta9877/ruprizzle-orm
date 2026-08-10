//! PostgreSQL dialect.

use ruprizzle_core::ir::{
    EnumDef, Field, FieldKind, IndexDef, Model, ResolvedRelation, ScalarType, Schema,
};

use crate::common::{
    base_column_type, create_table_body, default_sql, fk_constraint_sql, render_index_columns,
    rust_type_for,
};
use crate::{Capabilities, DbDialect, DialectError, JsonSupport, RustType, Stmt};

/// The PostgreSQL dialect implementation.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PostgresDialect;

impl DbDialect for PostgresDialect {
    fn name(&self) -> &'static str {
        "postgres"
    }

    fn quote_ident(&self, s: &str) -> String {
        format!("\"{}\"", s.replace('"', "\"\""))
    }

    fn placeholder(&self, index: usize) -> String {
        format!("${}", index + 1)
    }

    fn column_type(&self, f: &Field) -> Result<String, DialectError> {
        base_column_type("postgres", f, |f| match f.kind {
            FieldKind::Scalar(ScalarType::String) | FieldKind::Relation(_) | FieldKind::List(_) => {
                "TEXT".to_owned()
            }
            FieldKind::Scalar(ScalarType::Int) => "INTEGER".to_owned(),
            FieldKind::Scalar(ScalarType::BigInt) => "BIGINT".to_owned(),
            FieldKind::Scalar(ScalarType::Float) => "DOUBLE PRECISION".to_owned(),
            FieldKind::Scalar(ScalarType::Decimal) => "NUMERIC".to_owned(),
            FieldKind::Scalar(ScalarType::Boolean) => "BOOLEAN".to_owned(),
            FieldKind::Scalar(ScalarType::DateTime) => "TIMESTAMPTZ".to_owned(),
            FieldKind::Scalar(ScalarType::Date) => "DATE".to_owned(),
            FieldKind::Scalar(ScalarType::Time) => "TIME".to_owned(),
            FieldKind::Scalar(ScalarType::Uuid) => "UUID".to_owned(),
            FieldKind::Scalar(ScalarType::Json) => "JSONB".to_owned(),
            FieldKind::Scalar(ScalarType::Bytes) => "BYTEA".to_owned(),
            FieldKind::Enum(ref name) => name.as_str().to_owned(),
        })
    }

    fn rust_type(&self, f: &Field) -> RustType {
        rust_type_for(f)
    }

    fn create_table(&self, schema: &Schema, m: &Model) -> Vec<Stmt> {
        match create_table_body(self, schema, m) {
            Ok(stmt) => vec![stmt],
            Err(e) => vec![Stmt::new(format!("-- error: {e}"))],
        }
    }

    fn drop_table(&self, table: &str) -> Vec<Stmt> {
        vec![Stmt::new(format!("DROP TABLE IF EXISTS {};", self.quote_ident(table))).destructive()]
    }

    fn add_column(&self, _schema: &Schema, m: &Model, f: &Field) -> Vec<Stmt> {
        let col = self.column_type(f);
        let mut out = Vec::new();

        if let Ok(sql_type) = col {
            let mut parts = vec![self.quote_ident(&f.column), sql_type];

            if f.optional {
                parts.push("NULL".to_owned());
            } else {
                parts.push("NOT NULL".to_owned());
            }

            let default = default_sql(self, f);
            if !default.is_empty() {
                parts.push(format!("DEFAULT {default}"));
            }

            out.push(Stmt::new(format!(
                "ALTER TABLE {} ADD COLUMN {};",
                self.quote_ident(&m.table),
                parts.join(" ")
            )));
        } else {
            out.push(Stmt::new(format!(
                "-- error adding column: {}",
                col.unwrap_err()
            )));
        }

        // If the column is part of a unique constraint, the unique is already
        // reflected in the model. Adding a unique constraint is a separate
        // migration step handled elsewhere.
        out
    }

    fn drop_column(&self, table: &str, col: &str) -> Vec<Stmt> {
        vec![Stmt::new(format!(
            "ALTER TABLE {} DROP COLUMN {};",
            self.quote_ident(table),
            self.quote_ident(col)
        ))]
    }

    fn alter_column(&self, _schema: &Schema, m: &Model, from: &Field, to: &Field) -> Vec<Stmt> {
        let table = self.quote_ident(&m.table);
        let col = self.quote_ident(&from.column);
        let mut stmts = Vec::new();

        if from.column != to.column {
            stmts.push(Stmt::new(format!(
                "ALTER TABLE {table} RENAME COLUMN {col} TO {};",
                self.quote_ident(&to.column)
            )));
        }

        let to_col = self.quote_ident(&to.column);

        if from.kind != to.kind {
            match self.column_type(to) {
                Ok(sql_type) => stmts.push(Stmt::new(format!(
                    "ALTER TABLE {table} ALTER COLUMN {to_col} TYPE {sql_type} USING {to_col}::{sql_type};"
                ))),
                Err(e) => stmts.push(Stmt::new(format!("-- error: {e}"))),
            }
        }

        let old_default = default_sql(self, from);
        let new_default = default_sql(self, to);
        if old_default != new_default {
            if new_default.is_empty() {
                stmts.push(Stmt::new(format!(
                    "ALTER TABLE {table} ALTER COLUMN {to_col} DROP DEFAULT;"
                )));
            } else {
                stmts.push(Stmt::new(format!(
                    "ALTER TABLE {table} ALTER COLUMN {to_col} SET DEFAULT {new_default};"
                )));
            }
        }

        if from.optional != to.optional {
            if to.optional {
                stmts.push(Stmt::new(format!(
                    "ALTER TABLE {table} ALTER COLUMN {to_col} DROP NOT NULL;"
                )));
            } else {
                stmts.push(Stmt::new(format!(
                    "ALTER TABLE {table} ALTER COLUMN {to_col} SET NOT NULL;"
                )));
            }
        }

        stmts
    }

    fn create_index(&self, m: &Model, ix: &IndexDef) -> Vec<Stmt> {
        let cols = render_index_columns(self, m, ix);
        vec![Stmt::new(format!(
            "CREATE INDEX {} ON {} ({});",
            self.quote_ident(&ix.db_name),
            self.quote_ident(&m.table),
            cols
        ))]
    }

    fn drop_index(&self, _table: &str, name: &str) -> Vec<Stmt> {
        vec![Stmt::new(format!(
            "DROP INDEX IF EXISTS {};",
            self.quote_ident(name)
        ))]
    }

    fn add_foreign_key(&self, m: &Model, r: &ResolvedRelation) -> Vec<Stmt> {
        let table = self.quote_ident(&m.table);
        let constraint = fk_constraint_sql(self, r);
        vec![Stmt::new(format!("ALTER TABLE {table} ADD {constraint};"))]
    }

    fn create_enum(&self, e: &EnumDef) -> Vec<Stmt> {
        let name = self.quote_ident(&e.db_name);
        let variants = e
            .variants
            .values()
            .map(|v| format!("'{}'", v.db_name.replace('\'', "''")))
            .collect::<Vec<_>>()
            .join(", ");
        vec![Stmt::new(format!("CREATE TYPE {name} AS ENUM ({variants});")).non_transactional()]
    }

    fn alter_enum_add_variant(&self, e: &EnumDef, v: &str) -> Vec<Stmt> {
        let name = self.quote_ident(&e.db_name);
        let escaped = v.replace('\'', "''");
        vec![Stmt::new(format!("ALTER TYPE {name} ADD VALUE '{escaped}';")).non_transactional()]
    }

    fn returning_supported(&self) -> bool {
        true
    }

    fn upsert_clause(&self, conflict: &[String], update: &[String]) -> String {
        if update.is_empty() {
            format!("ON CONFLICT ({}) DO NOTHING", conflict.join(", "))
        } else {
            let quoted_update = update
                .iter()
                .map(|c| format!("{} = EXCLUDED.{}", self.quote_ident(c), self.quote_ident(c)))
                .collect::<Vec<_>>()
                .join(", ");
            format!(
                "ON CONFLICT ({}) DO UPDATE SET {}",
                conflict.join(", "),
                quoted_update
            )
        }
    }

    fn limit_offset(&self, limit: Option<u64>, offset: Option<u64>) -> String {
        let mut parts = Vec::new();
        if let Some(l) = limit {
            parts.push(format!("LIMIT {l}"));
        }
        if let Some(o) = offset {
            parts.push(format!("OFFSET {o}"));
        }
        parts.join(" ")
    }

    fn cast_expr(&self, expr: &str, ty: ScalarType) -> String {
        format!("{}::{}", expr, pg_type_name(ty))
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities {
            native_enums: true,
            native_uuid: true,
            alter_column_type: true,
            drop_column: true,
            add_fk_after_create: true,
            returning: true,
            partial_indexes: true,
            deferrable_fks: true,
            json_type: JsonSupport::Native,
            max_query_params: 65_535,
        }
    }
}

fn pg_type_name(ty: ScalarType) -> &'static str {
    match ty {
        ScalarType::String => "TEXT",
        ScalarType::Int => "INTEGER",
        ScalarType::BigInt => "BIGINT",
        ScalarType::Float => "DOUBLE PRECISION",
        ScalarType::Decimal => "NUMERIC",
        ScalarType::Boolean => "BOOLEAN",
        ScalarType::DateTime => "TIMESTAMPTZ",
        ScalarType::Date => "DATE",
        ScalarType::Time => "TIME",
        ScalarType::Uuid => "UUID",
        ScalarType::Json => "JSONB",
        ScalarType::Bytes => "BYTEA",
    }
}
