//! Conservative heuristic rename suggestions.

use ruprizzle_core::ir::{FieldKind, Schema};
use ruprizzle_core::names::ModelName;

use crate::Change;

/// A possible column rename inferred from a drop/add pair.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenameSuggestion {
    /// Model containing the column.
    pub model: ModelName,
    /// Previous field name.
    pub from: String,
    /// New field name.
    pub to: String,
    /// Previous physical column name.
    pub from_column: String,
    /// New physical column name.
    pub to_column: String,
    /// Why the pair was considered likely.
    pub reason: String,
}

/// Finds likely renames without changing the diff.
///
/// This function only reports candidates. Callers must ask the user before
/// turning one into a rename; the ordinary diff remains `DROP` + `ADD` until an
/// explicit `@renamedFrom` hint is present in the schema.
#[must_use]
pub fn suggest_renames(
    previous: &Schema,
    next: &Schema,
    changes: &[Change],
) -> Vec<RenameSuggestion> {
    let added = changes
        .iter()
        .filter_map(|change| match change {
            Change::AddColumn { model, field } => Some((model, field)),
            _ => None,
        })
        .collect::<Vec<_>>();
    let dropped = changes
        .iter()
        .filter_map(|change| match change {
            Change::DropColumn { model, column } => Some((model, column)),
            _ => None,
        })
        .collect::<Vec<_>>();

    let mut suggestions = Vec::new();
    for (model, field) in &added {
        let Some(previous_model) = previous.models.get(*model) else {
            continue;
        };
        let Some(next_model) = next.models.get(*model) else {
            continue;
        };
        if next_model.field(field.name.as_str()).is_none() {
            continue;
        }
        let candidates = dropped
            .iter()
            .filter(|(dropped_model, _)| *dropped_model == *model)
            .filter_map(|(_, column)| {
                let previous_field = previous_model
                    .scalar_fields()
                    .find(|candidate| candidate.column == **column)?;
                let score = candidate_score(previous_field, field);
                (score >= 6 || (score >= 4 && candidates_are_unambiguous(&added, &dropped, model)))
                    .then_some((previous_field, score))
            })
            .max_by_key(|(_, score)| *score);

        let Some((previous_field, score)) = candidates else {
            continue;
        };
        suggestions.push(RenameSuggestion {
            model: (*model).clone(),
            from: previous_field.name.to_string(),
            to: field.name.to_string(),
            from_column: previous_field.column.clone(),
            to_column: field.column.clone(),
            reason: format!("same type and nullability; field similarity score {score}/8"),
        });
    }
    suggestions
}

fn candidates_are_unambiguous(
    added: &[(&ModelName, &ruprizzle_core::ir::Field)],
    dropped: &[(&ModelName, &String)],
    model: &ModelName,
) -> bool {
    added
        .iter()
        .filter(|(candidate, _)| *candidate == model)
        .count()
        == 1
        && dropped
            .iter()
            .filter(|(candidate, _)| *candidate == model)
            .count()
            == 1
}

fn candidate_score(previous: &ruprizzle_core::ir::Field, next: &ruprizzle_core::ir::Field) -> u8 {
    let mut score = 0;
    if same_kind(&previous.kind, &next.kind) {
        score += 3;
    } else {
        return 0;
    }
    if previous.optional == next.optional {
        score += 1;
    }
    if previous.default == next.default {
        score += 1;
    }
    score + name_similarity(&previous.name.to_string(), &next.name.to_string()).min(3)
}

fn same_kind(previous: &FieldKind, next: &FieldKind) -> bool {
    match (previous, next) {
        (FieldKind::Scalar(a), FieldKind::Scalar(b)) => a == b,
        (FieldKind::Enum(a), FieldKind::Enum(b)) => a == b,
        _ => false,
    }
}

fn name_similarity(previous: &str, next: &str) -> u8 {
    let previous = previous.to_ascii_lowercase();
    let next = next.to_ascii_lowercase();
    let distance = levenshtein(&previous, &next);
    let max_len = previous.len().max(next.len());
    if max_len == 0 {
        return 3;
    }
    if distance == 0 {
        3
    } else if distance * 2 <= max_len
        || previous.contains(&next)
        || next.contains(&previous)
    {
        2
    } else {
        u8::from(common_prefix(&previous, &next) >= 3)
    }
}

fn common_prefix(left: &str, right: &str) -> usize {
    left.chars()
        .zip(right.chars())
        .take_while(|(left, right)| left == right)
        .count()
}

fn levenshtein(left: &str, right: &str) -> usize {
    let mut row = (0..=right.chars().count()).collect::<Vec<_>>();
    for (i, left) in left.chars().enumerate() {
        let mut diagonal = row[0];
        row[0] = i + 1;
        for (j, right) in right.chars().enumerate() {
            let above = row[j + 1];
            row[j + 1] = if left == right {
                diagonal
            } else {
                1 + diagonal.min(above).min(row[j])
            };
            diagonal = above;
        }
    }
    row[right.chars().count()]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diff::diff;

    fn schema(name: &str) -> Schema {
        let field = if name == "next" {
            "displayName"
        } else {
            "name"
        };
        let source = format!(
            r#"
            datasource db {{ provider = "sqlite" url = "sqlite::memory:" }}
            generator client {{ provider = "rust" }}
            model User {{
                id Int @id
                {field} String
            }}
            "#
        );
        ruprizzle_parser::parse(name, &source).unwrap()
    }

    #[test]
    fn suggests_matching_drop_and_add_without_mutating_diff() {
        let previous = schema("previous");
        let next = schema("next");
        let changes = diff(&previous, &next);
        assert!(
            changes
                .iter()
                .any(|change| matches!(change, Change::DropColumn { .. }))
        );
        assert!(
            changes
                .iter()
                .any(|change| matches!(change, Change::AddColumn { .. }))
        );

        let suggestions = suggest_renames(&previous, &next, &changes);
        assert_eq!(suggestions.len(), 1);
        assert_eq!(suggestions[0].from, "name");
        assert_eq!(suggestions[0].to, "displayName");
        assert_eq!(suggestions[0].from_column, "name");
    }

    #[test]
    fn does_not_suggest_different_scalar_types() {
        let previous = schema("previous");
        let source = r#"
            datasource db { provider = "sqlite" url = "sqlite::memory:" }
            generator client { provider = "rust" }
            model User {
                id Int @id
                age Int
            }
        "#;
        let next = ruprizzle_parser::parse("next", source).unwrap();
        let changes = diff(&previous, &next);
        assert!(suggest_renames(&previous, &next, &changes).is_empty());
    }
}
