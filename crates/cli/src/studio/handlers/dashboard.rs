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
    pub table: String,
    pub column_count: usize,
    pub primary_key: String,
    pub fk_count: usize,
}

#[derive(Template)]
#[template(path = "dashboard.html")]
pub struct DashboardTemplate<'a> {
    pub session_token: &'a str,
    pub models: &'a [ModelNav],
    pub current_model: &'a str,
    pub provider: &'a str,
    pub allow_writes: bool,
    pub connected: bool,
    pub model_count: usize,
    pub enum_count: usize,
    pub relation_count: usize,
    pub model_summaries: Vec<ModelSummary>,
}

/// Renders the dashboard overview page.
pub async fn render_dashboard(State(state): State<Arc<AppState>>) -> Response {
    let model_summaries = state
        .schema
        .models
        .values()
        .map(|m| {
            // `@@id([a, b])` has no single `@id` field; report every key column.
            let primary_key = if m.primary_key.fields.is_empty() {
                "none".to_string()
            } else {
                m.primary_key
                    .fields
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(", ")
            };
            // Only the owning side of a relation carries a foreign key.
            let fk_count = m
                .fields
                .values()
                .filter_map(|f| f.relation())
                .filter(|r| !r.fields.is_empty())
                .count();

            ModelSummary {
                name: m.name.to_string(),
                table: m.table.clone(),
                column_count: m.scalar_fields().count(),
                primary_key,
                fk_count,
            }
        })
        .collect();

    let tmpl = DashboardTemplate {
        session_token: &state.session_token,
        models: &state.models,
        current_model: "",
        provider: state.provider,
        allow_writes: state.config.allow_writes,
        connected: state.pool.is_some(),
        model_count: state.schema.models.len(),
        enum_count: state.schema.enums.len(),
        relation_count: state.schema.relations.len(),
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
