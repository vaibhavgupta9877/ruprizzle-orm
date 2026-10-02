//! Interactive schema ERD graph handler for Ruprizzle Studio.
//!
//! The graph is built from the schema's canonical relations
//! ([`Schema::relations`]), so each relation is one edge. Walking both sides'
//! navigation fields instead, as this handler used to, drew every relation twice,
//! once in each direction, and could not say which columns it joined.

use askama::Template;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{Html, IntoResponse, Json, Response};
use serde::Serialize;
use std::sync::Arc;

use ruprizzle_core::ir::{FieldKind, IndexTarget, Model, RelationKind, Schema};

use super::{AppState, ModelNav, field_type_label};

#[derive(Template)]
#[template(path = "erd/view.html")]
pub struct ErdViewTemplate<'a> {
    pub session_token: &'a str,
    pub models: &'a [ModelNav],
    pub current_model: &'a str,
    pub provider: &'a str,
    pub allow_writes: bool,
}

#[derive(Debug, Serialize)]
#[allow(clippy::struct_excessive_bools)]
pub struct ErdField {
    pub name: String,
    /// Physical column name; empty for a navigation field, which has none.
    pub column: String,
    /// The type as the schema spells it, e.g. `Int`, `Role?`, `Post[]`.
    pub type_name: String,
    pub is_id: bool,
    pub is_unique: bool,
    pub optional: bool,
    /// Whether the field is part of a foreign key.
    pub is_fk: bool,
    /// Whether the field is a navigation property rather than a column.
    pub is_relation: bool,
    /// The enum this field is typed by, if any.
    pub enum_name: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ErdModel {
    pub name: String,
    pub table: String,
    pub docs: Option<String>,
    pub fields: Vec<ErdField>,
}

#[derive(Debug, Serialize)]
pub struct ErdEnum {
    pub name: String,
    pub variants: Vec<String>,
}

/// One relation, drawn from the model holding the foreign key (`from`) to the
/// model it references (`to`).
#[derive(Debug, Serialize)]
pub struct ErdRelation {
    pub from: String,
    pub to: String,
    pub relation_name: String,
    /// `one_to_one`, `many_to_one` or `many_to_many`, from `from`'s point of view.
    pub kind: &'static str,
    /// Foreign-key fields on `from`, in key order.
    pub from_fields: Vec<String>,
    /// Referenced fields on `to`, positionally matching `from_fields`.
    pub to_fields: Vec<String>,
    /// Navigation field on `from`, if any.
    pub from_nav: Option<String>,
    /// Back-reference navigation field on `to`, if one was declared.
    pub to_nav: Option<String>,
    /// Whether the foreign key is nullable.
    pub optional: bool,
    pub on_delete: &'static str,
    pub on_update: &'static str,
    /// Join model of a many-to-many relation.
    pub through: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ErdGraphData {
    pub models: Vec<ErdModel>,
    pub relations: Vec<ErdRelation>,
    pub enums: Vec<ErdEnum>,
}

/// Renders the interactive ERD view page.
pub async fn render_erd_view(State(state): State<Arc<AppState>>) -> Response {
    let tmpl = ErdViewTemplate {
        session_token: &state.session_token,
        models: &state.models,
        current_model: "",
        provider: state.provider,
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
    Json(build_graph(&state.schema))
}

/// Builds the ERD payload for `schema`.
#[must_use]
pub fn build_graph(schema: &Schema) -> ErdGraphData {
    let relations = if schema.relations.is_empty() {
        owning_side_relations(schema)
    } else {
        canonical_relations(schema)
    };

    let models = schema
        .models
        .values()
        .map(|m| {
            let fk_fields: Vec<&str> = relations
                .iter()
                .filter(|r| r.from == m.name.as_str())
                .flat_map(|r| r.from_fields.iter().map(String::as_str))
                .collect();
            let unique_single: Vec<&str> = m
                .uniques
                .iter()
                .filter_map(|u| match u.targets.as_slice() {
                    [IndexTarget::Field(name, _)] => Some(name.as_str()),
                    _ => None,
                })
                .collect();

            let fields = m
                .fields
                .values()
                .map(|f| {
                    let is_relation = f.relation().is_some();
                    ErdField {
                        name: f.name.to_string(),
                        column: if f.has_column() {
                            f.column.clone()
                        } else {
                            String::new()
                        },
                        type_name: field_type_label(f),
                        is_id: f.attrs.is_id
                            || (m.primary_key.fields.len() > 1
                                && m.primary_key.fields.iter().any(|k| k == &f.name)),
                        is_unique: f.attrs.is_unique || unique_single.contains(&f.name.as_str()),
                        optional: f.optional,
                        is_fk: fk_fields.contains(&f.name.as_str()),
                        is_relation,
                        enum_name: enum_of(&f.kind),
                    }
                })
                .collect();

            ErdModel {
                name: m.name.to_string(),
                table: m.table.clone(),
                docs: m.docs.clone(),
                fields,
            }
        })
        .collect();

    let enums = schema
        .enums
        .values()
        .map(|e| ErdEnum {
            name: e.name.to_string(),
            variants: e.variants.keys().cloned().collect(),
        })
        .collect();

    ErdGraphData {
        models,
        relations,
        enums,
    }
}

fn enum_of(kind: &FieldKind) -> Option<String> {
    match kind {
        FieldKind::Enum(name) => Some(name.to_string()),
        FieldKind::List(inner) => enum_of(inner),
        _ => None,
    }
}

/// The field of `model` stored in `column`, falling back to the column name.
fn field_for_column(model: Option<&Model>, column: &str) -> String {
    model
        .and_then(|m| {
            m.fields
                .values()
                .find(|f| f.has_column() && f.column == column)
        })
        .map_or_else(|| column.to_string(), |f| f.name.to_string())
}

fn canonical_relations(schema: &Schema) -> Vec<ErdRelation> {
    schema
        .relations
        .iter()
        .map(|r| {
            let owner = schema.model(r.owner.as_str());
            let target = schema.model(r.target.as_str());
            ErdRelation {
                from: r.owner.to_string(),
                to: r.target.to_string(),
                relation_name: r.name.clone(),
                kind: match r.kind {
                    RelationKind::OneToOne => "one_to_one",
                    RelationKind::ManyToOne => "many_to_one",
                    RelationKind::ManyToMany => "many_to_many",
                },
                from_fields: r
                    .owner_cols
                    .iter()
                    .map(|c| field_for_column(owner, c))
                    .collect(),
                to_fields: r
                    .target_cols
                    .iter()
                    .map(|c| field_for_column(target, c))
                    .collect(),
                from_nav: Some(r.owner_field.to_string()),
                to_nav: r.target_field.as_ref().map(ToString::to_string),
                optional: r.optional,
                on_delete: r.on_delete.as_sql(),
                on_update: r.on_update.as_sql(),
                through: r.join_model.as_ref().map(ToString::to_string),
            }
        })
        .collect()
}

/// Relations recovered from the owning sides' `@relation(fields: …)`, for a schema
/// whose canonical relation list was never populated.
fn owning_side_relations(schema: &Schema) -> Vec<ErdRelation> {
    let mut out = Vec::new();
    for m in schema.models.values() {
        for f in m.fields.values() {
            let Some(rel) = f.relation() else { continue };
            if rel.fields.is_empty() {
                continue;
            }
            let optional = rel
                .fields
                .iter()
                .any(|name| m.field(name.as_str()).is_some_and(|fk| fk.optional));
            let unique = match rel.fields.as_slice() {
                [only] => m.field(only.as_str()).is_some_and(|fk| fk.attrs.is_unique),
                _ => false,
            };
            out.push(ErdRelation {
                from: m.name.to_string(),
                to: rel.target.to_string(),
                relation_name: rel.name.clone().unwrap_or_default(),
                kind: if unique { "one_to_one" } else { "many_to_one" },
                from_fields: rel.fields.iter().map(ToString::to_string).collect(),
                to_fields: rel.references.iter().map(ToString::to_string).collect(),
                from_nav: Some(f.name.to_string()),
                to_nav: None,
                optional,
                on_delete: rel.on_delete.unwrap_or_default().as_sql(),
                on_update: rel.on_update.unwrap_or_default().as_sql(),
                through: None,
            });
        }
    }
    out
}
