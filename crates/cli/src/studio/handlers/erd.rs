//! Interactive schema ERD graph handler for Ruprizzle Studio.

use askama::Template;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{Html, IntoResponse, Json, Response};
use serde::Serialize;
use std::sync::Arc;

use super::{AppState, ModelNav};

#[derive(Template)]
#[template(path = "erd/view.html")]
pub struct ErdViewTemplate<'a> {
    pub models: &'a [ModelNav],
    pub current_model: &'a str,
    pub provider: &'a str,
    pub allow_writes: bool,
}

#[derive(Debug, Serialize)]
pub struct ErdField {
    pub name: String,
    pub type_name: String,
    pub is_id: bool,
}

#[derive(Debug, Serialize)]
pub struct ErdModel {
    pub name: String,
    pub fields: Vec<ErdField>,
}

#[derive(Debug, Serialize)]
pub struct ErdRelation {
    pub from: String,
    pub to: String,
    pub relation_name: String,
}

#[derive(Debug, Serialize)]
pub struct ErdGraphData {
    pub models: Vec<ErdModel>,
    pub relations: Vec<ErdRelation>,
}

/// Renders the interactive ERD view page.
pub async fn render_erd_view(State(state): State<Arc<AppState>>) -> Response {
    let tmpl = ErdViewTemplate {
        models: &state.models,
        current_model: "",
        provider: state.schema.datasource.provider.as_str(),
        allow_writes: state.config.allow_writes,
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

/// Returns the schema graph nodes and edges as JSON for the graph visualizer.
pub async fn get_erd_data(State(state): State<Arc<AppState>>) -> Json<ErdGraphData> {
    let mut models = Vec::new();
    let mut relations = Vec::new();

    for m in state.schema.models.values() {
        let fields = m
            .fields
            .values()
            .map(|f| ErdField {
                name: f.name.to_string(),
                type_name: format!("{:?}", f.kind),
                is_id: f.attrs.is_id,
            })
            .collect();

        models.push(ErdModel {
            name: m.name.to_string(),
            fields,
        });

        for f in m.fields.values() {
            if let Some(rel) = f.relation() {
                relations.push(ErdRelation {
                    from: m.name.to_string(),
                    to: rel.target.to_string(),
                    relation_name: rel.name.clone().unwrap_or_default(),
                });
            }
        }
    }

    Json(ErdGraphData { models, relations })
}
