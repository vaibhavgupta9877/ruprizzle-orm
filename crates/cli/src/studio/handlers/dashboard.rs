//! Dashboard overview handler for Ruprizzle Studio.

use askama::Template;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{Html, IntoResponse, Response};
use std::sync::Arc;

use super::{AppState, ModelNav};

#[derive(Debug, Clone)]
pub struct ModelSummary {
    pub name: String,
    pub field_count: usize,
    pub primary_key: String,
    pub fk_count: usize,
}

#[derive(Template)]
#[template(path = "dashboard.html")]
pub struct DashboardTemplate<'a> {
    pub models: &'a [ModelNav],
    pub current_model: &'a str,
    pub provider: &'a str,
    pub allow_writes: bool,
    pub model_count: usize,
    pub enum_count: usize,
    pub model_summaries: Vec<ModelSummary>,
}

/// Renders the dashboard overview page.
pub async fn render_dashboard(State(state): State<Arc<AppState>>) -> Response {
    let provider_name = match state.schema.datasource.provider {
        ruprizzle_core::ir::Provider::Postgres => "PostgreSQL",
        ruprizzle_core::ir::Provider::Sqlite => "SQLite",
        ruprizzle_core::ir::Provider::Mysql => "MySQL",
    };

    let model_summaries = state
        .schema
        .models
        .values()
        .map(|m| {
            let pk = m
                .fields
                .values()
                .find(|f| f.attrs.is_id)
                .map_or_else(|| "none".to_string(), |f| f.name.to_string());
            let fk_count = m.fields.values().filter(|f| f.relation().is_some()).count();

            ModelSummary {
                name: m.name.to_string(),
                field_count: m.fields.len(),
                primary_key: pk,
                fk_count,
            }
        })
        .collect();

    let tmpl = DashboardTemplate {
        models: &state.models,
        current_model: "",
        provider: provider_name,
        allow_writes: state.config.allow_writes,
        model_count: state.schema.models.len(),
        enum_count: state.schema.enums.len(),
        model_summaries,
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
