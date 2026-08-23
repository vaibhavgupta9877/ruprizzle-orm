//! Request handlers for Ruprizzle Studio endpoints.

pub mod dashboard;
pub mod diff;
pub mod erd;
pub mod explain;
pub mod relations;
pub mod sandbox;
pub mod table;

use crate::studio::config::StudioConfig;
use ruprizzle_core::ir::Schema;

/// Model summary displayed in the navigation sidebar.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ModelNav {
    pub name: String,
    pub field_count: usize,
}

/// Shared application state passed to all Axum request handlers.
#[derive(Clone)]
pub struct AppState {
    pub schema: Schema,
    pub config: StudioConfig,
    #[allow(dead_code)]
    pub pool: Option<ruprizzle::Pool>,
    pub models: Vec<ModelNav>,
}

impl AppState {
    pub fn new(schema: Schema, config: StudioConfig, pool: Option<ruprizzle::Pool>) -> Self {
        let models = schema
            .models
            .values()
            .map(|m| ModelNav {
                name: m.name.to_string(),
                field_count: m.fields.len(),
            })
            .collect();

        Self {
            schema,
            config,
            pool,
            models,
        }
    }
}
