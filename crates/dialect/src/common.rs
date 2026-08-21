//! Shared helpers for SQL dialect implementations.

use ruprizzle_core::SchemaError;
use ruprizzle_core::ir::{
    DefaultFn, DefaultValue, Field, FieldKind, Literal, Model, Provider, RelationKind,
    ResolvedRelation, ScalarType, Schema, SortOrder,
};

use crate::{DbDialect, DialectError, RustType, Stmt};

static POSTGRES_DIALECT: crate::PostgresDialect = crate::PostgresDialect;
static SQLITE_DIALECT: crate::SqliteDialect = crate::SqliteDialect;
static MYSQL_DIALECT: crate::MySqlDialect = crate::MySqlDialect;

/// Returns the dialect implementation for a provider.
///
/// The returned reference is `'static'`; the dialect implementations are
/// zero-sized and live for the life of the process, so callers do not need to
/// box or clone them.
#[must_use]
pub fn dialect_for(provider: Provider) -> &'static dyn DbDialect {
    match provider {
        Provider::Postgres => &POSTGRES_DIALECT,
        Provider::Sqlite => &SQLITE_DIALECT,
        Provider::Mysql => &MYSQL_DIALECT,
    }
}

/// Creates the full set of statements needed to create a model, including
/// foreign keys, for the initial schema creation.
///
/// * PostgreSQL emits `CREATE TABLE` followed by one `ALTER TABLE` per owned
///   relation.
/// * SQLite embeds the foreign key constraints in the `CREATE TABLE` because it
///   cannot add them later.
#[must_use]
pub fn full_create_table(dialect: &dyn DbDialect, schema: &Schema, m: &Model) -> Vec<Stmt> {
    let mut stmts = dialect.create_table(schema, m);

    let owned: Vec<&ResolvedRelation> = schema
        .relations
        .iter()
        .filter(|r| r.owner == m.name && r.kind != RelationKind::ManyToMany)
        .collect();

    if owned.is_empty() {
        return stmts;
    }

    if dialect.capabilities().add_fk_after_create {
        for r in owned {
            stmts.extend(dialect.add_foreign_key(m, r));
        }
        return stmts;
    }

    // SQLite: foreign keys must be inline. The last statement is the
    // CREATE TABLE body; rewrite it to include the constraints.
    if let Some(last) = stmts.last_mut() {
        let body = last
            .sql
            .trim_end()
            .strip_suffix(';')
            .unwrap_or(&last.sql)
            .to_owned();

        let mut fks = Vec::new();
        for r in owned {
            for c in dialect.add_foreign_key(m, r) {
                fks.push(c.sql);
            }
        }

        if !fks.is_empty() {
            if let Some(idx) = body.rfind(')') {
                let before = &body[..idx];
                let after = &body[idx..];
                let joined = fks.join(", ");
                last.sql = format!("{before}, {joined}{after};");
            }
        }
    }

    stmts
}

/// Creates the full sequence of statements needed to change a column on SQLite,
/// including the table rebuild, index recreation, and foreign-key recreation.
///
/// On PostgreSQL this delegates directly to `alter_column`.
///
/// The `source` model is the table as it exists before the change. For a single
/// column change it can be the same as `target`; for sequential changes within one
/// migration it should reflect the table state after all earlier changes have been
/// applied, so the table rebuild does not reference columns that do not yet exist.
#[must_use]
pub fn full_alter_column(
    dialect: &dyn DbDialect,
    schema: &Schema,
    m: &Model,
    from: &Field,
    to: &Field,
) -> Vec<Stmt> {
    full_alter_column_with_source(dialect, schema, m, m, from, to)
}

/// [`full_alter_column`] with an explicit source model.
#[must_use]
pub fn full_alter_column_with_source(
    dialect: &dyn DbDialect,
    schema: &Schema,
    target: &Model,
    source: &Model,
    from: &Field,
    to: &Field,
) -> Vec<Stmt> {
    if dialect.capabilities().alter_column_type {
        return dialect.alter_column(schema, target, from, to);
    }

    crate::sqlite::rebuild_table(dialect, schema, target, source, from, to)
}

