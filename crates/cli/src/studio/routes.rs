//! Route definitions and Axum router for Ruprizzle Studio.

use axum::Router;
use axum::extract::Path;
use axum::response::{IntoResponse, Redirect, Response};
use axum::routing::{delete, get, patch, post};
use std::sync::Arc;

use super::assets::serve_asset;
use super::handlers::{AppState, dashboard, diff, erd, explain, relations, sandbox, table};

async fn handle_asset(Path(path): Path<String>) -> Response {
    serve_asset(&path).await
}

async fn handle_root_redirect() -> impl IntoResponse {
    Redirect::permanent("/studio")
}

/// Builds the Axum router with all studio routes and middleware.
pub fn create_router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/", get(handle_root_redirect))
        .route("/studio", get(dashboard::render_dashboard))
        .route("/studio/models/{model}", get(table::render_table_view))
        .route(
            "/studio/models/{model}/table",
            get(table::render_table_grid),
        )
        .route("/studio/models/{model}/rows", post(table::insert_row))
        .route(
            "/studio/models/{model}/rows/{id}/cell",
            patch(table::patch_cell),
        )
        .route(
            "/studio/models/{model}/rows/{id}",
            delete(table::delete_row),
        )
        .route(
            "/studio/relations/{model}/{id}",
            get(relations::render_relation_drawer),
        )
        .route("/studio/erd", get(erd::render_erd_view))
        .route("/studio/erd/data", get(erd::get_erd_data))
        .route("/studio/sandbox", get(sandbox::render_sandbox_view))
        .route(
            "/studio/sandbox/execute",
            post(sandbox::execute_sandbox_query),
        )
        .route("/studio/diff", get(diff::render_diff_view))
        .route("/studio/explain", post(explain::render_explain_tree))
        .route("/studio/assets/{*path}", get(handle_asset))
        .with_state(state)
}
