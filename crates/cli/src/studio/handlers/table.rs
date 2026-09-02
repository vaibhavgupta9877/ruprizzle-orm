//! Table data browser, filtering, sorting, and inline editing handlers.
//!
//! Every route here issues real SQL against `AppState::pool`. There is no synthetic
//! fallback: without a connection the browser renders an error banner, a failed
//! `UPDATE` returns the database's message, and a `DELETE` that matched nothing
//! returns `404` rather than `200`.

use askama::Template;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{Html, IntoResponse, Response};
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::Arc;

use ruprizzle::{Pool, Value};
use ruprizzle_core::ir::{Model, Provider};
use ruprizzle_dialect::dialect_for;

use super::{AppState, ModelNav};
use crate::studio::db;

/// Rows fetched per page of the browser.
const PAGE_SIZE: usize = 50;

#[derive(Debug, Clone, serde::Serialize)]
pub struct FieldInfo {
    pub name: String,
    pub type_name: String,
    pub is_id: bool,
    pub is_relation: bool,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct CellInfo {
    pub column_name: String,
    pub value: String,
    pub is_id: bool,
    pub is_null: bool,
    pub is_relation: bool,
    pub relation_target: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct RowInfo {
    pub id: String,
    pub cells: Vec<CellInfo>,
}

#[derive(Template)]
#[template(path = "table/view.html")]
pub struct TableViewTemplate<'a> {
    pub models: &'a [ModelNav],
    pub current_model: &'a str,
    pub provider: &'a str,
    pub allow_writes: bool,
    pub fields: Vec<FieldInfo>,
    pub rows: Vec<RowInfo>,
    pub page: usize,
    /// Set when the rows could not be read. The grid renders the message instead.
    pub error: Option<String>,
}

#[derive(Template)]
#[template(path = "table/grid.html")]
pub struct TableGridTemplate<'a> {
    pub current_model: &'a str,
    pub allow_writes: bool,
    pub fields: Vec<FieldInfo>,
    pub rows: Vec<RowInfo>,
    pub page: usize,
    pub error: Option<String>,
}

#[derive(Template)]
#[template(path = "table/row.html")]
pub struct TableRowTemplate<'a> {
    pub current_model: &'a str,
    pub allow_writes: bool,
    pub row: RowInfo,
}

#[derive(Template)]
#[template(path = "table/cell.html")]
pub struct TableCellTemplate<'a> {
    pub current_model: &'a str,
    pub allow_writes: bool,
    pub row_id: String,
    pub cell: CellInfo,
}

