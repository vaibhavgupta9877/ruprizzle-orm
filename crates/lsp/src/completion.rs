//! Completion for `schema.ruprizzle`.

use ruprizzle_core::ir::{ScalarType, Schema};
use tower_lsp::lsp_types::{
    CompletionItem, CompletionItemKind, CompletionList, CompletionResponse, InsertTextFormat,
    Position,
};

/// Build completion items for the given source and cursor position.
#[allow(clippy::too_many_lines, clippy::cast_possible_truncation)]
#[must_use]
pub fn complete(
    text: &str,
    schema: Option<&Schema>,
    position: Position,
) -> Option<CompletionResponse> {
    let offset = position_to_byte_offset(text, position);
    let before = &text[..offset];
    let current_line_start = before.rfind('\n').map_or(0, |i| i + 1);
    let current_line = &before[current_line_start..];

    // Inside relation argument list: @relation(...)
    if current_line.contains("@relation(") {
        let (in_model, model_name) = nearest_model_context(text, offset);
        if in_model {
            if let Some(name) = &model_name {
                if let Some(schema) = schema {
                    if let Some(model) = schema.model(name) {
                        // If typing fields: [
                        if current_line.contains("fields:") {
                            let items = model
                                .fields
                                .keys()
                                .map(|f| CompletionItem {
                                    label: f.as_str().to_owned(),
                                    kind: Some(CompletionItemKind::FIELD),
                                    detail: Some("scalar field of current model".to_owned()),
                                    ..CompletionItem::default()
                                })
                                .collect();
                            return Some(CompletionResponse::List(CompletionList {
                                is_incomplete: false,
                                items,
                            }));
                        }
                    }
                }
            }
        }

        return Some(CompletionResponse::List(CompletionList {
            is_incomplete: false,
            items: relation_argument_items(),
        }));
    }

    // Inside an attribute argument list or after an attribute trigger.
    if current_line.trim_start().starts_with("@@") {
        return Some(CompletionResponse::List(CompletionList {
            is_incomplete: false,
            items: model_attribute_items(),
        }));
    }
    if current_line.trim_start().starts_with('@') {
        return Some(CompletionResponse::List(CompletionList {
            is_incomplete: false,
            items: field_attribute_items(),
        }));
    }

    // Determine whether we are inside a model block.
    let (in_model, model_name) = nearest_model_context(text, offset);

    if in_model {
        // If the line has two bare words already, the user is likely typing an attribute.
        let trimmed = current_line.trim_start();
        let parts: Vec<&str> = trimmed.split_whitespace().collect();
        if parts.len() >= 2 && !parts.iter().any(|p| p.starts_with('@')) {
            return Some(CompletionResponse::List(CompletionList {
                is_incomplete: false,
                items: field_attribute_items(),
            }));
        }

        // Otherwise complete a field type.
        let mut items = scalar_type_items();
        if let Some(schema) = schema {
            for model in schema.models.keys() {
                items.push(CompletionItem {
                    label: model.as_str().to_owned(),
                    kind: Some(CompletionItemKind::CLASS),
                    detail: Some("model relation".to_owned()),
                    ..CompletionItem::default()
                });
                items.push(CompletionItem {
                    label: format!("{}[]", model.as_str()),
                    kind: Some(CompletionItemKind::CLASS),
                    detail: Some("list relation".to_owned()),
                    ..CompletionItem::default()
                });
            }
            for enm in schema.enums.keys() {
                items.push(CompletionItem {
                    label: enm.as_str().to_owned(),
                    kind: Some(CompletionItemKind::ENUM),
                    detail: Some("enum type".to_owned()),
                    ..CompletionItem::default()
                });
            }
        }

        // If we know the model, offer its field names inside an index/unique list.
        if current_line.contains('[') && !current_line.contains(']') {
            if let Some(name) = model_name {
                if let Some(schema) = schema {
                    if let Some(model) = schema.model(&name) {
                        items = model
                            .fields
                            .keys()
                            .map(|f| CompletionItem {
                                label: f.as_str().to_owned(),
                                kind: Some(CompletionItemKind::FIELD),
                                detail: Some("model field".to_owned()),
                                ..CompletionItem::default()
                            })
                            .collect();
                    }
                }
            }
        }

        return Some(CompletionResponse::List(CompletionList {
            is_incomplete: false,
            items,
        }));
    }

    // Top-level keywords.
    Some(CompletionResponse::List(CompletionList {
        is_incomplete: false,
        items: top_level_items(schema),
    }))
}

fn position_to_byte_offset(text: &str, pos: Position) -> usize {
    let mut line = 0;
    let mut character = 0;
    for (i, c) in text.char_indices() {
        if line == pos.line && character == pos.character {
            return i;
        }
        if c == '\n' {
            line += 1;
            character = 0;
        } else {
            character += 1;
        }
    }
    text.len()
}