/// Checks a schema for constructs that the active provider handles poorly.
///
/// Returns advisory diagnostics (warnings) for cases such as `Decimal` on SQLite
/// or `Json` query limitations. These are V18 from ImplPlan02.
#[must_use]
pub fn check_schema_capabilities(dialect: &dyn DbDialect, schema: &Schema) -> Vec<SchemaError> {
    let mut out = Vec::new();
    let cap = dialect.capabilities();

    for model in schema.models.values() {
        for field in model.fields.values() {
            if !field.has_column() {
                continue;
            }

            match &field.kind {
                FieldKind::Scalar(ScalarType::Decimal)
                    if !cap.native_uuid && dialect.name() == "sqlite" =>
                {
                    out.push(SchemaError::DialectDegraded {
                        construct: "Decimal".to_owned(),
                        provider: dialect.name().to_owned(),
                        advice: Some(
                            "for money on SQLite, consider Int storing minor units (cents)"
                                .to_owned(),
                        ),
                        span: field.span.into(),
                        consequence:
                            "stored as TEXT; SQL arithmetic and ordering are lexicographic"
                                .to_owned(),
                    });
                }
                FieldKind::Scalar(ScalarType::Json)
                    if matches!(
                        cap.json_type,
                        crate::JsonSupport::TextEncoded | crate::JsonSupport::None
                    ) && dialect.name() == "sqlite" =>
                {
                    out.push(SchemaError::DialectDegraded {
                        construct: "Json".to_owned(),
                        provider: dialect.name().to_owned(),
                        advice: Some(
                            "use Postgres for JSONB operators or store a string and parse it manually".to_owned(),
                        ),
                        span: field.span.into(),
                        consequence: "stored as TEXT; JSON path queries are not supported".to_owned(),
                    });
                }
                FieldKind::Enum(_) if !cap.native_enums && dialect.name() == "sqlite" => {
                    out.push(SchemaError::DialectDegraded {
                        construct: format!("enum column on {}", model.name),
                        provider: dialect.name().to_owned(),
                        advice: Some(
                            "values are enforced by a CHECK constraint; adding variants requires a table rebuild".to_owned(),
                        ),
                        span: field.span.into(),
                        consequence: "enum is emulated as TEXT with a CHECK constraint".to_owned(),
                    });
                }
                _ => {}
            }

            if let Some(ref nt) = field.attrs.native_type {
                if dialect.column_type(field).is_err() {
                    // column_type only errors for unsupported native types.
                    out.push(SchemaError::DialectDegraded {
                        construct: format!("@db.{}", nt.name),
                        provider: dialect.name().to_owned(),
                        advice: Some(format!(
                            "remove `@db.{}` or use a type supported by {}",
                            nt.name,
                            dialect.name()
                        )),
                        span: nt.span.into(),
                        consequence: format!(
                            "`@db.{}` is not supported on {}",
                            nt.name,
                            dialect.name()
                        ),
                    });
                }
            }
        }
    }

    out
}

// ---------------------------------------------------------------------------
// Common SQL building blocks.
// ---------------------------------------------------------------------------

/// SQL fragment for a `FOREIGN KEY` table constraint.
pub(crate) fn fk_constraint_sql(dialect: &dyn DbDialect, r: &ResolvedRelation) -> String {
    let quoted_owner_cols = r
        .owner_cols
        .iter()
        .map(|c| dialect.quote_ident(c))
        .collect::<Vec<_>>()
        .join(", ");
    let quoted_target = dialect.quote_ident(&r.target_table);
    let quoted_target_cols = r
        .target_cols
        .iter()
        .map(|c| dialect.quote_ident(c))
        .collect::<Vec<_>>()
        .join(", ");

    let constraint = dialect.quote_ident(&r.constraint_name);
    let mut sql = format!(
        "CONSTRAINT {constraint} FOREIGN KEY ({quoted_owner_cols}) REFERENCES {quoted_target} ({quoted_target_cols})"
    );

    if r.on_delete != ruprizzle_core::ir::ReferentialAction::NoAction {
        let _ = std::fmt::Write::write_fmt(
            &mut sql,
            format_args!(" ON DELETE {}", r.on_delete.as_sql()),
        );
    }
    if r.on_update != ruprizzle_core::ir::ReferentialAction::NoAction {
        let _ = std::fmt::Write::write_fmt(
            &mut sql,
            format_args!(" ON UPDATE {}", r.on_update.as_sql()),
        );
    }

    if dialect.capabilities().deferrable_fks {
        sql.push_str(" DEFERRABLE INITIALLY IMMEDIATE");
    }

    sql
}

/// Escapes a single quote in a string literal.
#[must_use]
pub(crate) fn escape_literal(s: &str) -> String {
    s.replace('\'', "''")
}

/// SQL expression for a literal value.
pub(crate) fn literal_sql(dialect: &dyn DbDialect, lit: &Literal) -> String {
    match lit {
        Literal::String(s) => format!("'{}'", escape_literal(s)),
        Literal::Int(n) => n.to_string(),
        Literal::Float(n) => n.to_string(),
        Literal::Bool(b) => bool_sql(dialect, *b),
        Literal::EnumVariant(v) => format!("'{}'", escape_literal(v)),
    }
}

