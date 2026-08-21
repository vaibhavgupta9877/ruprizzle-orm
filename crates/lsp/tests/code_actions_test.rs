use tower_lsp::lsp_types::{CodeActionOrCommand, Position, Range, Url};

#[test]
fn test_code_action_suggests_type_typo_fix() {
    let source = r#"model User {
  id int @id
}
"#;
    let uri: Url = "file:///schema.ruprizzle".parse().unwrap();
    let range = Range {
        start: Position {
            line: 1,
            character: 5,
        },
        end: Position {
            line: 1,
            character: 8,
        },
    };

    let actions = ruprizzle_lsp::code_actions::code_actions(&uri, source, None, range);
    assert!(!actions.is_empty(), "expected at least one code action");

    let has_int_fix = actions.iter().any(|a| match a {
        CodeActionOrCommand::CodeAction(ca) => ca.title.contains("Change type `int` to `Int`"),
        _ => false,
    });
    assert!(has_int_fix, "expected typo quick-fix in {actions:?}");
}

#[test]
fn test_code_action_suggests_inverse_relation() {
    let source = r#"datasource db {
  provider = "postgres"
  url      = env("DATABASE_URL")
}

model Post {
  id     Int  @id
  author User @relation(fields: [authorId], references: [id])
}

model User {
  id Int @id
}
"#;
    let uri: Url = "file:///schema.ruprizzle".parse().unwrap();
    let range = Range {
        start: Position {
            line: 6,
            character: 2,
        },
        end: Position {
            line: 6,
            character: 10,
        },
    };

    let schema = ruprizzle_parser::parse("schema.ruprizzle", source).ok();
    let actions = ruprizzle_lsp::code_actions::code_actions(&uri, source, schema.as_ref(), range);

    let has_inverse_fix = actions.iter().any(|a| match a {
        CodeActionOrCommand::CodeAction(ca) => ca.title.contains("Add inverse relation"),
        _ => false,
    });
    assert!(
        has_inverse_fix,
        "expected inverse relation quick fix in {actions:?}"
    );
}
