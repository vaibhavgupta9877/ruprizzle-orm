//! Convert schema errors into LSP diagnostics.

use miette::Diagnostic;
use ruprizzle_core::SchemaError;
use tower_lsp::lsp_types::{
    Diagnostic as LspDiagnostic, DiagnosticSeverity, NumberOrString, Position, Range,
};

fn byte_offset_to_position(source: &str, offset: usize) -> Position {
    let mut line = 0;
    let mut character = 0;
    for (i, c) in source.char_indices() {
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

/// Convert a single `SchemaError` into an LSP diagnostic.
#[must_use]
pub fn schema_error_to_diagnostic(source: &str, err: &SchemaError) -> LspDiagnostic {
    let default_span = miette::LabeledSpan::new_with_span(None, miette::SourceSpan::from((0, 0)));
    let label = err
        .labels()
        .and_then(|mut iter| iter.next())
        .unwrap_or(default_span);
    let start = byte_offset_to_position(source, label.offset());
    let end = byte_offset_to_position(source, label.offset() + label.len());
    LspDiagnostic {
        range: Range { start, end },
        severity: if err.is_warning() {
            Some(DiagnosticSeverity::WARNING)
        } else {
            Some(DiagnosticSeverity::ERROR)
        },
        code: err.code().map(|c| NumberOrString::String(c.to_string())),
        source: Some("ruprizzle".into()),
        message: format!("{err}"),
        ..Default::default()
    }
}
