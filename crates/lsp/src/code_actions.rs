//! Code actions and quick-fixes for `schema.ruprizzle`.

use std::collections::HashMap;

use ruprizzle_core::ir::Schema;
use tower_lsp::lsp_types::{
    CodeAction, CodeActionKind, CodeActionOrCommand, Position, Range, TextEdit, Url, WorkspaceEdit,
};

/// Computes available code actions / quick fixes for the document at the given range.
#[allow(clippy::cast_possible_truncation)]
#[must_use]
pub fn code_actions(
    uri: &Url,
    text: &str,
    schema: Option<&Schema>,
    range: Range,
) -> Vec<CodeActionOrCommand> {
    let mut actions = Vec::new();

    // 1. Missing Primary Key Quick-Fix
    if let Some(schema) = schema {
        for (model_name, model) in &schema.models {
            if model.primary_key.fields.is_empty() {
                if let Some(action) = make_add_pk_action(uri, text, model_name.as_str()) {
                    actions.push(CodeActionOrCommand::CodeAction(action));
                }
            }
        }
    }

    // 2. Type Typo Quick-Fixes
    let line_idx = range.start.line as usize;
    if let Some(line) = text.lines().nth(line_idx) {
        if let Some(action) = make_type_typo_action(uri, line, line_idx as u32) {
            actions.push(CodeActionOrCommand::CodeAction(action));
        }
    }

    // 3. Inverse Relation Quick-Fixes
    if let Ok(ast) = ruprizzle_parser::parse_ast("schema.ruprizzle", text) {
        for model in ast.models() {
            for field in &model.fields {
                let target_model_name =
                    field.type_name.trim_end_matches("[]").trim_end_matches('?');
                if let Some(target) = ast.models().find(|m| m.name == target_model_name) {
                    // Check if target model has inverse relation back to current model
                    let has_inverse = target.fields.iter().any(|f| {
                        f.type_name.trim_end_matches("[]").trim_end_matches('?') == model.name
                    });

                    if !has_inverse && model.name != target.name {
                        if let Some(action) =
                            make_inverse_relation_action(uri, text, &target.name, &model.name)
                        {
                            actions.push(CodeActionOrCommand::CodeAction(action));
                        }
                    }
                }
            }
        }
    }

    actions
}

#[allow(clippy::cast_possible_truncation)]
fn make_add_pk_action(uri: &Url, text: &str, model_name: &str) -> Option<CodeAction> {
    let lines: Vec<&str> = text.lines().collect();
    for (idx, line) in lines.iter().enumerate() {
        if line.trim().starts_with(&format!("model {model_name}")) {
            let insert_pos = Position {
                line: (idx + 1) as u32,
                character: 0,
            };
            let mut changes = HashMap::new();
            changes.insert(
                uri.clone(),
                vec![TextEdit {
                    range: Range {
                        start: insert_pos,
                        end: insert_pos,
                    },
                    new_text: "  id        String   @id @default(uuid())\n".to_owned(),
                }],
            );

            return Some(CodeAction {
                title: format!("Add default primary key to `{model_name}`"),
                kind: Some(CodeActionKind::QUICKFIX),
                edit: Some(WorkspaceEdit {
                    changes: Some(changes),
                    ..WorkspaceEdit::default()
                }),
                is_preferred: Some(true),
                ..CodeAction::default()
            });
        }
    }
    None
}

#[allow(clippy::cast_possible_truncation)]
fn make_type_typo_action(uri: &Url, line: &str, line_idx: u32) -> Option<CodeAction> {
    let typos = [
        ("str", "String"),
        ("text", "String"),
        ("int", "Int"),
        ("integer", "Int"),
        ("bool", "Boolean"),
        ("boolean", "Boolean"),
        ("float", "Float"),
        ("double", "Float"),
        ("datetime", "DateTime"),
        ("json", "Json"),
    ];

    let parts: Vec<&str> = line.split_whitespace().collect();
    if parts.len() >= 2 {
        let current_type = parts[1];
        for (typo, fix) in typos {
            if current_type.eq_ignore_ascii_case(typo) && current_type != fix {
                let start_char = line.find(current_type)? as u32;
                let end_char = start_char + current_type.len() as u32;

                let mut changes = HashMap::new();
                changes.insert(
                    uri.clone(),
                    vec![TextEdit {
                        range: Range {
                            start: Position {
                                line: line_idx,
                                character: start_char,
                            },
                            end: Position {
                                line: line_idx,
                                character: end_char,
                            },
                        },
                        new_text: (*fix).to_owned(),
                    }],
                );

                return Some(CodeAction {
                    title: format!("Change type `{current_type}` to `{fix}`"),
                    kind: Some(CodeActionKind::QUICKFIX),
                    edit: Some(WorkspaceEdit {
                        changes: Some(changes),
                        ..WorkspaceEdit::default()
                    }),
                    is_preferred: Some(true),
                    ..CodeAction::default()
                });
            }
        }
    }
    None
}

#[allow(clippy::cast_possible_truncation)]
fn make_inverse_relation_action(
    uri: &Url,
    text: &str,
    target_model: &str,
    source_model: &str,
) -> Option<CodeAction> {
    let lines: Vec<&str> = text.lines().collect();
    for (idx, line) in lines.iter().enumerate() {
        if line.trim().starts_with(&format!("model {target_model}")) {
            let insert_pos = Position {
                line: (idx + 1) as u32,
                character: 0,
            };

            let field_name = source_model.to_lowercase();
            let new_field = format!("  {field_name}s {source_model}[]\n");

            let mut changes = HashMap::new();
            changes.insert(
                uri.clone(),
                vec![TextEdit {
                    range: Range {
                        start: insert_pos,
                        end: insert_pos,
                    },
                    new_text: new_field,
                }],
            );

            return Some(CodeAction {
                title: format!(
                    "Add inverse relation `{field_name}s {source_model}[]` on `{target_model}`"
                ),
                kind: Some(CodeActionKind::QUICKFIX),
                edit: Some(WorkspaceEdit {
                    changes: Some(changes),
                    ..WorkspaceEdit::default()
                }),
                is_preferred: Some(false),
                ..CodeAction::default()
            });
        }
    }
    None
}