#[derive(Debug, Deserialize)]
pub struct TableQuery {
    pub page: Option<usize>,
    pub search: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CellPatchQuery {
    pub column: String,
}

#[derive(Debug, Deserialize)]
pub struct CellPatchForm {
    pub value: Option<String>,
}

/// Renders the full table browser page for a model.
pub async fn render_table_view(
    Path(model_name): Path<String>,
    Query(query): Query<TableQuery>,
    State(state): State<Arc<AppState>>,
) -> Response {
    let Some(model) = state.schema.model(&model_name) else {
        return (StatusCode::NOT_FOUND, "Model not found").into_response();
    };

    let page = query.page.unwrap_or(1).max(1);
    let (rows, error) = load_rows(&state, model, page, query.search.as_deref()).await;

    let tmpl = TableViewTemplate {
        models: &state.models,
        current_model: &model_name,
        provider: state.schema.datasource.provider.as_str(),
        allow_writes: state.config.allow_writes,
        fields: extract_field_infos(model),
        rows,
        page,
        error,
    };

    render(&tmpl)
}

/// Renders the table grid partial for HTMX updates.
pub async fn render_table_grid(
    Path(model_name): Path<String>,
    Query(query): Query<TableQuery>,
    State(state): State<Arc<AppState>>,
) -> Response {
    let Some(model) = state.schema.model(&model_name) else {
        return (StatusCode::NOT_FOUND, "Model not found").into_response();
    };

    let page = query.page.unwrap_or(1).max(1);
    let (rows, error) = load_rows(&state, model, page, query.search.as_deref()).await;

    let tmpl = TableGridTemplate {
        current_model: &model_name,
        allow_writes: state.config.allow_writes,
        fields: extract_field_infos(model),
        rows,
        page,
        error,
    };

    render(&tmpl)
}

/// Inserts a new record into the database table.
pub async fn insert_row(
    Path(model_name): Path<String>,
    State(state): State<Arc<AppState>>,
    axum::extract::Form(form): axum::extract::Form<HashMap<String, String>>,
) -> Response {
    let Some(pool) = writable(&state) else {
        return write_refusal(&state);
    };
    let Some(model) = state.schema.model(&model_name) else {
        return (StatusCode::NOT_FOUND, "Model not found").into_response();
    };
    let Some(id_field) = primary_key(model) else {
        return (
            StatusCode::BAD_REQUEST,
            "This model has no single-column primary key, so Studio cannot edit its rows.",
        )
            .into_response();
    };

    let provider = pool.provider();
    let dialect = dialect_for(provider);

    // Only fields the user actually filled in are written, so database defaults
    // and generated identities still apply to the rest.
    let mut columns = Vec::new();
    let mut exprs = Vec::new();
    let mut binds = Vec::new();
    for field in model.fields.values().filter(|f| db::is_editable(f)) {
        let Some(raw) = form.get(field.name.as_str()) else {
            continue;
        };
        if raw.is_empty() {
            continue;
        }
        columns.push(db::quote_ident(provider, &field.column));
        exprs.push(db::bind_expr(dialect, provider, binds.len(), field));
        binds.push(Value::Str(raw.clone().into()));
    }

    if columns.is_empty() {
        return (StatusCode::BAD_REQUEST, "No values were supplied.").into_response();
    }

    let table = db::quote_ident(provider, &model.table);
    let sql = format!(
        "INSERT INTO {table} ({}) VALUES ({})",
        columns.join(", "),
        exprs.join(", ")
    );

    if let Err(e) = db::execute(pool, sql, binds).await {
        return (StatusCode::BAD_REQUEST, format!("Insert failed: {e}")).into_response();
    }

    // Read the row back rather than echoing the submitted values: defaults,
    // triggers and type coercion mean what was stored may not be what was typed.
    let id_column = db::quote_ident(provider, &id_field.column);
    let order_by = format!("ORDER BY {id_column} DESC");
    match select_rows(
        pool,
        provider,
        model,
        &format!("{order_by} LIMIT 1"),
        Vec::new(),
    )
    .await
    {
        Ok(mut rows) if !rows.is_empty() => {
            let tmpl = TableRowTemplate {
                current_model: &model_name,
                allow_writes: state.config.allow_writes,
                row: rows.remove(0),
            };
            render(&tmpl)
        }
        Ok(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            "The insert reported success but the row could not be read back.",
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("The insert succeeded but reading the row back failed: {e}"),
        )
            .into_response(),
    }
}

/// Updates an individual cell inline.
pub async fn patch_cell(
    Path((model_name, row_id)): Path<(String, String)>,
    Query(query): Query<CellPatchQuery>,
    State(state): State<Arc<AppState>>,
    axum::extract::Form(form): axum::extract::Form<CellPatchForm>,
) -> Response {
    let Some(pool) = writable(&state) else {
        return write_refusal(&state);
    };
    let Some(model) = state.schema.model(&model_name) else {
        return (StatusCode::NOT_FOUND, "Model not found").into_response();
    };
    let Some(id_field) = primary_key(model) else {
        return (
            StatusCode::BAD_REQUEST,
            "This model has no single-column primary key, so Studio cannot edit its rows.",
        )
            .into_response();
    };

    let Some(field) = model
        .fields
        .values()
        .find(|f| f.name.as_str() == query.column && db::is_editable(f))
    else {
        return (
            StatusCode::BAD_REQUEST,
            "Column not found, or not editable through Studio.",
        )
            .into_response();
    };

    let provider = pool.provider();
    let dialect = dialect_for(provider);

    let raw = form.value.unwrap_or_default();
    // An emptied optional column is written as NULL; an emptied required one is
    // written as the empty string, which is what the user actually asked for.
    let (value_expr, value_bind) = if raw.is_empty() && field.optional {
        ("NULL".to_string(), None)
    } else {
        (
            db::bind_expr(dialect, provider, 0, field),
            Some(Value::Str(raw.into())),
        )
    };

    let mut binds: Vec<Value> = value_bind.into_iter().collect();
    let id_expr = db::bind_expr(dialect, provider, binds.len(), id_field);
    binds.push(Value::Str(row_id.clone().into()));

    let sql = format!(
        "UPDATE {} SET {} = {value_expr} WHERE {} = {id_expr}",
        db::quote_ident(provider, &model.table),
        db::quote_ident(provider, &field.column),
        db::quote_ident(provider, &id_field.column),
    );

    match db::execute(pool, sql, binds).await {
        Ok(0) => {
            return (
                StatusCode::NOT_FOUND,
                format!("No row with {} = {row_id} was updated.", id_field.column),
            )
                .into_response();
        }
        Ok(_) => {}
        Err(e) => return (StatusCode::BAD_REQUEST, format!("Update failed: {e}")).into_response(),
    }

    // Re-read the stored value so the cell shows what the database kept, not what
    // was typed into the box.
    let where_clause = format!(
        "WHERE {} = {}",
        db::quote_ident(provider, &id_field.column),
        db::bind_expr(dialect, provider, 0, id_field)
    );
    match select_rows(
        pool,
        provider,
        model,
        &where_clause,
        vec![Value::Str(row_id.clone().into())],
    )
    .await
    {
        Ok(rows) => match rows.into_iter().next().and_then(|row| {
            row.cells
                .into_iter()
                .find(|c| c.column_name == field.name.as_str())
        }) {
            Some(cell) => {
                let tmpl = TableCellTemplate {
                    current_model: &model_name,
                    allow_writes: state.config.allow_writes,
                    row_id,
                    cell,
                };
                render(&tmpl)
            }
            None => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "The update succeeded but the cell could not be read back.",
            )
                .into_response(),
        },
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("The update succeeded but reading the cell back failed: {e}"),
        )
            .into_response(),
    }
}