fn bool_sql(dialect: &dyn DbDialect, b: bool) -> String {
    if dialect.name() == "sqlite" || dialect.name() == "mysql" {
        i32::from(b).to_string()
    } else if b {
        "true".to_owned()
    } else {
        "false".to_owned()
    }
}

/// SQL expression for a default value, excluding the `DEFAULT` keyword.
pub(crate) fn default_sql(dialect: &dyn DbDialect, f: &Field) -> String {
    let Some(ref d) = f.default else {
        return String::new();
    };

    match d {
        DefaultValue::Literal(lit) => literal_sql(dialect, lit),
        DefaultValue::Function(func) => match (func, dialect.name()) {
            (DefaultFn::Uuid4, "postgres") => "gen_random_uuid()".to_owned(),
            (DefaultFn::Uuid4, "mysql") => "UUID()".to_owned(),
            (DefaultFn::Now, "postgres" | "mysql") => "NOW()".to_owned(),
            (DefaultFn::Now, "sqlite") => "(datetime('now'))".to_owned(),
            (DefaultFn::Now, _) => "datetime('now')".to_owned(),
            (
                DefaultFn::Uuid4
                | DefaultFn::Uuid7
                | DefaultFn::Cuid2
                | DefaultFn::Nanoid
                | DefaultFn::AutoIncrement,
                _,
            ) => String::new(),
        },
        DefaultValue::DbGenerated(sql) => sql.clone(),
    }
}

/// Builds the Rust type for a field.
#[must_use]
pub(crate) fn rust_type_for(f: &Field) -> RustType {
    fn kind_to_rust(kind: &FieldKind) -> RustType {
        match kind {
            FieldKind::Scalar(ScalarType::String) | FieldKind::Relation(_) => RustType::String,
            FieldKind::Scalar(ScalarType::Int) => RustType::Int,
            FieldKind::Scalar(ScalarType::BigInt) => RustType::BigInt,
            FieldKind::Scalar(ScalarType::Float) => RustType::Float,
            FieldKind::Scalar(ScalarType::Decimal) => RustType::Decimal,
            FieldKind::Scalar(ScalarType::Boolean) => RustType::Boolean,
            FieldKind::Scalar(ScalarType::DateTime) => RustType::DateTime,
            FieldKind::Scalar(ScalarType::Date) => RustType::Date,
            FieldKind::Scalar(ScalarType::Time) => RustType::Time,
            FieldKind::Scalar(ScalarType::Uuid) => RustType::Uuid,
            FieldKind::Scalar(ScalarType::Json) => RustType::Json,
            FieldKind::Scalar(ScalarType::Bytes) => RustType::Bytes,
            FieldKind::Enum(name) => RustType::Enum(name.as_str().to_owned()),
            FieldKind::List(inner) => RustType::Vec(Box::new(kind_to_rust(inner))),
        }
    }

    let base = kind_to_rust(&f.kind);

    if f.optional {
        RustType::Option(Box::new(base))
    } else {
        base
    }
}

/// A column declaration while it is being built.
pub(crate) struct ColumnSpec {
    /// The quoted column name.
    pub quoted_name: String,
    /// The SQL type.
    pub sql_type: String,
    /// Whether the column is `NOT NULL`.
    pub not_null: bool,
    /// The `DEFAULT` expression, if any.
    pub default: Option<String>,
    /// Whether the column is `PRIMARY KEY`.
    pub primary_key: bool,
    /// Whether the column is `UNIQUE`.
    pub unique: bool,
    /// Whether the column has an identity clause.
    pub identity: bool,
    /// An optional generated-column clause.
    pub generated: Option<ruprizzle_core::ir::GeneratedClause>,
    /// An optional `CHECK` constraint.
    pub check: Option<String>,
}

impl ColumnSpec {
    /// Renders the full column declaration for a `CREATE TABLE`.
    #[must_use]
    pub fn render(&self, dialect: &dyn DbDialect) -> String {
        format!("{} {}", self.quoted_name, self.render_body(dialect))
    }

