//! Go-to-definition for `schema.ruprizzle`.

use ruprizzle_core::Span;
use ruprizzle_core::ir::Schema;
use ruprizzle_parser::ast::Ast;
use tower_lsp::lsp_types::{GotoDefinitionResponse, Location, Position, Range, Url};

/// Resolve the target location for the symbol at `position`.
#[must_use]
pub fn goto_definition(
    uri: &Url,
    text: &str,
    schema: Option<&Schema>,
    position: Position,
) -> Option<GotoDefinitionResponse> {
    let offset = position_to_byte_offset(text, position);

    let Ok(ast) = ruprizzle_parser::parse_ast(uri.path(), text) else {
        return None;
    };

    for model in ast.models() {
        if contains(model.name_span, offset) {
            return None; // self-definition
        }
        for field in &model.fields {
            if contains(field.type_span, offset) {
                let target = field.type_name.trim_end_matches("[]").trim_end_matches('?');
                return resolve_target(uri, text, schema, &ast, target);
            }
        }
    }

    None
}

fn resolve_target(
    base: &Url,
    text: &str,
    schema: Option<&Schema>,
    ast: &Ast,
    name: &str,
) -> Option<GotoDefinitionResponse> {
    for model in ast.models() {
        if model.name == name {
            return Some(GotoDefinitionResponse::Scalar(Location {
                uri: base.clone(),
                range: span_to_range(text, model.name_span),
            }));
        }
    }
    for enm in ast.enums() {
        if enm.name == name {
            return Some(GotoDefinitionResponse::Scalar(Location {
                uri: base.clone(),
                range: span_to_range(text, enm.name_span),
            }));
        }
    }

    if let Some(schema) = schema {
        if let Some(model) = schema.model(name) {
            return Some(GotoDefinitionResponse::Scalar(Location {
                uri: base.clone(),
                range: span_to_range(text, model.span),
            }));
        }
        if let Some(enm) = schema.enum_def(name) {
            return Some(GotoDefinitionResponse::Scalar(Location {
                uri: base.clone(),
                range: span_to_range(text, enm.span),
            }));
        }
    }

    None
}

fn contains(span: Span, offset: usize) -> bool {
    span.start <= offset && offset < span.end
}

fn span_to_range(text: &str, span: Span) -> Range {
    let start = byte_offset_to_position(text, span.start);
    let end = byte_offset_to_position(text, span.end);
    Range { start, end }
}

fn byte_offset_to_position(text: &str, offset: usize) -> Position {
    let mut line = 0;
    let mut character = 0;
    for (i, c) in text.char_indices() {
        if i >= offset {
            break;
        }
        if c == '\n' {
            line += 1;
            character = 0;
        } else {
            character += 1;
        }
    }
    Position { line, character }
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