fn nearest_model_context(text: &str, offset: usize) -> (bool, Option<String>) {
    let mut in_model = false;
    let mut name: Option<String> = None;
    let mut brace_depth = 0;
    let mut tokens: Vec<String> = Vec::new();
    for (i, c) in text.char_indices() {
        if c.is_whitespace() {
            continue;
        }
        if c == '{' {
            brace_depth += 1;
            if brace_depth == 1 {
                // The token before this brace might be the model name.
                if let Some(word) = tokens.last() {
                    name = Some(word.clone());
                }
            }
        } else if c == '}' {
            brace_depth -= 1;
        } else if c.is_alphanumeric() || c == '_' {
            let start = i;
            let mut end = i;
            for (j, ch) in text[i..].char_indices() {
                if ch.is_alphanumeric() || ch == '_' {
                    end = start + j + ch.len_utf8();
                } else {
                    break;
                }
            }
            let word = &text[start..end];
            tokens.push(word.to_owned());
            if i + word.len() < offset && word == "model" {
                in_model = true;
                name = None;
            }
            continue;
        }
        if i >= offset {
            break;
        }
    }
    (brace_depth > 0 && in_model, name)
}

fn top_level_items(schema: Option<&Schema>) -> Vec<CompletionItem> {
    let mut items = vec![
        keyword_snippet(
            "datasource",
            "datasource db {\n  provider = \"$1\"\n  url      = env(\"$2\")\n}",
            "database connection block",
        ),
        keyword_snippet(
            "generator",
            "generator client {\n  output      = \"$1\"\n  module_name = \"$2\"\n}",
            "code generator settings",
        ),
        keyword_snippet(
            "model",
            "model $1 {\n  id    Int    @id @default(autoincrement())\n  $0\n}",
            "declare a database model",
        ),
        keyword_snippet("enum", "enum $1 {\n  $0\n}", "declare an enumeration type"),
    ];
    if let Some(schema) = schema {
        for model in schema.models.keys() {
            items.push(CompletionItem {
                label: model.as_str().to_owned(),
                kind: Some(CompletionItemKind::CLASS),
                detail: Some("model".to_owned()),
                ..CompletionItem::default()
            });
        }
    }
    items
}

fn keyword_snippet(label: &str, snippet: &str, detail: &str) -> CompletionItem {
    CompletionItem {
        label: label.to_owned(),
        kind: Some(CompletionItemKind::SNIPPET),
        insert_text: Some(snippet.to_owned()),
        insert_text_format: Some(InsertTextFormat::SNIPPET),
        detail: Some(detail.to_owned()),
        ..CompletionItem::default()
    }
}

fn scalar_type_items() -> Vec<CompletionItem> {
    ScalarType::ALL
        .iter()
        .map(|t| CompletionItem {
            label: t.as_str().to_owned(),
            kind: Some(CompletionItemKind::TYPE_PARAMETER),
            detail: Some("scalar type".to_owned()),
            ..CompletionItem::default()
        })
        .collect()
}

fn field_attribute_items() -> Vec<CompletionItem> {
    vec![
        attr("@id", "primary key constraint"),
        attr_snippet(
            "@default",
            "@default($1)",
            "default column value expression",
        ),
        attr("@unique", "unique column constraint"),
        attr_snippet(
            "@relation",
            "@relation(fields: [$1], references: [$2])",
            "define foreign key relation",
        ),
        attr_snippet(
            "@map",
            "@map(\"$1\")",
            "map to a different physical column name",
        ),
        attr("@updatedAt", "automatically updated timestamp on row edit"),
        attr("@deletedAt", "declarative soft-delete timestamp field"),
        attr("@createdAt", "creation timestamp default value"),
        attr("@ignore", "exclude from generated client"),
    ]
}

fn model_attribute_items() -> Vec<CompletionItem> {
    vec![
        attr_snippet("@@index", "@@index([$1])", "table secondary index"),
        attr_snippet(
            "@@unique",
            "@@unique([$1])",
            "table composite unique constraint",
        ),
        attr_snippet("@@id", "@@id([$1])", "table composite primary key"),
        attr_snippet(
            "@@map",
            "@@map(\"$1\")",
            "map to a different physical table name",
        ),
        attr_snippet(
            "@@tenant",
            "@@tenant($1)",
            "declare multi-tenant partition key",
        ),
        attr_snippet(
            "@@policy",
            "@@policy($1, for: $2, using: \"$3\")",
            "declare row-level security policy",
        ),
    ]
}

fn relation_argument_items() -> Vec<CompletionItem> {
    vec![
        attr_snippet("fields", "fields: [$1]", "local foreign key fields"),
        attr_snippet(
            "references",
            "references: [$1]",
            "target referenced primary key fields",
        ),
        attr_snippet(
            "onDelete",
            "onDelete: Cascade",
            "referential action on delete (Cascade, SetNull, Restrict)",
        ),
        attr_snippet(
            "onUpdate",
            "onUpdate: Cascade",
            "referential action on update",
        ),
    ]
}

fn attr(label: &str, detail: &str) -> CompletionItem {
    CompletionItem {
        label: label.to_owned(),
        kind: Some(CompletionItemKind::PROPERTY),
        detail: Some(detail.to_owned()),
        ..CompletionItem::default()
    }
}

fn attr_snippet(label: &str, snippet: &str, detail: &str) -> CompletionItem {
    CompletionItem {
        label: label.to_owned(),
        kind: Some(CompletionItemKind::SNIPPET),
        insert_text: Some(snippet.to_owned()),
        insert_text_format: Some(InsertTextFormat::SNIPPET),
        detail: Some(detail.to_owned()),
        ..CompletionItem::default()
    }
}
