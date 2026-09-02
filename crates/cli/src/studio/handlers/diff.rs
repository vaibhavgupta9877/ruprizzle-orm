//! Migration safety diff and schema drift inspection handlers.
//!
//! This screen is consulted immediately before a destructive migration, so it must
//! never report safety it has not established. Every card below is derived from a
//! live `information_schema` / `PRAGMA` read plus, for anything that can lose data,
//! a `COUNT(*)` against the affected column. When the database cannot be reached
//! the page says so and renders no cards at all — it does not fall back to `SAFE`.

use askama::Template;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{Html, IntoResponse, Response};
use std::collections::BTreeSet;
use std::sync::Arc;

use ruprizzle::Pool;
use ruprizzle_core::ir::{Provider, Schema};
use ruprizzle_migrate::introspect::{DatabaseSchema, Table};

use super::{AppState, ModelNav};
use crate::studio::db;

/// Risk classification rendered as the card badge.
mod risk {
    /// The change cannot lose data.
    pub const SAFE: &str = "SAFE";
    /// The change may fail, or needs a human decision, but does not itself delete rows.
    pub const CAUTION: &str = "CAUTION";
    /// The change deletes data that exists in the database right now.
    pub const DESTRUCTIVE: &str = "DESTRUCTIVE";
}

#[derive(Debug, Clone)]
pub struct DiffChange {
    pub title: String,
    pub description: String,
    pub risk: String,
}

#[derive(Template)]
#[template(path = "diff/view.html")]
pub struct DiffViewTemplate<'a> {
    pub models: &'a [ModelNav],
    pub current_model: &'a str,
    pub provider: &'a str,
    pub allow_writes: bool,
    pub changes: Vec<DiffChange>,
    /// Set when no comparison could be made. Suppresses the "zero drift" banner.
    pub error: Option<String>,
}

/// Renders the migration safety diff view page.
pub async fn render_diff_view(State(state): State<Arc<AppState>>) -> Response {
    let (changes, error) = match state.pool.as_ref() {
        Some(pool) => match compare(pool, &state.schema).await {
            Ok(changes) => (changes, None),
            Err(e) => (Vec::new(), Some(e)),
        },
        None => (
            Vec::new(),
            Some(
                "Studio has no database connection, so nothing can be compared. \
                 Start studio with a --database-url (or DATABASE_URL) to use this screen."
                    .to_string(),
            ),
        ),
    };

    let tmpl = DiffViewTemplate {
        models: &state.models,
        current_model: "",
        provider: state.schema.datasource.provider.as_str(),
        allow_writes: state.config.allow_writes,
        changes,
        error,
    };

    match tmpl.render() {
        Ok(html) => Html(html).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Template error: {e}"),
        )
            .into_response(),
    }
}

/// Compares the live database against `schema`.
///
/// Scope matches `ruprizzle_migrate::drift`: tables, columns and nullability.
/// Column types, indexes and foreign keys are not compared, and the page says so
/// rather than implying they were checked.
async fn compare(pool: &Pool, schema: &Schema) -> Result<Vec<DiffChange>, String> {
    let provider = pool.provider();
    let database = ruprizzle_migrate::introspect::pull(pool)
        .await
        .map_err(|e| format!("Could not read the database catalog: {e}"))?;

    let mut changes = Vec::new();
    let mut matched_tables = BTreeSet::new();

    for model in schema.models.values() {
        let table = database.tables.iter().find(|t| t.name == model.table);
        let Some(table) = table else {
            changes.push(DiffChange {
                title: format!("Create table `{}`", model.table),
                description: format!(
                    "Model `{}` has no table in the database. Migrating creates it with \
                     {} column(s); nothing existing is touched.",
                    model.name,
                    model.fields.values().filter(|f| f.has_column()).count()
                ),
                risk: risk::SAFE.to_string(),
            });
            continue;
        };
        matched_tables.insert(table.name.clone());

        compare_table(pool, provider, model, table, &mut changes).await?;
    }

    unmanaged_tables(
        pool,
        provider,
        schema,
        &database,
        &matched_tables,
        &mut changes,
    )
    .await?;

    Ok(changes)
}

async fn compare_table(
    pool: &Pool,
    provider: Provider,
    model: &ruprizzle_core::ir::Model,
    table: &Table,
    changes: &mut Vec<DiffChange>,
) -> Result<(), String> {
    let row_count = db::count_rows(pool, provider, &table.name).await?;
    added_columns(model, table, row_count, changes);
    dropped_columns(pool, provider, model, table, changes).await?;
    nullability(pool, provider, model, table, changes).await
}

