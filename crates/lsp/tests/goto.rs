use tower_lsp::lsp_types::{GotoDefinitionResponse, Position, Url};

#[test]
fn goto_definition_resolves_model_reference() {
    let source = r#"datasource db {
  provider = "postgres"
  url      = env("DATABASE_URL")
}

model Post {
  id     Uuid   @id @default(uuid7())
  author User
}

model User {
  id    Uuid   @id @default(uuid7())
  email String @unique
}
"#;
    let pos = line_col_to_position(source, 7, 11); // on "User" in author User
    let uri: Url = "file:///schema.ruprizzle".parse().unwrap();

    let schema = ruprizzle_parser::parse("schema.ruprizzle", source).ok();
    let response = ruprizzle_lsp::goto::goto_definition(&uri, source, schema.as_ref(), pos);

    assert!(response.is_some(), "expected a goto-definition response");
    if let Some(GotoDefinitionResponse::Scalar(loc)) = response {
        assert_eq!(loc.uri, uri);
    } else {
        panic!("expected scalar location; got {response:?}");
    }
}

fn line_col_to_position(_text: &str, line: u32, character: u32) -> Position {
    Position { line, character }
}
