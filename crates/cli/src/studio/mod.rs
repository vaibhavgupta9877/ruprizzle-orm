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

/// Whether a database URL *looks* like production.
///
/// This is a substring match on `prod`, `aws.neon.tech` and `turso.io`. It is a
/// speed bump, not a guardrail, and the difference matters: it misses every
/// production database not named for the fact — an RDS endpoint, a bare IP,
/// `main-db.internal` — and it false-positives on `…/product_catalog_dev`. Do not
/// describe it to users as protection. Anything that must not be reachable belongs
/// behind credentials the developer does not hold, not behind this function.
#[must_use]
pub fn looks_like_production_url(url: &str) -> bool {
    let lower = url.to_lowercase();
    lower.contains("prod")
        || lower.contains("production")
        || lower.contains("aws.neon.tech")
        || lower.contains("turso.io")
}

/// Whether `host` is a loopback address.
///
/// A non-loopback bind puts Studio's unauthenticated mutation routes — including
/// `DELETE` — on the network. Studio has no authentication of any kind, so the
/// bind address is the only thing standing between those routes and anyone who can
/// reach the port.
#[must_use]
pub fn is_loopback_host(host: &str) -> bool {
    let host = host.trim().trim_start_matches('[').trim_end_matches(']');
    match host.parse::<std::net::IpAddr>() {
        Ok(ip) => ip.is_loopback(),
        Err(_) => host.eq_ignore_ascii_case("localhost"),
    }
}

/// Runs the Ruprizzle Studio embedded server.
///
/// # Errors
///
/// Returns an error if the schema cannot be parsed, the database URL looks like
/// production without `--yes-i-know`, writes are requested on a non-loopback bind,
/// the database cannot be reached, or the HTTP server fails to bind.
pub async fn run_studio(
    config: StudioConfig,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let schema_str = std::fs::read_to_string(&config.schema_path)?;
    let schema_name = config.schema_path.to_string_lossy();
    let schema = ruprizzle_parser::parse(&schema_name, &schema_str)
        .map_err(|e| format!("Failed to parse schema: {e}"))?;

    let db_url = config.database_url.as_deref().unwrap_or("");
    if looks_like_production_url(db_url) && !config.yes_i_know {
        return Err(
            "The database URL contains `prod`, `aws.neon.tech` or `turso.io`, which usually              means production. This is a name check, not a real safeguard — it misses any              production database not named for the fact. Pass --yes-i-know to proceed."
                .into(),
        );
    }

    // Studio has no authentication. Binding it off loopback with writes enabled
    // publishes DELETE routes to anyone who can reach the port.
    if !is_loopback_host(&config.host) && config.allow_writes && !config.yes_i_know {
        return Err(format!(
            "Refusing to bind Studio to {} with --allow-writes. Studio has no authentication,              so this publishes its INSERT, UPDATE and DELETE routes to every host that can              reach port {}. Drop --allow-writes, bind to 127.0.0.1, or pass --yes-i-know if              the network is genuinely trusted.",
            config.host, config.port
        )
        .into());
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
        println!("   Mode: read-write (--allow-writes)");
    } else {
        println!("   Mode: read-only (mutations disabled)");
    }
    if !is_loopback_host(&config.host) {
        println!(
            "   ⚠ Bound to {} — Studio has no authentication, so anyone who can reach              port {} can use it.",
            config.host, config.port
        );
    }

    if !config.no_browser {
        let _ = opener::open_browser(&url);
    }

    axum::serve(listener, app).await?;
    Ok(())
}
