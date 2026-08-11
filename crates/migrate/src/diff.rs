//! Schema diffing: turn two [`Schema`]s into a [`Vec<Change>`].

use ruprizzle_core::ir::{Field, FieldKind, Model, ResolvedRelation, Schema};
use ruprizzle_core::names::{FieldName, ModelName};

use crate::change::{Change, ColumnAspect};

/// Diff `prev` against `next`, returning the changes required to migrate.
#[must_use]
pub fn diff(prev: &Schema, next: &Schema) -> Vec<Change> {
    let mut changes = Vec::new();
    diff_enums(prev, next, &mut changes);
    diff_models(prev, next, &mut changes);
    diff_relations(prev, next, &mut changes);
    changes
}

fn diff_enums(prev: &Schema, next: &Schema, changes: &mut Vec<Change>) {
    // Create new enums first.
    for (name, def) in &next.enums {
        if !prev.enums.contains_key(name) {
            changes.push(Change::CreateEnum(def.clone()));
        }
    }

    // Drop removed enums last (handled at the very end by ordering).
    for (name, def) in &prev.enums {
        if !next.enums.contains_key(name) {
            changes.push(Change::DropEnum(name.clone(), def.db_name.clone()));
        }
    }

    // Add/drop variants for existing enums.
    for (name, next_def) in &next.enums {
        if let Some(prev_def) = prev.enums.get(name) {
            for variant in next_def.variants.keys() {
                if !prev_def.variants.contains_key(variant) {
                    changes.push(Change::AddEnumVariant {
                        enum_: name.clone(),
                        variant: variant.clone(),
                    });
                }
            }
            for variant in prev_def.variants.keys() {
                if !next_def.variants.contains_key(variant) {
                    changes.push(Change::DropEnumVariant {
                        enum_: name.clone(),
                        variant: variant.clone(),
                    });
                }
            }
        }
    }
}

fn diff_models(prev: &Schema, next: &Schema, changes: &mut Vec<Change>) {
    // Create new models.
    for (name, model) in &next.models {
        if !prev.models.contains_key(name) {
            changes.push(Change::CreateModel(model.clone()));
        }
    }

    // Drop removed models.
    for (name, model) in &prev.models {
        if !next.models.contains_key(name) {
            changes.push(Change::DropModel(name.clone(), model.table.clone()));
        }
    }

    // Compare existing models.
    for (name, next_model) in &next.models {
        if let Some(prev_model) = prev.models.get(name) {
            diff_columns(name, prev_model, next_model, changes);
            diff_indexes(name, prev_model, next_model, changes);
            diff_uniques(name, prev_model, next_model, changes);
        }
    }
}

fn diff_columns(model: &ModelName, prev: &Model, next: &Model, changes: &mut Vec<Change>) {
    // Process authoritative rename hints first.
    let mut renamed: Vec<FieldName> = Vec::new();
    let mut consumed_prev: Vec<FieldName> = Vec::new();

    for (new_name, field) in &next.fields {
        if let Some(old_name) = field.attrs.renamed_from.as_ref() {
            if let Some(prev_field) = prev.fields.get(old_name.as_str()) {
                if !prev_field.has_column() || !field.has_column() {
                    continue;
                }
                let old_field_name = FieldName::from(old_name.as_str());
                changes.push(Change::RenameColumn {
                    model: model.clone(),
                    from: old_field_name.clone(),
                    to: new_name.clone(),
                    from_column: prev_field.column.clone(),
                    to_column: field.column.clone(),
                });
                renamed.push(new_name.clone());
                consumed_prev.push(old_field_name);

                // If the type/default/etc also changed, emit an alter after the rename.
                let mut prev_field = prev_field.clone();
                prev_field.name = new_name.clone();
                prev_field.column = field.column.clone();
                if let Some(aspects) = column_aspects(&prev_field, field) {
                    changes.push(Change::AlterColumn {
                        model: model.clone(),
                        from: prev_field,
                        to: field.clone(),
                        aspects,
                    });
                }
            }
        }
    }

    // Add new columns.
    for (name, field) in &next.fields {
        if renamed.contains(name) {
            continue;
        }
        if !field.has_column() {
            continue;
        }
        if !prev.fields.contains_key(name) {
            changes.push(Change::AddColumn {
                model: model.clone(),
                field: field.clone(),
            });
        }
    }

    // Drop removed columns.
    for (name, field) in &prev.fields {
        if consumed_prev.contains(name) {
            continue;
        }
        if !field.has_column() {
            continue;
        }
        if !next.fields.contains_key(name) {
            changes.push(Change::DropColumn {
                model: model.clone(),
                column: field.column.clone(),
            });
        }
    }

    // Alter changed columns.
    for (name, next_field) in &next.fields {
        if renamed.contains(name) || !next_field.has_column() {
            continue;
        }
        if let Some(prev_field) = prev.fields.get(name) {
            if !prev_field.has_column() {
                continue;
            }
            if prev_field.column != next_field.column {
                changes.push(Change::RenameColumn {
                    model: model.clone(),
                    from: name.clone(),
                    to: name.clone(),
                    from_column: prev_field.column.clone(),
                    to_column: next_field.column.clone(),
                });
            }
            if let Some(aspects) = column_aspects(prev_field, next_field) {
                changes.push(Change::AlterColumn {
                    model: model.clone(),
                    from: prev_field.clone(),
                    to: next_field.clone(),
                    aspects,
                });
            }
        }
    }
}

