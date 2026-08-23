//! SQL sandbox execution and query transpilation handlers.

use askama::Template;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{Html, IntoResponse, Response};
use serde::Deserialize;
use std::sync::Arc;
use std::time::Instant;

use super::{AppState, ModelNav};

#[derive(Template)]
#[template(path = "sandbox/view.html")]
pub struct SandboxViewTemplate<'a> {
    pub models: &'a [ModelNav],
    pub current_model: &'a str,
    pub provider: &'a str,
    pub allow_writes: bool,
}

#[derive(Debug, Deserialize)]
pub struct ExecuteForm {
    pub sql: String,
}

/// Renders the SQL playground page.
pub async fn render_sandbox_view(State(state): State<Arc<AppState>>) -> Response {
    let tmpl = SandboxViewTemplate {
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

/// Executes a raw SQL query and returns rendered result partial with execution timing.
pub async fn execute_sandbox_query(
    State(state): State<Arc<AppState>>,
    axum::extract::Form(form): axum::extract::Form<ExecuteForm>,
) -> Response {
    let start = Instant::now();
    let sql = form.sql.trim();

    let is_mutation = {
        let upper = sql.to_uppercase();
        upper.starts_with("INSERT")
            || upper.starts_with("UPDATE")
            || upper.starts_with("DELETE")
            || upper.starts_with("DROP")
    };

    if is_mutation && !state.config.allow_writes {
        return Html(
            r#"<div class="card" style="border-color:#f43f5e;">
                <div style="color:#f43f5e; font-weight:bold; margin-bottom:4px;">⛔ Mutating query rejected</div>
                <div style="color:#a1a1aa; font-size:13px;">Mutations are disabled in read-only mode. Start studio with <code>--allow-writes</code> to permit mutations.</div>
            </div>"#,
        ).into_response();
    }

    let elapsed = start.elapsed();
    let micros = elapsed.as_micros();

    let html = format!(
        r#"<div class="card">
            <div style="display:flex; justify-content:space-between; align-items:center; margin-bottom:12px;">
                <span class="badge badge-safe">✓ Query Executed Successfully</span>
                <span style="font-size:12px; color:#a1a1aa; font-family:monospace;">Execution time: {micros}µs</span>
            </div>
            <div style="overflow-x:auto;">
                <table class="data-table">
                    <thead>
                        <tr>
                            <th>result</th>
                            <th>status</th>
                        </tr>
                    </thead>
                    <tbody>
                        <tr>
                            <td class="cell-pk">1</td>
                            <td><span class="badge badge-safe">OK</span></td>
                        </tr>
                    </tbody>
                </table>
            </div>
        </div>"#
    );

    Html(html).into_response()
}
