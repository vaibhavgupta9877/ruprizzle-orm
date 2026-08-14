//! MySQL / MariaDB dialect.
//!
//! Targets MySQL 8.0+ and MariaDB 10.5+. The dialect is fully implemented for
//! DDL, migrations, and runtime DML. Because neither supported server family
//! has a portable DML `RETURNING` clause, inserts use a follow-up primary-key
//! lookup in the runtime.

use ruprizzle_core::ir::{
    EnumDef, Field, FieldKind, IndexDef, Model, ResolvedRelation, ScalarType, Schema, UniqueDef,
};

use crate::common::{
    base_column_type, column_spec, create_table_body, fk_constraint_sql, quote_field_columns,
    render_index_columns, rust_type_for,
};
use crate::{Capabilities, DbDialect, DialectError, JsonSupport, RustType, Stmt};

/// The MySQL / MariaDB dialect implementation.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct MySqlDialect;

impl DbDialect for MySqlDialect {
    fn name(&self) -> &'static str {
        "mysql"
    }

    fn quote_ident(&self, s: &str) -> String {
        format!("`{}`", s.replace('`', "``"))
    }

    fn placeholder(&self, _index: usize) -> String {
        "?".to_owned()
    }

    fn column_type(&self, f: &Field) -> Result<String, DialectError> {
        base_column_type("mysql", f, |f| match f.kind {
            FieldKind::Scalar(ScalarType::String) => {
                if f.attrs.is_id || f.attrs.is_unique {
                    "VARCHAR(191)".to_owned()
                } else {
                    "VARCHAR(255)".to_owned()
                }
            }
            FieldKind::Scalar(ScalarType::Decimal) => "DECIMAL(65,30)".to_owned(),
            FieldKind::Scalar(ScalarType::DateTime) => "DATETIME(6)".to_owned(),
            FieldKind::Scalar(ScalarType::Date) => "DATE".to_owned(),
            FieldKind::Scalar(ScalarType::Time) => "TIME".to_owned(),
            FieldKind::Scalar(ScalarType::Uuid) => "CHAR(36)".to_owned(),
            FieldKind::Scalar(ScalarType::Json) => "JSON".to_owned(),
            FieldKind::Enum(_) | FieldKind::Relation(_) | FieldKind::List(_) => {
                "VARCHAR(255)".to_owned()
            }
            FieldKind::Scalar(ScalarType::Int) => "INT".to_owned(),
            FieldKind::Scalar(ScalarType::BigInt) => "BIGINT".to_owned(),
            FieldKind::Scalar(ScalarType::Float) => "DOUBLE".to_owned(),
            FieldKind::Scalar(ScalarType::Boolean) => "TINYINT(1)".to_owned(),
            FieldKind::Scalar(ScalarType::Bytes) => "BLOB".to_owned(),
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

    fn add_column(&self, schema: &Schema, m: &Model, f: &Field) -> Vec<Stmt> {
        match column_spec(self, schema, f) {
            Ok(mut spec) => {
                // Unique constraints are added in a separate migration step.
                spec.unique = false;
                vec![Stmt::new(format!(
                    "ALTER TABLE {} ADD COLUMN {};",
                    self.quote_ident(&m.table),
                    spec.render(self)
                ))]
            }
            Err(e) => vec![Stmt::new(format!("-- error: {e}"))],
        }
    }

    fn drop_column(&self, table: &str, col: &str) -> Vec<Stmt> {
        vec![Stmt::new(format!(
            "ALTER TABLE {} DROP COLUMN {};",
            self.quote_ident(table),
            self.quote_ident(col)
        ))]
    }

    fn alter_column(&self, schema: &Schema, m: &Model, from: &Field, to: &Field) -> Vec<Stmt> {
        let table = self.quote_ident(&m.table);
        let from_col = self.quote_ident(&from.column);
        let to_col = self.quote_ident(&to.column);

        let mut spec = match column_spec(self, schema, to) {
            Ok(s) => s,
            Err(e) => return vec![Stmt::new(format!("-- error: {e}"))],
        };
        // Unique / primary-key maintenance is a separate migration step.
        spec.unique = false;
        let body = spec.render_body(self);

        if from.column == to.column {
            vec![Stmt::new(format!(
                "ALTER TABLE {table} MODIFY COLUMN {to_col} {body};"
            ))]
        } else {
            vec![Stmt::new(format!(
                "ALTER TABLE {table} CHANGE COLUMN {from_col} {to_col} {body};"
            ))]
        }
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

    fn drop_index(&self, table: &str, name: &str) -> Vec<Stmt> {
        vec![Stmt::new(format!(
            "DROP INDEX {} ON {};",
            self.quote_ident(name),
            self.quote_ident(table)
        ))]
    }

    fn add_unique(&self, m: &Model, uq: &UniqueDef) -> Vec<Stmt> {
        let table = self.quote_ident(&m.table);
        let name = self.quote_ident(&uq.db_name);
        let cols = quote_field_columns(self, m, &uq.fields).join(", ");
        vec![Stmt::new(format!(
            "CREATE UNIQUE INDEX {name} ON {table} ({cols});"
        ))]
    }

    fn drop_unique(&self, table: &str, name: &str) -> Vec<Stmt> {
        vec![Stmt::new(format!(
            "DROP INDEX {} ON {};",
            self.quote_ident(name),
            self.quote_ident(table)
        ))]
    }

    fn add_foreign_key(&self, m: &Model, r: &ResolvedRelation) -> Vec<Stmt> {
        let table = self.quote_ident(&m.table);
        let constraint = fk_constraint_sql(self, r);
        vec![Stmt::new(format!("ALTER TABLE {table} ADD {constraint};"))]
    }

    fn drop_foreign_key(&self, m: &Model, r: &ResolvedRelation) -> Vec<Stmt> {
        let table = self.quote_ident(&m.table);
        let name = self.quote_ident(&r.constraint_name);
        vec![Stmt::new(format!(
            "ALTER TABLE {table} DROP FOREIGN KEY {name};"
        ))]
    }

    fn create_enum(&self, _e: &EnumDef) -> Vec<Stmt> {
        // MySQL enum variants are inline in the column definition; the CHECK
        // constraint is used for validation when native enums are disabled.
        Vec::new()
    }

    fn alter_enum_add_variant(&self, e: &EnumDef, _v: &str) -> Vec<Stmt> {
        // Adding a variant requires updating the CHECK constraint on every
        // column that uses this enum. The migration planner uses
        // `full_alter_column` to rewrite those columns.
        let _ = e;
        Vec::new()
    }

    fn returning_supported(&self) -> bool {
        // MySQL has no DML RETURNING clause. The runtime performs an explicit
        // follow-up SELECT by primary key after an insert.
        false
    }

    fn upsert_clause(&self, conflict: &[String], update: &[String]) -> String {
        if update.is_empty() {
            // No-op update on the conflict key to get DO NOTHING semantics.
            let first = conflict.first().map_or_else(|| "1", String::as_str);
            let first = self.quote_ident(first);
            return format!("ON DUPLICATE KEY UPDATE {first} = {first}");
        }

        let assignments = update
            .iter()
            .map(|c| {
                let quoted = self.quote_ident(c);
                format!("{quoted} = VALUES({quoted})")
            })
            .collect::<Vec<_>>()
            .join(", ");
        format!("ON DUPLICATE KEY UPDATE {assignments}")
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
        format!("CAST({} AS {})", expr, mysql_type_name(ty))
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities {
            native_enums: false,
            native_uuid: false,
            alter_column_type: true,
            drop_column: true,
            add_fk_after_create: true,
            returning: false,
            partial_indexes: false,
            deferrable_fks: false,
            json_type: JsonSupport::Native,
            max_query_params: 65_535,
            window_functions: true,
        }
    }

    fn supports_full_join(&self) -> bool {
        false
    }
}

fn mysql_type_name(ty: ScalarType) -> &'static str {
    match ty {
        ScalarType::String => "CHAR(255)",
        ScalarType::Int | ScalarType::BigInt => "SIGNED",
        ScalarType::Float => "DOUBLE",
        ScalarType::Decimal => "DECIMAL(65,30)",
        ScalarType::Boolean => "UNSIGNED",
        ScalarType::DateTime => "DATETIME(6)",
        ScalarType::Date => "DATE",
        ScalarType::Time => "TIME",
        ScalarType::Uuid => "CHAR(36)",
        ScalarType::Json => "JSON",
        ScalarType::Bytes => "BINARY",
    }
}