fn column_aspects(prev: &Field, next: &Field) -> Option<Vec<ColumnAspect>> {
    let mut aspects = Vec::new();

    if scalar_changed(&prev.kind, &next.kind) || prev.attrs.native_type != next.attrs.native_type {
        aspects.push(ColumnAspect::Type);
    }

    if prev.optional != next.optional {
        aspects.push(ColumnAspect::Nullability);
    }

    if prev.default != next.default {
        aspects.push(ColumnAspect::Default);
    }

    // We do not currently detect identity changes in the IR; identity is
    // folded into `is_id`/primary-key handling. Leave `Identity` unused for
    // now and let primary-key index changes cover it.

    if aspects.is_empty() {
        None
    } else {
        Some(aspects)
    }
}

fn scalar_changed(prev: &FieldKind, next: &FieldKind) -> bool {
    match (prev, next) {
        (FieldKind::Scalar(a), FieldKind::Scalar(b)) => a != b,
        (FieldKind::Enum(a), FieldKind::Enum(b)) => a != b,
        _ => prev != next,
    }
}

fn diff_indexes(model: &ModelName, prev: &Model, next: &Model, changes: &mut Vec<Change>) {
    for ix in &next.indexes {
        if !prev.indexes.iter().any(|p| p.db_name == ix.db_name) {
            changes.push(Change::CreateIndex(model.clone(), ix.clone()));
        }
    }
    for ix in &prev.indexes {
        if !next.indexes.iter().any(|n| n.db_name == ix.db_name) {
            changes.push(Change::DropIndex(model.clone(), ix.db_name.clone()));
        }
    }
}

fn diff_uniques(model: &ModelName, prev: &Model, next: &Model, changes: &mut Vec<Change>) {
    for uq in &next.uniques {
        if !prev.uniques.iter().any(|p| p.db_name == uq.db_name) {
            changes.push(Change::AddUnique(model.clone(), uq.clone()));
        }
    }
    for uq in &prev.uniques {
        if !next.uniques.iter().any(|n| n.db_name == uq.db_name) {
            changes.push(Change::DropUnique(model.clone(), uq.db_name.clone()));
        }
    }
}

fn diff_relations(prev: &Schema, next: &Schema, changes: &mut Vec<Change>) {
    let by_name = |r: &ResolvedRelation| r.name.clone();

    let mut prev_map: std::collections::HashMap<String, &ResolvedRelation> =
        std::collections::HashMap::new();
    for r in &prev.relations {
        prev_map.insert(by_name(r), r);
    }

    let mut next_map: std::collections::HashMap<String, &ResolvedRelation> =
        std::collections::HashMap::new();
    for r in &next.relations {
        next_map.insert(by_name(r), r);
    }

    let dropped_models: std::collections::HashSet<&str> = prev
        .models
        .iter()
        .filter(|(k, _)| !next.models.contains_key(k.as_str()))
        .map(|(k, _)| k.as_str())
        .collect();

    for (name, r) in &next_map {
        match prev_map.get(name) {
            None => changes.push(Change::AddForeignKey(r.owner.clone(), (*r).clone())),
            Some(p) => {
                if p.on_delete != r.on_delete
                    || p.on_update != r.on_update
                    || p.owner_cols != r.owner_cols
                    || p.target != r.target
                    || p.target_cols != r.target_cols
                    || p.constraint_name != r.constraint_name
                {
                    changes.push(Change::DropForeignKey(p.owner.clone(), (*p).clone()));
                    changes.push(Change::AddForeignKey(r.owner.clone(), (*r).clone()));
                }
            }
        }
    }

    for (name, r) in &prev_map {
        if dropped_models.contains(r.owner.as_str()) {
            // The owning table is being dropped; DROP TABLE removes the FK.
            continue;
        }
        if !next_map.contains_key(name) {
            changes.push(Change::DropForeignKey(r.owner.clone(), (*r).clone()));
        }
    }
}
