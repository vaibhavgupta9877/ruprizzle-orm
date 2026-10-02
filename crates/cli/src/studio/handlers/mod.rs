//! Request handlers for Ruprizzle Studio endpoints.

pub mod dashboard;
pub mod diff;
pub mod erd;
pub mod explain;
pub mod relations;
pub mod sandbox;
pub mod table;

use crate::studio::config::StudioConfig;
use ruprizzle_core::ir::{Field, FieldKind, Provider, Schema};

/// The human-readable name of a database provider, as shown in the page chrome.
#[must_use]
pub const fn provider_label(provider: Provider) -> &'static str {
    match provider {
        Provider::Postgres => "PostgreSQL",
        Provider::Sqlite => "SQLite",
        Provider::Mysql => "MySQL",
    }
}

/// A field's type spelled the way the schema DSL writes it: `Int`, `Role`,
/// `Post[]`, with a trailing `?` for an optional field.
///
/// Studio used to print the IR's `Debug` form, which put `Scalar(Int)` and
/// `Relation(RelationRef { .. })` in column headers.
#[must_use]
pub fn field_type_label(field: &Field) -> String {
    fn kind_label(kind: &FieldKind) -> String {
        match kind {
            FieldKind::Scalar(ty) => ty.as_str().to_string(),
            FieldKind::Enum(name) => name.to_string(),
            FieldKind::Relation(r) => r.target.to_string(),
            FieldKind::List(inner) => format!("{}[]", kind_label(inner)),
        }
    }
    let mut label = kind_label(&field.kind);
    if field.optional {
        label.push('?');
    }
    label
}

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
    /// Display name of the schema's provider, e.g. `PostgreSQL`.
    pub provider: &'static str,
    /// Session token every request must present (see [`crate::studio::guard`]);
    /// mutations must echo it in [`crate::studio::guard::TOKEN_HEADER`].
    /// Rendered into the page shell.
    pub session_token: String,
}

impl AppState {
    pub fn new(schema: Schema, config: StudioConfig, pool: Option<ruprizzle::Pool>) -> Self {
        let models = schema
            .models
            .values()
            .map(|m| ModelNav {
                name: m.name.to_string(),
                field_count: m.scalar_fields().count(),
            })
            .collect();

        let provider = provider_label(schema.datasource.provider);
        let session_token = config
            .auth_token
            .clone()
            .filter(|t| !t.trim().is_empty())
            .unwrap_or_else(crate::studio::guard::new_session_token);

        Self {
            schema,
            config,
            pool,
            models,
            provider,
            session_token,
        }
    }
}
