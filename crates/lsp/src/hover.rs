//! Hover information for `schema.ruprizzle`.

use ruprizzle_core::Span;
use ruprizzle_core::ir::Schema;
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
                    value: format!(
                        "### `model {}`\n\nDatabase table entity definition.\n\n{}",
                        model.name, docs
                    ),
                }),
                range: None,
            });
        }
        for field in &model.fields {
            if contains(field.name_span, offset) {
                let docs = field.docs.as_deref().unwrap_or("");
                let arity_str = match field.arity {
                    ruprizzle_parser::ast::Arity::Optional => "?",
                    ruprizzle_parser::ast::Arity::List => "[]",
                    ruprizzle_parser::ast::Arity::Required => "",
                };
                return Some(Hover {
                    contents: HoverContents::Markup(MarkupContent {
                        kind: MarkupKind::Markdown,
                        value: format!(
                            "### `{}: {}{}`\n\nField on `{}`.\n\n{}",
                            field.name, field.type_name, arity_str, model.name, docs
                        ),
                    }),
                    range: None,
                });
            }
            if contains(field.type_span, offset) {
                let doc = type_documentation(&field.type_name, schema);
                return Some(Hover {
                    contents: HoverContents::Markup(MarkupContent {
                        kind: MarkupKind::Markdown,
                        value: doc,
                    }),
                    range: None,
                });
            }
            for attr in &field.attrs {
                if contains(attr.span, offset) {
                    let doc = attribute_documentation(&attr.path);
                    return Some(Hover {
                        contents: HoverContents::Markup(MarkupContent {
                            kind: MarkupKind::Markdown,
                            value: doc,
                        }),
                        range: None,
                    });
                }
            }
        }
        for attr in &model.block_attrs {
            if contains(attr.span, offset) {
                let doc = block_attribute_documentation(&attr.path);
                return Some(Hover {
                    contents: HoverContents::Markup(MarkupContent {
                        kind: MarkupKind::Markdown,
                        value: doc,
                    }),
                    range: None,
                });
            }
        }
    }

    None
}

fn type_documentation(type_name: &str, schema: Option<&Schema>) -> String {
    let base = type_name.trim_end_matches("[]").trim_end_matches('?');
    match base {
        "Int" => "### `Int`\n\n32-bit signed integer (`i32` in Rust, `INTEGER` / `INT` in SQL).".to_owned(),
        "BigInt" => "### `BigInt`\n\n64-bit signed integer (`i64` in Rust, `BIGINT` in SQL).".to_owned(),
        "String" => "### `String`\n\nUTF-8 text / string (`String` in Rust, `TEXT` / `VARCHAR` in SQL).".to_owned(),
        "Boolean" => "### `Boolean`\n\nBoolean value (`bool` in Rust, `BOOLEAN` / `TINYINT(1)` in SQL).".to_owned(),
        "DateTime" => "### `DateTime`\n\nUTC timestamp (`chrono::DateTime<Utc>` in Rust, `TIMESTAMPTZ` / `DATETIME` in SQL).".to_owned(),
        "Date" => "### `Date`\n\nCalendar date without timezone (`chrono::NaiveDate` in Rust, `DATE` in SQL).".to_owned(),
        "Time" => "### `Time`\n\nTime of day (`chrono::NaiveTime` in Rust, `TIME` in SQL).".to_owned(),
        "Decimal" => "### `Decimal`\n\nArbitrary-precision fixed-point decimal (`rust_decimal::Decimal` in Rust, `NUMERIC` / `DECIMAL` in SQL).".to_owned(),
        "Float" => "### `Float`\n\n64-bit floating point number (`f64` in Rust, `DOUBLE PRECISION` / `REAL` in SQL).".to_owned(),
        "Uuid" => "### `Uuid`\n\nUniversally Unique Identifier (`uuid::Uuid` in Rust, `UUID` / `TEXT` in SQL).".to_owned(),
        "Json" => "### `Json`\n\nArbitrary JSON structure (`serde_json::Value` in Rust, `JSONB` / `JSON` in SQL).".to_owned(),
        "Bytes" => "### `Bytes`\n\nBinary blob (`Vec<u8>` in Rust, `BYTEA` / `BLOB` in SQL).".to_owned(),
        other => {
            if let Some(schema) = schema {
                if schema.model(other).is_some() {
                    return format!("### Model Relation `{other}`\n\nReferences model `{other}`.");
                }
                if schema.enum_def(other).is_some() {
                    return format!("### Enum Type `{other}`\n\nEnumeration type `{other}`.");
                }
            }
            format!("### Type `{type_name}`")
        }
    }
}

fn attribute_documentation(attr_path: &str) -> String {
    match attr_path {
        "id" => "### `@id`\n\nMarks this field as the primary key of the model.\n\n**Dialects:** Postgres, SQLite, MySQL".to_owned(),
        "default" => "### `@default(...)`\n\nSets a default value expression when inserting records.\n\nExamples: `@default(autoincrement())`, `@default(uuid())`, `@default(now())`, `@default(\"active\")`.".to_owned(),
        "unique" => "### `@unique`\n\nAdds a unique constraint on this field preventing duplicate entries.".to_owned(),
        "updatedAt" => "### `@updatedAt`\n\nAutomatically manages timestamp whenever the record is updated.".to_owned(),
        "deletedAt" => "### `@deletedAt`\n\nEnables declarative soft deletes. Queries automatically exclude soft-deleted records unless `.with_deleted()` is called.".to_owned(),
        "createdAt" => "### `@createdAt`\n\nConvenience attribute for automatically storing the row creation timestamp.".to_owned(),
        "relation" => "### `@relation(fields: [...], references: [...])`\n\nSpecifies the foreign key binding and referential actions for relational navigation.".to_owned(),
        "map" => "### `@map(\"...\")`\n\nMaps this schema field to a different physical database column name.".to_owned(),
        "ignore" => "### `@ignore`\n\nOmits this field from the generated Rust client interface.".to_owned(),
        _ => format!("### `@{attr_path}`\n\nField attribute directive."),
    }
}

fn block_attribute_documentation(attr_path: &str) -> String {
    match attr_path {
        "id" => "### `@@id([...])`\n\nDeclares a composite primary key consisting of multiple columns.".to_owned(),
        "unique" => "### `@@unique([...])`\n\nDeclares a composite unique constraint across multiple columns.".to_owned(),
        "index" => "### `@@index([...])`\n\nDeclares a composite or single-column database index.".to_owned(),
        "map" => "### `@@map(\"...\")`\n\nMaps this model to a different physical database table name.".to_owned(),
        "tenant" => "### `@@tenant(field)`\n\nDeclares this table as multi-tenant, partitioned by the specified field.".to_owned(),
        "policy" => "### `@@policy(...)`\n\nDeclares row-level security policies (RLS) for fine-grained access control.".to_owned(),
        _ => format!("### `@@{attr_path}`\n\nModel block attribute directive."),
    }
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
