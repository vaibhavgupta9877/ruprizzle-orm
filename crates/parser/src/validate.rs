//! Validation rules that need only the finished IR (P1-03).
//!
//! The rule table lives in
//! `ProjectPlan/ImplementationPlan/ImplPlan02SchemaDslParser.md`; each rule is one
//! [`SchemaError`] variant. This module owns **V01, V11, V14 (empty enums), V16,
//! and V17** — the rules expressible over the IR alone. The rest need the source
//! spans of individual attributes and so are enforced during lowering; see
//! [`crate::lower`].
//!
//! The validator **collects**. Nothing here returns early on the first problem,
//! because a schema author fixing five mistakes should see five diagnostics, not
//! recompile five times.

use std::collections::HashMap;

use ruprizzle_core::diagnostic::{Diagnostics, SchemaError};
use ruprizzle_core::ir::{Model, Schema};
use ruprizzle_core::suggest;

use crate::naming;

/// Runs every IR-level rule, recording each failure.
pub(crate) fn validate(schema: &Schema, diags: &mut Diagnostics) {
    for def in schema.enums.values() {
        // V14
        if def.variants.is_empty() {
            diags.push(SchemaError::EmptyEnum {
                name: def.name.to_string(),
                span: def.span.into(),
            });
        }
    }

    for model in schema.models.values() {
        primary_key(model, diags);
        index_fields(model, diags);
        column_collisions(model, diags);
        reserved_keywords(model, diags);
    }

    table_collisions(schema, diags);
}

/// V01 — every model has exactly one primary key, over fields that exist.
fn primary_key(model: &Model, diags: &mut Diagnostics) {
    if model.primary_key.fields.is_empty() {
        diags.push(SchemaError::MissingPrimaryKey {
            model: model.name.to_string(),
            span: model.primary_key.span.into(),
        });
        return;
    }

    for field in &model.primary_key.fields {
        if !model.fields.contains_key(field.as_str()) {
            diags.push(SchemaError::UnknownIndexField {
                model: model.name.to_string(),
                attribute: "id".to_owned(),
                missing: field.to_string(),
                advice: did_you_mean(model, field.as_str()),
                span: model.primary_key.span.into(),
            });
        }
    }
}

/// V11 — `@@index` and `@@unique` name existing, column-bearing fields.
fn index_fields(model: &Model, diags: &mut Diagnostics) {
    let mut check = |attribute: &str, name: &str, span: ruprizzle_core::span::Span| match model
        .fields
        .get(name)
    {
        Some(f) if f.has_column() && f.relation().is_none() => {}
        Some(_) => diags.push(SchemaError::UnknownIndexField {
            model: model.name.to_string(),
            attribute: attribute.to_owned(),
            missing: name.to_owned(),
            advice: Some(
                "index the foreign key column rather than the navigation property".to_owned(),
            ),
            span: span.into(),
        }),
        None => diags.push(SchemaError::UnknownIndexField {
            model: model.name.to_string(),
            attribute: attribute.to_owned(),
            missing: name.to_owned(),
            advice: did_you_mean(model, name),
            span: span.into(),
        }),
    };

    for index in &model.indexes {
        for f in &index.fields {
            check("index", f.field.as_str(), index.span);
        }
    }
    for unique in &model.uniques {
        for f in &unique.fields {
            check("unique", f.as_str(), unique.span);
        }
    }
}

/// V16 — two fields of one model cannot land on the same column.
fn column_collisions(model: &Model, diags: &mut Diagnostics) {
    let mut seen: HashMap<&str, &str> = HashMap::new();
    for field in model.fields.values() {
        if !field.has_column() {
            continue;
        }
        if let Some(first) = seen.insert(field.column.as_str(), field.name.as_str()) {
            diags.push(SchemaError::NameCollision {
                a: format!("{}.{first}", model.name),
                b: format!("{}.{}", model.name, field.name),
                physical: field.column.clone(),
                span: field.span.into(),
            });
        }
    }
}

/// V16 — two models cannot land on the same table, and no table may collide with
/// an enum type.
fn table_collisions(schema: &Schema, diags: &mut Diagnostics) {
    let mut tables: HashMap<&str, &str> = HashMap::new();
    for model in schema.models.values() {
        if let Some(first) = tables.insert(model.table.as_str(), model.name.as_str()) {
            diags.push(SchemaError::NameCollision {
                a: first.to_owned(),
                b: model.name.to_string(),
                physical: model.table.clone(),
                span: model.span.into(),
            });
        }
    }

    let mut types: HashMap<&str, &str> = HashMap::new();
    for def in schema.enums.values() {
        if let Some(first) = types.insert(def.db_name.as_str(), def.name.as_str()) {
            diags.push(SchemaError::NameCollision {
                a: first.to_owned(),
                b: def.name.to_string(),
                physical: def.db_name.clone(),
                span: def.span.into(),
            });
        }
    }
}

/// V17 — field names that need `r#` escaping in generated Rust (a warning).
fn reserved_keywords(model: &Model, diags: &mut Diagnostics) {
    for field in model.fields.values() {
        if naming::is_rust_keyword(field.name.as_str()) {
            diags.push(SchemaError::ReservedKeyword {
                field: field.name.to_string(),
                span: field.span.into(),
            });
        }
    }
}

fn did_you_mean(model: &Model, name: &str) -> Option<String> {
    let candidates: Vec<String> = model.fields.keys().map(ToString::to_string).collect();
    suggest::closest(name, candidates.iter())
        .map(|c| format!("did you mean `{c}`?"))
        .or_else(|| Some(format!("`{}` has no field by that name", model.name)))
}
