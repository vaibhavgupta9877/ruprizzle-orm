use tower_lsp::lsp_types::{CompletionResponse, Position};

#[test]
fn completion_offers_unique_attribute_after_field_type() {
    let source = r#"datasource db {
  provider = "postgres"
  url      = env("DATABASE_URL")
}

model User {
  id Int 
}
"#;
    let pos = line_col_to_position(source, 5, 9); // after "Int "

    let schema = ruprizzle_parser::parse("schema.ruprizzle", source).ok();
    let response = ruprizzle_lsp::completion::complete(source, schema.as_ref(), pos)
        .expect("completion should return a response");

    let items = match response {
        CompletionResponse::Array(items) => items,
        CompletionResponse::List(list) => list.items,
    };

    let labels: Vec<_> = items.iter().map(|i| i.label.as_str()).collect();
    assert!(
        labels.iter().any(|l| l.starts_with("@unique")),
        "expected @unique in completion items; got {labels:?}"
    );
}

/// The editor must not offer a block attribute the toolchain does not act on.
///
/// `@@tenant` and `@@policy` were offered while row-level security and
/// multi-tenancy were unimplemented. The grammar accepts them and lowering drops
/// them silently, so a developer who took the suggestion got no partitioning and
/// no error. See ProjectPlan/v2/ProductionReadinessV1_5.md section 4.7.
#[test]
fn completion_does_not_offer_unimplemented_block_attributes() {
    let source = r#"datasource db {
  provider = "postgres"
  url      = env("DATABASE_URL")
}

model User {
  id  Int    @id
  org String
  @@
}
"#;
    let pos = line_col_to_position(source, 8, 4); // after "  @@"

    let schema = ruprizzle_parser::parse("schema.ruprizzle", source).ok();
    let response = ruprizzle_lsp::completion::complete(source, schema.as_ref(), pos)
        .expect("completion should return a response");

    let items = match response {
        CompletionResponse::Array(items) => items,
        CompletionResponse::List(list) => list.items,
    };
    let labels: Vec<_> = items.iter().map(|i| i.label.as_str()).collect();

    assert!(
        labels.iter().any(|l| l.starts_with("@@index")),
        "the implemented block attributes must still be offered; got {labels:?}"
    );
    for unimplemented in ["@@tenant", "@@policy"] {
        assert!(
            !labels.iter().any(|l| l.starts_with(unimplemented)),
            "{unimplemented} is not implemented and must not be suggested; got {labels:?}"
        );
    }
}

fn line_col_to_position(_text: &str, line: u32, character: u32) -> Position {
    Position { line, character }
}
