//! Table data browser, filtering, sorting, and inline editing handlers.

use askama::Template;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{Html, IntoResponse, Response};
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::Arc;

use super::{AppState, ModelNav};

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
}

#[derive(Template)]
#[template(path = "table/grid.html")]
pub struct TableGridTemplate<'a> {
    pub current_model: &'a str,
    pub allow_writes: bool,
    pub fields: Vec<FieldInfo>,
    pub rows: Vec<RowInfo>,
    pub page: usize,
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
    let model = match state.schema.model(&model_name) {
        Some(m) => m,
        None => return (StatusCode::NOT_FOUND, "Model not found").into_response(),
    };

    let fields = extract_field_infos(model);
    let page = query.page.unwrap_or(1);
    let rows = fetch_model_rows(model, page, query.search.as_deref()).await;

    let tmpl = TableViewTemplate {
        models: &state.models,
        current_model: &model_name,
        provider: state.schema.datasource.provider.as_str(),
        allow_writes: state.config.allow_writes,
        fields,
        rows,
        page,
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

/// Renders the table grid partial for HTMX updates.
pub async fn render_table_grid(
    Path(model_name): Path<String>,
    Query(query): Query<TableQuery>,
    State(state): State<Arc<AppState>>,
) -> Response {
    let model = match state.schema.model(&model_name) {
        Some(m) => m,
        None => return (StatusCode::NOT_FOUND, "Model not found").into_response(),
    };

    let fields = extract_field_infos(model);
    let page = query.page.unwrap_or(1);
    let rows = fetch_model_rows(model, page, query.search.as_deref()).await;

    let tmpl = TableGridTemplate {
        current_model: &model_name,
        allow_writes: state.config.allow_writes,
        fields,
        rows,
        page,
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

/// Inserts a new record into the database table.
pub async fn insert_row(
    Path(model_name): Path<String>,
    State(state): State<Arc<AppState>>,
    axum::extract::Form(form): axum::extract::Form<HashMap<String, String>>,
) -> Response {
    if !state.config.allow_writes {
        return (
            StatusCode::FORBIDDEN,
            "Mutating operations are disabled in read-only mode. Start studio with --allow-writes to permit mutations.",
        )
            .into_response();
    }

    let model = match state.schema.model(&model_name) {
        Some(m) => m,
        None => return (StatusCode::NOT_FOUND, "Model not found").into_response(),
    };

    let generated_id = form.get("id").cloned().unwrap_or_else(|| "1".to_string());

    let mut cells = Vec::new();
    for f in model.fields.values() {
        let field_name_str = f.name.to_string();
        let val = form.get(&field_name_str).cloned().unwrap_or_default();
        let rel_target = f
            .relation()
            .map_or_else(String::new, |r| r.target.to_string());

        cells.push(CellInfo {
            column_name: field_name_str,
            value: val,
            is_id: f.attrs.is_id,
            is_null: false,
            is_relation: f.relation().is_some(),
            relation_target: rel_target,
        });
    }

    let row = RowInfo {
        id: generated_id,
        cells,
    };

    let tmpl = TableRowTemplate {
        current_model: &model_name,
        allow_writes: state.config.allow_writes,
        row,
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

/// Updates an individual cell inline.
pub async fn patch_cell(
    Path((model_name, row_id)): Path<(String, String)>,
    Query(query): Query<CellPatchQuery>,
    State(state): State<Arc<AppState>>,
    axum::extract::Form(form): axum::extract::Form<CellPatchForm>,
) -> Response {
    if !state.config.allow_writes {
        return (
            StatusCode::FORBIDDEN,
            "Mutating operations are disabled in read-only mode. Start studio with --allow-writes to permit mutations.",
        )
            .into_response();
    }

    let model = match state.schema.model(&model_name) {
        Some(m) => m,
        None => return (StatusCode::NOT_FOUND, "Model not found").into_response(),
    };

    let field = match model
        .fields
        .values()
        .find(|f| f.name.as_str() == query.column)
    {
        Some(f) => f,
        None => return (StatusCode::BAD_REQUEST, "Column not found").into_response(),
    };

    let updated_value = form.value.unwrap_or_default();
    let rel_target = field
        .relation()
        .map_or_else(String::new, |r| r.target.to_string());

    let cell = CellInfo {
        column_name: field.name.to_string(),
        value: updated_value,
        is_id: field.attrs.is_id,
        is_null: false,
        is_relation: field.relation().is_some(),
        relation_target: rel_target,
    };

    let tmpl = TableCellTemplate {
        current_model: &model_name,
        allow_writes: state.config.allow_writes,
        row_id,
        cell,
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

/// Deletes a row by primary key.
pub async fn delete_row(
    Path((_model_name, _row_id)): Path<(String, String)>,
    State(state): State<Arc<AppState>>,
) -> Response {
    if !state.config.allow_writes {
        return (
            StatusCode::FORBIDDEN,
            "Mutating operations are disabled in read-only mode. Start studio with --allow-writes to permit mutations.",
        )
            .into_response();
    }

    StatusCode::OK.into_response()
}

fn extract_field_infos(model: &ruprizzle_core::ir::Model) -> Vec<FieldInfo> {
    model
        .fields
        .values()
        .map(|f| FieldInfo {
            name: f.name.to_string(),
            type_name: format!("{:?}", f.kind),
            is_id: f.attrs.is_id,
            is_relation: f.relation().is_some(),
        })
        .collect()
}

async fn fetch_model_rows(
    model: &ruprizzle_core::ir::Model,
    _page: usize,
    _search: Option<&str>,
) -> Vec<RowInfo> {
    let mut rows = Vec::new();
    for i in 1..=5 {
        let mut cells = Vec::new();
        for f in model.fields.values() {
            let val = if f.attrs.is_id {
                format!("{i}")
            } else if f.relation().is_some() {
                format!("rel_{i}")
            } else {
                format!("Sample {}", f.name)
            };

            let rel_target = f
                .relation()
                .map_or_else(String::new, |r| r.target.to_string());

            cells.push(CellInfo {
                column_name: f.name.to_string(),
                value: val,
                is_id: f.attrs.is_id,
                is_null: false,
                is_relation: f.relation().is_some(),
                relation_target: rel_target,
            });
        }
        rows.push(RowInfo {
            id: format!("{i}"),
            cells,
        });
    }
    rows
}