/// Deletes a row by primary key.
pub async fn delete_row(
    Path((model_name, row_id)): Path<(String, String)>,
    State(state): State<Arc<AppState>>,
) -> Response {
    let Some(pool) = writable(&state) else {
        return write_refusal(&state);
    };
    let Some(model) = state.schema.model(&model_name) else {
        return (StatusCode::NOT_FOUND, "Model not found").into_response();
    };
    let Some(id_field) = primary_key(model) else {
        return (
            StatusCode::BAD_REQUEST,
            "This model has no single-column primary key, so Studio cannot delete its rows.",
        )
            .into_response();
    };

    let provider = pool.provider();
    let dialect = dialect_for(provider);

    let sql = format!(
        "DELETE FROM {} WHERE {} = {}",
        db::quote_ident(provider, &model.table),
        db::quote_ident(provider, &id_field.column),
        db::bind_expr(dialect, provider, 0, id_field),
    );

    match db::execute(pool, sql, vec![Value::Str(row_id.clone().into())]).await {
        // The row element is replaced by an empty body, which removes it.
        Ok(1..) => StatusCode::OK.into_response(),
        Ok(_) => (
            StatusCode::NOT_FOUND,
            format!("No row with {} = {row_id} was deleted.", id_field.column),
        )
            .into_response(),
        Err(e) => (StatusCode::BAD_REQUEST, format!("Delete failed: {e}")).into_response(),
    }
}

/// The pool, when writes are permitted and a database is connected.
fn writable(state: &AppState) -> Option<&Pool> {
    if state.config.allow_writes {
        state.pool.as_ref()
    } else {
        None
    }
}

fn write_refusal(state: &AppState) -> Response {
    if state.config.allow_writes {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            "Studio has no database connection, so nothing can be written. Start studio with a --database-url.",
        )
            .into_response()
    } else {
        (
            StatusCode::FORBIDDEN,
            "Mutating operations are disabled in read-only mode. Start studio with --allow-writes to permit mutations.",
        )
            .into_response()
    }
}

fn render<T: Template>(tmpl: &T) -> Response {
    match tmpl.render() {
        Ok(html) => Html(html).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Template error: {e}"),
        )
            .into_response(),
    }
}

/// The single-column primary key of `model`, if it has one.
fn primary_key(model: &Model) -> Option<&ruprizzle_core::ir::Field> {
    model
        .fields
        .values()
        .find(|f| f.attrs.is_id && f.has_column())
}

