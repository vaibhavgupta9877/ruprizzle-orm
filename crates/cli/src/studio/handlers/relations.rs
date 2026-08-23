//! Foreign key traversal and relation drawer handlers.

use askama::Template;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{Html, IntoResponse, Response};
use std::sync::Arc;

use super::AppState;

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
}

/// Renders the slide-over relation inspection drawer.
pub async fn render_relation_drawer(
    Path((model_name, id)): Path<(String, String)>,
    State(state): State<Arc<AppState>>,
) -> Response {
    let model = match state.schema.model(&model_name) {
        Some(m) => m,
        None => return (StatusCode::NOT_FOUND, "Model not found").into_response(),
    };

    let mut fields = Vec::new();
    for f in model.fields.values() {
        let val = if f.attrs.is_id {
            id.clone()
        } else {
            format!("Linked {}", f.name)
        };
        fields.push(RelationField {
            name: f.name.to_string(),
            value: val,
        });
    }

    let tmpl = RelationDrawerTemplate {
        target_model: &model_name,
        target_id: &id,
        fields,
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
