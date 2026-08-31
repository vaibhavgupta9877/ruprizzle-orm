//! Foreign key traversal and relation drawer handlers.
//!
//! The drawer shows the record the foreign key actually points at. It used to fill
//! every field with the literal `"Linked <field_name>"` without opening a connection.

use askama::Template;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{Html, IntoResponse, Response};
use std::sync::Arc;

use ruprizzle::Value;
use ruprizzle_dialect::dialect_for;

use super::AppState;
use crate::studio::db;

#[derive(Debug, Clone)]
pub struct RelationField {
    pub name: String,
    pub value: String,
}

#[derive(Template)]
#[template(path = "relations/drawer.html")]
pub struct RelationDrawerTemplate<'a> {
    pub target_model: &'a str,
    pub target_id: &'a str,
    pub fields: Vec<RelationField>,
    /// Set when the record could not be read. Suppresses the attribute list.
    pub error: Option<String>,
}

/// Renders the slide-over relation inspection drawer.
pub async fn render_relation_drawer(
    Path((model_name, id)): Path<(String, String)>,
    State(state): State<Arc<AppState>>,
) -> Response {
    let Some(model) = state.schema.model(&model_name) else {
        return (StatusCode::NOT_FOUND, "Model not found").into_response();
    };

    let (fields, error) = load_record(&state, model, &id).await;

    let tmpl = RelationDrawerTemplate {
        target_model: &model_name,
        target_id: &id,
        fields,
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

async fn load_record(
    state: &AppState,
    model: &ruprizzle_core::ir::Model,
    id: &str,
) -> (Vec<RelationField>, Option<String>) {
    let Some(pool) = state.pool.as_ref() else {
        return (
            Vec::new(),
            Some(
                "Studio has no database connection, so this record could not be read.".to_string(),
            ),
        );
    };

    let Some(id_field) = model
        .fields
        .values()
        .find(|f| f.attrs.is_id && f.has_column())
    else {
        return (
            Vec::new(),
            Some(
                "This model has no single-column primary key to look the record up by.".to_string(),
            ),
        );
    };

    let provider = pool.provider();
    let dialect = dialect_for(provider);
    let columns: Vec<&ruprizzle_core::ir::Field> =
        model.fields.values().filter(|f| f.has_column()).collect();

    let projection = columns
        .iter()
        .map(|f| db::to_text(provider, &db::quote_ident(provider, &f.column)))
        .collect::<Vec<_>>()
        .join(", ");

    let sql = format!(
        "SELECT {projection} FROM {} WHERE {} = {}",
        db::quote_ident(provider, &model.table),
        db::quote_ident(provider, &id_field.column),
        db::bind_expr(dialect, provider, 0, id_field),
    );

    match db::fetch_text_rows(
        pool,
        sql,
        vec![Value::Str(id.to_owned().into())],
        columns.len(),
    )
    .await
    {
        Ok(rows) => match rows.into_iter().next() {
            Some(values) => (
                columns
                    .iter()
                    .zip(values)
                    .map(|(field, value)| RelationField {
                        name: field.name.to_string(),
                        value: value.unwrap_or_else(|| "null".to_string()),
                    })
                    .collect(),
                None,
            ),
            None => (
                Vec::new(),
                Some(format!(
                    "No `{}` row has {} = {id}. The foreign key points at a record that is not there.",
                    model.name, id_field.column
                )),
            ),
        },
        Err(e) => (Vec::new(), Some(format!("Could not read the record: {e}"))),
    }
}