/// Loads one page of rows, turning any failure into a message for the grid.
async fn load_rows(
    state: &AppState,
    model: &Model,
    page: usize,
    search: Option<&str>,
) -> (Vec<RowInfo>, Option<String>) {
    let Some(pool) = state.pool.as_ref() else {
        return (
            Vec::new(),
            Some(
                "Studio has no database connection, so no rows can be shown. \
                 Start studio with a --database-url (or DATABASE_URL) to browse data."
                    .to_string(),
            ),
        );
    };

    let provider = pool.provider();
    let dialect = dialect_for(provider);
    let mut binds = Vec::new();
    let mut clauses: Vec<String> = Vec::new();

    if let Some(term) = search.map(str::trim).filter(|t| !t.is_empty()) {
        let mut predicates = Vec::new();
        for field in model.fields.values().filter(|f| db::is_editable(f)) {
            let column = db::quote_ident(provider, &field.column);
            let placeholder = dialect.placeholder(binds.len());
            predicates.push(format!(
                "{} LIKE {placeholder}",
                db::to_text(provider, &column)
            ));
            binds.push(Value::Str(format!("%{term}%").into()));
        }
        if !predicates.is_empty() {
            clauses.push(format!("WHERE {}", predicates.join(" OR ")));
        }
    }

    if let Some(id_field) = primary_key(model) {
        clauses.push(format!(
            "ORDER BY {}",
            db::quote_ident(provider, &id_field.column)
        ));
    }
    clauses.push(dialect.limit_offset(
        Some(PAGE_SIZE as u64),
        Some(((page - 1) * PAGE_SIZE) as u64),
    ));
    let clauses = clauses.join(" ");

    match select_rows(pool, provider, model, &clauses, binds).await {
        Ok(rows) => (rows, None),
        Err(e) => (Vec::new(), Some(format!("Could not read rows: {e}"))),
    }
}

/// Selects the model's columns with `tail` appended (`WHERE` / `ORDER BY` / `LIMIT`).
async fn select_rows(
    pool: &Pool,
    provider: Provider,
    model: &Model,
    tail: &str,
    binds: Vec<Value>,
) -> Result<Vec<RowInfo>, String> {
    let fields: Vec<&ruprizzle_core::ir::Field> = model
        .fields
        .values()
        .filter(|f| f.has_column())
        .collect::<Vec<_>>();

    let projection = fields
        .iter()
        .map(|f| db::to_text(provider, &db::quote_ident(provider, &f.column)))
        .collect::<Vec<_>>()
        .join(", ");

    let sql = format!(
        "SELECT {projection} FROM {} {tail}",
        db::quote_ident(provider, &model.table)
    );
    let raw = db::fetch_text_rows(pool, sql, binds, fields.len()).await?;

    let id_index = fields.iter().position(|f| f.attrs.is_id);

    Ok(raw
        .into_iter()
        .map(|values| {
            let cells = fields
                .iter()
                .zip(values.iter())
                .map(|(field, value)| CellInfo {
                    column_name: field.name.to_string(),
                    value: value.clone().unwrap_or_default(),
                    is_id: field.attrs.is_id,
                    is_null: value.is_none(),
                    is_relation: relation_target(model, field).is_some(),
                    relation_target: relation_target(model, field).unwrap_or_default(),
                })
                .collect();

            let id = id_index
                .and_then(|idx| values.get(idx).cloned().flatten())
                .unwrap_or_default();

            RowInfo { id, cells }
        })
        .collect())
}

/// The model a scalar foreign-key column points at, if any.
///
/// The relation lives on a navigation field that names its `fields:`; the column
/// itself is a plain scalar, so the link has to be recovered from the other side.
fn relation_target(model: &Model, field: &ruprizzle_core::ir::Field) -> Option<String> {
    model.fields.values().find_map(|nav| {
        let relation = nav.relation()?;
        relation
            .fields
            .iter()
            .any(|f| f.as_str() == field.name.as_str())
            .then(|| relation.target.to_string())
    })
}

fn extract_field_infos(model: &Model) -> Vec<FieldInfo> {
    model
        .fields
        .values()
        .filter(|f| f.has_column())
        .map(|f| FieldInfo {
            name: f.name.to_string(),
            type_name: format!("{:?}", f.kind),
            is_id: f.attrs.is_id,
            is_relation: relation_target(model, f).is_some(),
        })
        .collect()
}