    /// Renders the column definition (type, constraints, etc.) without the name.
    ///
    /// Used by dialects that rewrite an existing column, such as MySQL's
    /// `MODIFY COLUMN` and `CHANGE COLUMN`, where the name is stated separately.
    #[must_use]
    pub fn render_body(&self, dialect: &dyn DbDialect) -> String {
        let mut parts = vec![self.sql_type.clone()];

        // Postgres uses `GENERATED BY DEFAULT AS IDENTITY` in the usual
        // position. MySQL and SQLite add the autoincrement keyword next to
        // `PRIMARY KEY` below.
        if self.identity && dialect.name() != "sqlite" && dialect.name() != "mysql" {
            parts.push(identity_clause(dialect));
        }

        if let Some(ref g) = self.generated {
            let kind = match g.kind {
                ruprizzle_core::ir::GeneratedKind::Virtual => "VIRTUAL",
                ruprizzle_core::ir::GeneratedKind::Stored => "STORED",
            };
            parts.push(format!("GENERATED ALWAYS AS ({}) {}", g.expr, kind));
        }

        if self.not_null {
            parts.push("NOT NULL".to_owned());
        } else if !self.primary_key {
            parts.push("NULL".to_owned());
        }

        if let Some(ref d) = self.default {
            parts.push(format!("DEFAULT {d}"));
        }

        if self.primary_key {
            parts.push("PRIMARY KEY".to_owned());
        }

        if self.identity && (dialect.name() == "sqlite" || dialect.name() == "mysql") {
            if dialect.name() == "mysql" {
                parts.push("AUTO_INCREMENT".to_owned());
            } else {
                parts.push("AUTOINCREMENT".to_owned());
            }
        }

        if self.unique && !self.primary_key {
            parts.push("UNIQUE".to_owned());
        }

        if let Some(ref c) = self.check {
            parts.push(format!("CHECK ({c})"));
        }

        parts.join(" ")
    }
}

/// Returns the identity clause for an autoincrement column.
pub(crate) fn identity_clause(dialect: &dyn DbDialect) -> String {
    if dialect.name() == "sqlite" || dialect.name() == "mysql" {
        // MySQL and SQLite add AUTOINCREMENT/AUTO_INCREMENT next to PRIMARY KEY
        // in `ColumnSpec::render` instead of the usual position.
        String::new()
    } else {
        "GENERATED BY DEFAULT AS IDENTITY".to_owned()
    }
}

/// Builds a [`ColumnSpec`] for a field.
pub(crate) fn column_spec(
    dialect: &dyn DbDialect,
    schema: &Schema,
    f: &Field,
) -> Result<ColumnSpec, DialectError> {
    let sql_type = dialect.column_type(f)?;

    let (not_null, default) = match f.default {
        Some(DefaultValue::Function(func)) if func.is_client_side() => (!f.optional, None),
        Some(_) => {
            let sql = default_sql(dialect, f);
            (!f.optional, if sql.is_empty() { None } else { Some(sql) })
        }
        None => (!f.optional, None),
    };

    let identity = f
        .default
        .as_ref()
        .is_some_and(|d| matches!(d, DefaultValue::Function(DefaultFn::AutoIncrement)));

    let check = enum_check(dialect, schema, f);

    Ok(ColumnSpec {
        quoted_name: dialect.quote_ident(&f.column),
        sql_type,
        not_null,
        default,
        primary_key: f.attrs.is_id,
        unique: f.attrs.is_unique && !f.attrs.is_id,
        identity,
        generated: f.generated.clone(),
        check,
    })
}

/// An optional CHECK constraint for an enum column on dialects without native enums.
pub(crate) fn enum_check(dialect: &dyn DbDialect, schema: &Schema, f: &Field) -> Option<String> {
    if let FieldKind::Enum(ref name) = f.kind {
        if !dialect.capabilities().native_enums {
            let variants = schema
                .enums
                .get(name.as_str())
                .map(|e| {
                    e.variants
                        .values()
                        .map(|v| format!("'{}'", escape_literal(&v.db_name)))
                        .collect::<Vec<_>>()
                        .join(", ")
                })
                .unwrap_or_default();
            if !variants.is_empty() {
                let col = dialect.quote_ident(&f.column);
                return Some(format!("{col} IN ({variants})"));
            }
        }
    }
    None
}

/// Returns the SQL type string for a field, handling common native types.
pub(crate) fn base_column_type(
    dialect_name: &str,
    f: &Field,
    without_native: impl FnOnce(&Field) -> String,
) -> Result<String, DialectError> {
    if let Some(ref nt) = f.attrs.native_type {
        native_type_sql(dialect_name, f, nt.name.as_str(), &nt.args)
    } else {
        Ok(without_native(f))
    }
}

