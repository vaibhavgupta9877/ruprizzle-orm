//! SQLite dialect.

use ruprizzle_core::ir::{
    EnumDef, Field, FieldKind, IndexDef, Model, ResolvedRelation, ScalarType, Schema,
};

use crate::common::{
    base_column_type, column_spec, create_table_body, fk_constraint_sql, quote_field_columns,
    render_index_columns, rust_type_for,
};
use crate::{Capabilities, DbDialect, DialectError, JsonSupport, RustType, Stmt};

/// The SQLite dialect implementation.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SqliteDialect;

impl DbDialect for SqliteDialect {
    fn name(&self) -> &'static str {
        "sqlite"
    }

    fn quote_ident(&self, s: &str) -> String {
        format!("`{}`", s.replace('`', "``"))
    }

    fn placeholder(&self, _index: usize) -> String {
        "?".to_owned()
    }

    fn column_type(&self, f: &Field) -> Result<String, DialectError> {
        base_column_type("sqlite", f, |f| match f.kind {
            FieldKind::Scalar(
                ScalarType::String
                | ScalarType::Decimal
                | ScalarType::DateTime
                | ScalarType::Date
                | ScalarType::Time
                | ScalarType::Uuid
                | ScalarType::Json,
            )
            | FieldKind::Enum(_)
            | FieldKind::Relation(_)
            | FieldKind::List(_) => "TEXT".to_owned(),
            FieldKind::Scalar(ScalarType::Int | ScalarType::BigInt | ScalarType::Boolean) => {
                "INTEGER".to_owned()
            }
            FieldKind::Scalar(ScalarType::Float) => "REAL".to_owned(),
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
            Ok(spec) => vec![Stmt::new(format!(
                "ALTER TABLE {} ADD COLUMN {};",
                self.quote_ident(&m.table),
                spec.render(self)
            ))],
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

    fn alter_column(&self, _schema: &Schema, _m: &Model, _from: &Field, _to: &Field) -> Vec<Stmt> {
        // SQLite cannot alter a column directly. Use `full_alter_column`.
        Vec::new()
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
        // SQLite cannot add a foreign key with ALTER TABLE. Return the inline
        // constraint so `full_create_table` can embed it in CREATE TABLE.
        let _ = m;
        vec![Stmt::new(fk_constraint_sql(self, r))]
    }

    fn create_enum(&self, _e: &EnumDef) -> Vec<Stmt> {
        // SQLite has no enum type; values are enforced by CHECK constraints.
        Vec::new()
    }

    fn alter_enum_add_variant(&self, e: &EnumDef, _v: &str) -> Vec<Stmt> {
        // Adding a variant requires rebuilding the CHECK constraint on every
        // table that uses this enum. The migration planner handles that.
        let _ = e;
        Vec::new()
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
                .map(|c| format!("{} = excluded.{}", self.quote_ident(c), self.quote_ident(c)))
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
        format!("CAST({} AS {})", expr, sqlite_type_name(ty))
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities {
            native_enums: false,
            native_uuid: false,
            alter_column_type: false,
            drop_column: true,
            add_fk_after_create: false,
            returning: true,
            partial_indexes: true,
            deferrable_fks: true,
            json_type: JsonSupport::TextEncoded,
            max_query_params: 32_766,
        }
    }
}

fn sqlite_type_name(ty: ScalarType) -> &'static str {
    match ty {
        ScalarType::String
        | ScalarType::Decimal
        | ScalarType::DateTime
        | ScalarType::Date
        | ScalarType::Time
        | ScalarType::Uuid
        | ScalarType::Json => "TEXT",
        ScalarType::Int | ScalarType::BigInt | ScalarType::Boolean => "INTEGER",
        ScalarType::Float => "REAL",
        ScalarType::Bytes => "BLOB",
    }
}

// ---------------------------------------------------------------------------
// SQLite table rebuild.
// ---------------------------------------------------------------------------

/// Rebuilds a SQLite table to alter a column, preserving as much data as
/// possible.
///
/// This is the 12-step sequence from ImplPlan03:
///
/// 1. Turn foreign keys off.
/// 2. Create the new table.
/// 3. Copy the intersection of old and new columns.
/// 4. Drop the old table.
/// 5. Rename the new table.
/// 6. Recreate indexes.
/// 7. Recreate foreign keys inline (they were in the original `CREATE TABLE`).
/// 8. Turn foreign keys on and verify.
#[must_use]
pub(crate) fn rebuild_table(
    dialect: &dyn DbDialect,
    schema: &Schema,
    m: &Model,
    from: &Field,
    to: &Field,
) -> Vec<Stmt> {
    let table = &m.table;
    let new_table = format!("{table}__new");

    let mut stmts = Vec::new();
    stmts.push(Stmt::new("PRAGMA foreign_keys=OFF;".to_owned()).non_transactional());

    // Build the new table schema. Replace the old field with the new one.
    let mut new_model = m.clone();
    new_model.table.clone_from(&new_table);
    new_model.fields.shift_remove(from.name.as_str());
    new_model.fields.insert(to.name.clone(), to.clone());

    let create_new = create_table_body(dialect, schema, &new_model)
        .unwrap_or_else(|e| Stmt::new(format!("-- error: {e}")));
    stmts.push(create_new);

    // Foreign keys must be inline in the new CREATE TABLE.
    let owned: Vec<&ResolvedRelation> = schema
        .relations
        .iter()
        .filter(|r| r.owner == new_model.name)
        .collect();
    if let Some(last) = stmts.last_mut() {
        let body = last
            .sql
            .trim_end()
            .strip_suffix(';')
            .unwrap_or(&last.sql)
            .to_owned();
        let mut fks: Vec<String> = owned
            .iter()
            .map(|r| fk_constraint_sql(dialect, r))
            .collect();

        // Update the owner-side column if it is part of this relation and its
        // name changed.
        if from.column != to.column {
            for r in &owned {
                let mut updated = fk_constraint_sql(dialect, r);
                for col in &r.owner_cols {
                    if col == &from.column {
                        let old = dialect.quote_ident(col);
                        let new = dialect.quote_ident(&to.column);
                        updated = updated.replace(
                            &format!("FOREIGN KEY ({old})"),
                            &format!("FOREIGN KEY ({new})"),
                        );
                        break;
                    }
                }
                if let Some(pos) = fks.iter().position(|s| s.contains(&r.constraint_name)) {
                    fks[pos] = updated;
                }
            }
        }

        if !fks.is_empty() {
            if let Some(idx) = body.rfind(')') {
                let before = &body[..idx];
                let after = &body[idx..];
                last.sql = format!("{before}, {} {after};", fks.join(", "));
            }
        }
    }

    // Copy the intersection of old and new columns. If the column is being
    // renamed, use the old name on the source side and the new name on the
    // destination side.
    let mut old_cols = Vec::new();
    let mut new_cols = Vec::new();
    for f in m.scalar_fields() {
        if f.column == from.column && to.column != from.column {
            // The old column is being renamed. The destination is `to.column`
            // while the source is `from.column`.
            old_cols.push(dialect.quote_ident(&from.column));
            new_cols.push(dialect.quote_ident(&to.column));
        } else if new_model.fields.contains_key(f.name.as_str()) {
            let col = dialect.quote_ident(&f.column);
            old_cols.push(col.clone());
            new_cols.push(col);
        }
    }

    if !old_cols.is_empty() {
        stmts.push(Stmt::new(format!(
            "INSERT INTO {} ({}) SELECT {} FROM {};",
            dialect.quote_ident(&new_table),
            new_cols.join(", "),
            old_cols.join(", "),
            dialect.quote_ident(table)
        )));
    }

    stmts.push(Stmt::new(format!("DROP TABLE {};", dialect.quote_ident(table))).destructive());
    stmts.push(Stmt::new(format!(
        "ALTER TABLE {} RENAME TO {};",
        dialect.quote_ident(&new_table),
        dialect.quote_ident(table)
    )));

    // Recreate indexes that still apply to the new model.
    for ix in &m.indexes {
        // Drop the index if any of its columns no longer exist. We check by
        // looking up the new model fields.
        let missing = ix
            .fields
            .iter()
            .any(|idx_field| !new_model.fields.contains_key(idx_field.field.as_str()));
        if !missing {
            let cols = render_index_columns(dialect, &new_model, ix);
            stmts.push(Stmt::new(format!(
                "CREATE INDEX {} ON {} ({});",
                dialect.quote_ident(&ix.db_name),
                dialect.quote_ident(table),
                cols
            )));
        }
    }

    for u in &m.uniques {
        let missing = u
            .fields
            .iter()
            .any(|field_name| !new_model.fields.contains_key(field_name.as_str()));
        if !missing {
            let cols = quote_field_columns(dialect, &new_model, &u.fields).join(", ");
            stmts.push(Stmt::new(format!(
                "CREATE UNIQUE INDEX {} ON {} ({});",
                dialect.quote_ident(&u.db_name),
                dialect.quote_ident(table),
                cols
            )));
        }
    }

    stmts.push(Stmt::new("PRAGMA foreign_key_check;".to_owned()).non_transactional());
    stmts.push(Stmt::new("PRAGMA foreign_keys=ON;".to_owned()).non_transactional());

    stmts
}