/// Columns the schema declares that the database does not have.
fn added_columns(
    model: &ruprizzle_core::ir::Model,
    table: &Table,
    row_count: i64,
    changes: &mut Vec<DiffChange>,
) {
    for field in model.fields.values().filter(|f| f.has_column()) {
        if table.columns.iter().any(|c| c.name == field.column) {
            continue;
        }

        let needs_backfill = !field.optional && field.default.is_none() && row_count > 0;
        let (description, level) = if needs_backfill {
            (
                format!(
                    "Column `{}` is NOT NULL with no default, and `{}` already holds {row_count} row(s). \
                     The migration will fail unless a default or a backfill is supplied first.",
                    field.column, table.name
                ),
                risk::CAUTION,
            )
        } else {
            (
                format!(
                    "Column `{}` will be added to `{}` ({row_count} existing row(s)). No data is removed.",
                    field.column, table.name
                ),
                risk::SAFE,
            )
        };

        changes.push(DiffChange {
            title: format!("Add column `{}`.`{}`", table.name, field.column),
            description,
            risk: level.to_string(),
        });
    }
}

/// Columns the database has that the schema does not declare.
async fn dropped_columns(
    pool: &Pool,
    provider: Provider,
    model: &ruprizzle_core::ir::Model,
    table: &Table,
    changes: &mut Vec<DiffChange>,
) -> Result<(), String> {
    for column in &table.columns {
        if model
            .fields
            .values()
            .any(|f| f.has_column() && f.column == column.name)
        {
            continue;
        }

        let populated = db::count_non_null(pool, provider, &table.name, &column.name).await?;
        let (description, level) = if populated > 0 {
            (
                format!(
                    "Column `{}`.`{}` is not in the schema. Dropping it destroys the {populated} row(s) \
                     that currently hold a value in it. This cannot be undone by a rollback.",
                    table.name, column.name
                ),
                risk::DESTRUCTIVE,
            )
        } else {
            (
                format!(
                    "Column `{}`.`{}` is not in the schema and holds no non-NULL values. \
                     Dropping it loses nothing, but confirm it is genuinely unused.",
                    table.name, column.name
                ),
                risk::CAUTION,
            )
        };

        changes.push(DiffChange {
            title: format!("Drop column `{}`.`{}`", table.name, column.name),
            description,
            risk: level.to_string(),
        });
    }

    Ok(())
}

/// Nullability disagreements on columns both sides have.
async fn nullability(
    pool: &Pool,
    provider: Provider,
    model: &ruprizzle_core::ir::Model,
    table: &Table,
    changes: &mut Vec<DiffChange>,
) -> Result<(), String> {
    for field in model.fields.values().filter(|f| f.has_column()) {
        let Some(column) = table.columns.iter().find(|c| c.name == field.column) else {
            continue;
        };

        if column.nullable && !field.optional {
            let nulls = db::count_null(pool, provider, &table.name, &column.name).await?;
            let (description, level) = if nulls > 0 {
                (
                    format!(
                        "`{}`.`{}` is nullable in the database but required by the schema, and \
                         {nulls} row(s) are NULL right now. Adding NOT NULL will fail until they are filled in.",
                        table.name, column.name
                    ),
                    risk::CAUTION,
                )
            } else {
                (
                    format!(
                        "`{}`.`{}` is nullable in the database but required by the schema. \
                         No row is NULL, so the NOT NULL constraint can be applied.",
                        table.name, column.name
                    ),
                    risk::SAFE,
                )
            };
            changes.push(DiffChange {
                title: format!("Tighten `{}`.`{}` to NOT NULL", table.name, column.name),
                description,
                risk: level.to_string(),
            });
        } else if !column.nullable && field.optional {
            changes.push(DiffChange {
                title: format!("Relax `{}`.`{}` to nullable", table.name, column.name),
                description: format!(
                    "`{}`.`{}` is NOT NULL in the database but optional in the schema. \
                     Relaxing a constraint keeps every existing value.",
                    table.name, column.name
                ),
                risk: risk::SAFE.to_string(),
            });
        }
    }

    Ok(())
}

async fn unmanaged_tables(
    pool: &Pool,
    provider: Provider,
    schema: &Schema,
    database: &DatabaseSchema,
    matched: &BTreeSet<String>,
    changes: &mut Vec<DiffChange>,
) -> Result<(), String> {
    for table in &database.tables {
        if matched.contains(&table.name) {
            continue;
        }
        // A table may match a model whose own comparison already ran; guard anyway.
        if schema.models.values().any(|m| m.table == table.name) {
            continue;
        }

        let rows = db::count_rows(pool, provider, &table.name).await?;
        changes.push(DiffChange {
            title: format!("Unmanaged table `{}`", table.name),
            description: format!(
                "`{}` exists in the database ({rows} row(s)) but no model maps to it. \
                 Ruprizzle will not migrate it; a `db pull` would be needed to bring it under management.",
                table.name
            ),
            risk: risk::CAUTION.to_string(),
        });
    }

    Ok(())
}