fn native_type_sql(
    dialect_name: &str,
    f: &Field,
    name: &str,
    args: &[String],
) -> Result<String, DialectError> {
    let err = |reason: &str| {
        Err(DialectError::InvalidNativeArg {
            name: name.to_owned(),
            args: args.join(", "),
            reason: reason.to_owned(),
        })
    };

    match name {
        "VarChar" => match args.len() {
            1 => {
                if dialect_name == "sqlite" {
                    return Ok("TEXT".to_owned());
                }
                let n = args[0]
                    .parse::<usize>()
                    .map_err(|_| DialectError::InvalidNativeArg {
                        name: name.to_owned(),
                        args: args.join(", "),
                        reason: "expected a length".to_owned(),
                    })?;
                Ok(format!("VARCHAR({n})"))
            }
            _ => err("expected one length argument"),
        },
        "SmallInt" => {
            if !args.is_empty() {
                return err("takes no arguments");
            }
            if dialect_name == "sqlite" {
                Ok("INTEGER".to_owned())
            } else {
                Ok("SMALLINT".to_owned())
            }
        }
        "Decimal" => match args.len() {
            2 => {
                let p = args[0]
                    .parse::<usize>()
                    .map_err(|_| DialectError::InvalidNativeArg {
                        name: name.to_owned(),
                        args: args.join(", "),
                        reason: "expected precision".to_owned(),
                    })?;
                let s = args[1]
                    .parse::<usize>()
                    .map_err(|_| DialectError::InvalidNativeArg {
                        name: name.to_owned(),
                        args: args.join(", "),
                        reason: "expected scale".to_owned(),
                    })?;
                if dialect_name == "sqlite" {
                    Ok("TEXT".to_owned())
                } else {
                    Ok(format!("NUMERIC({p},{s})"))
                }
            }
            0 if dialect_name == "sqlite" => Ok("TEXT".to_owned()),
            0 => Ok("NUMERIC".to_owned()),
            _ => err("expected zero or two arguments: precision and scale"),
        },
        _ => Err(DialectError::UnsupportedNativeType {
            dialect: dialect_name.to_owned(),
            name: name.to_owned(),
            column: f.column.clone(),
        }),
    }
}

/// Builds a `CREATE TABLE` body without foreign keys.
///
/// This is shared by both dialects; they only differ in how they quote
/// identifiers and render types.
pub(crate) fn create_table_body(
    dialect: &dyn DbDialect,
    schema: &Schema,
    m: &Model,
) -> Result<Stmt, DialectError> {
    let mut columns = Vec::new();
    for f in m.scalar_fields() {
        columns.push(column_spec(dialect, schema, f)?.render(dialect));
    }

    // Block-level primary key for composite @@id.
    if m.primary_key.is_composite() {
        let pk_cols = quote_field_columns(dialect, m, &m.primary_key.fields).join(", ");
        let pk_name = m
            .primary_key
            .name
            .clone()
            .unwrap_or_else(|| format!("{}_pkey", m.table));
        columns.push(format!(
            "CONSTRAINT {} PRIMARY KEY ({})",
            dialect.quote_ident(&pk_name),
            pk_cols
        ));
    }

    // Table-level unique constraints.
    for u in &m.uniques {
        let cols = render_index_targets(dialect, m, &u.targets);
        columns.push(format!(
            "CONSTRAINT {} UNIQUE ({})",
            dialect.quote_ident(&u.db_name),
            cols
        ));
    }

    let table = dialect.quote_ident(&m.table);
    let body = columns.join(", ");
    Ok(Stmt::new(format!("CREATE TABLE {table} ({body});")))
}

/// Quoted physical column names for a set of field names.
pub(crate) fn quote_field_columns(
    dialect: &dyn DbDialect,
    m: &Model,
    fields: &[ruprizzle_core::names::FieldName],
) -> Vec<String> {
    fields
        .iter()
        .map(|n| {
            m.fields
                .get(n.as_str())
                .map_or_else(|| n.to_string(), |f| dialect.quote_ident(&f.column))
        })
        .collect()
}

/// Renders the targets of an index or unique constraint.
pub(crate) fn render_index_targets(
    dialect: &dyn DbDialect,
    m: &Model,
    targets: &[ruprizzle_core::ir::IndexTarget],
) -> String {
    use ruprizzle_core::ir::IndexTarget;
    targets
        .iter()
        .map(|target| match target {
            IndexTarget::Field(name, order) => {
                let col = m
                    .fields
                    .get(name.as_str())
                    .map_or_else(|| name.to_string(), |f| f.column.clone());
                let quoted = dialect.quote_ident(&col);
                match order {
                    SortOrder::Asc => quoted,
                    SortOrder::Desc => format!("{quoted} DESC"),
                }
            }
            IndexTarget::Expression(expr) => expr.clone(),
        })
        .collect::<Vec<_>>()
        .join(", ")
}
