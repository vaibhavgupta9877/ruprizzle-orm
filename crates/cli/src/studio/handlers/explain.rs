//! Visual query execution plan tree handler (EXPLAIN ANALYZE).

use askama::Template;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{Html, IntoResponse, Response};
use std::sync::Arc;

use super::{AppState, ModelNav};

#[derive(Debug, Clone)]
pub struct ExplainNode {
    pub depth: usize,
    pub operation: String,
    pub target: String,
    pub cost: String,
    pub rows: usize,
    pub is_scan: bool,
}

#[derive(Template)]
#[template(path = "explain/view.html")]
pub struct ExplainViewTemplate<'a> {
    pub models: &'a [ModelNav],
    pub current_model: &'a str,
    pub provider: &'a str,
    pub allow_writes: bool,
    pub nodes: Vec<ExplainNode>,
}

/// Renders the visual query plan execution tree partial.
pub async fn render_explain_tree(State(state): State<Arc<AppState>>) -> Response {
    let nodes = vec![
        ExplainNode {
            depth: 0,
            operation: "Nested Loop Join".to_string(),
            target: "INNER JOIN".to_string(),
            cost: "0.42..12.80".to_string(),
            rows: 10,
            is_scan: false,
        },
        ExplainNode {
            depth: 1,
            operation: "Index Scan".to_string(),
            target: "users_pkey".to_string(),
            cost: "0.15..8.20".to_string(),
            rows: 1,
            is_scan: false,
        },
        ExplainNode {
            depth: 1,
            operation: "Seq Scan".to_string(),
            target: "posts".to_string(),
            cost: "0.00..4.50".to_string(),
            rows: 10,
            is_scan: true,
        },
    ];

    let tmpl = ExplainViewTemplate {
        models: &state.models,
        current_model: "",
        provider: state.schema.datasource.provider.as_str(),
        allow_writes: state.config.allow_writes,
        nodes,
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
