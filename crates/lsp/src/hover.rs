//! Hover information for `schema.ruprizzle`.

use ruprizzle_core::ir::Schema;
use ruprizzle_core::Span;
use tower_lsp::lsp_types::{Hover, HoverContents, MarkupContent, MarkupKind, Position};

/// Build hover information for the symbol at `position`.
#[must_use]
pub fn hover(text: &str, schema: Option<&Schema>, position: Position) -> Option<Hover> {
    let offset = position_to_byte_offset(text, position);

    let Ok(ast) = ruprizzle_parser::parse_ast("schema.ruprizzle", text) else {
        return None;
    };

    for model in ast.models() {
        if contains(model.name_span, offset) {
            let docs = model.docs.as_deref().unwrap_or("");
            return Some(Hover {
                contents: HoverContents::Markup(MarkupContent {
                    kind: MarkupKind::Markdown,
                    value: format!("**model** `{}`\n\n{}", model.name, docs),
                }),
                range: None,
            });
        }
        for field in &model.fields {
            if contains(field.name_span, offset) {
                let docs = field.docs.as_deref().unwrap_or("");
                let optional = if field.arity == ruprizzle_parser::ast::Arity::Optional {
                    "?"
                } else {
                    ""
                };
                let list = if field.arity == ruprizzle_parser::ast::Arity::List {
                    "[]"
                } else {
                    ""
                };
                return Some(Hover {
                    contents: HoverContents::Markup(MarkupContent {
                        kind: MarkupKind::Markdown,
                        value: format!(
                            "**field** `{}: {}{}{}\n\n{}",
                            field.name, field.type_name, optional, list, docs
                        ),
                    }),
                    range: None,
                });
            }
            if contains(field.type_span, offset) {
                let resolved = resolve_type(schema, &field.type_name);
                return Some(Hover {
                    contents: HoverContents::Markup(MarkupContent {
                        kind: MarkupKind::Markdown,
                        value: format!("**type** `{}{}`", field.type_name, resolved),
                    }),
                    range: None,
                });
            }
        }
    }

    None
}

fn resolve_type(schema: Option<&Schema>, name: &str) -> String {
    let base = name.trim_end_matches("[]").trim_end_matches('?');
    if let Some(schema) = schema {
        if schema.model(base).is_some() {
            return " — model".to_owned();
        }
        if schema.enum_def(base).is_some() {
            return " — enum".to_owned();
        }
    }
    String::new()
}

fn contains(span: Span, offset: usize) -> bool {
    span.start <= offset && offset < span.end
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
