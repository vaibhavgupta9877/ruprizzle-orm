//! Embedded Pure-Rust Ruprizzle Studio web application.
//!
//! Provides a single-binary visual data workbench booting in <15ms with zero
//! Node/npm dependencies: table browser, live cell editor, foreign key drawer,
//! interactive ERD visualizer, SQL playground, and migration safety diffs.

pub mod assets;
pub mod config;
pub mod db;
pub mod handlers;
pub mod routes;

pub use config::StudioConfig;
pub use routes::create_router;
use std::sync::Arc;

use handlers::AppState;

/// Checks whether a database URL targets a production or remote environment.
#[must_use]
pub fn is_production_url(url: &str) -> bool {
    let lower = url.to_lowercase();
    lower.contains("prod")
        || lower.contains("production")
        || lower.contains("aws.neon.tech")
        || lower.contains("turso.io")
}

/// Runs the Ruprizzle Studio embedded server.
///
/// # Errors
///
/// Returns an error if the schema cannot be parsed, production guardrails are violated,
/// or the HTTP server fails to bind.
pub async fn run_studio(
    config: StudioConfig,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let schema_str = std::fs::read_to_string(&config.schema_path)?;
    let schema_name = config.schema_path.to_string_lossy();
    let schema = ruprizzle_parser::parse(&schema_name, &schema_str)
        .map_err(|e| format!("Failed to parse schema: {e}"))?;

    let db_url = config.database_url.as_deref().unwrap_or("");
    if is_production_url(db_url) && !config.yes_i_know {
        return Err(
            "Studio blocked connecting to a detected production database URL. Pass --yes-i-know to override this safety guardrail."
                .into(),
        );
    }

    // A failed connection used to be swallowed with `.ok()`, which left Studio
    // running against `None` and every data screen quietly empty. Fail loudly
    // instead: a URL was supplied, so the user expects to be connected.
    let pool = if db_url.is_empty() {
        None
    } else {
        Some(
            ruprizzle::connect(db_url)
                .await
                .map_err(|e| format!("Failed to connect to the database: {e}"))?,
        )
    };

    let state = Arc::new(AppState::new(schema, config.clone(), pool));
    let app = create_router(state);

    let bind_addr = format!("{}:{}", config.host, config.port);
    let listener = tokio::net::TcpListener::bind(&bind_addr).await?;
    let url = format!("http://{bind_addr}/studio");

    println!("⚡ Ruprizzle Studio running at: {url}");
    if config.allow_writes {
        println!("   Mode: Read-Write Active (--allow-writes)");
    } else {
        println!("   Mode: Read-Only Safe (mutations disabled)");
    }

    if !config.no_browser {
        let _ = opener::open_browser(&url);
    }

    axum::serve(listener, app).await?;
    Ok(())
}
