//! Pest grammar unit tests.

use ruprizzle_parser::ast::{Arity, Value};
use ruprizzle_parser::parse_ast;

fn parse(src: &str) -> ruprizzle_parser::Ast {
    parse_ast("grammar.rs", src).unwrap()
}

#[test]
fn parses_every_production() {
    let src = r#"
// a line comment that must not eat the next doc comment
datasource db {
  provider = "postgres"
  url      = env("DATABASE_URL")
  strict   = true
}

generator client {
  output      = "src/db"
  module_name = "db"
}

/// A registered account.
enum Role {
  /// Ordinary user.
  USER
  ADMIN @map("admin")
}

/// A user.
model User {
  id        Uuid     @id @default(uuid7())
  email     String   @unique @db.VarChar(200)
  name      String?
  role      Role     @default(USER)
  posts     Post[]
  createdAt DateTime @default(now()) @map("created_at")

  @@index([email])
  @@map("users")
}

model Post {
  id       Uuid @id @default(uuid7())
  authorId Uuid @map("author_id")
  author   User @relation(fields: [authorId], references: [id], onDelete: Cascade)
}
"#;
    let ast = parse(src);
    assert_eq!(ast.decls.len(), 5);

    let user = ast.models().next().unwrap();
    assert_eq!(user.name, "User");
    assert_eq!(user.fields.len(), 6);
    assert_eq!(user.block_attrs.len(), 2);
    assert_eq!(user.docs.as_deref(), Some("A user."));
    assert_eq!(user.fields[4].arity, Arity::List);
    assert_eq!(user.fields[2].arity, Arity::Optional);

    let email = &user.fields[1];
    assert!(email.has_attr("unique"));
    let varchar = email.attr("db.VarChar").unwrap();
    assert_eq!(
        varchar.first_positional().map(Value::describe),
        Some("200".to_owned())
    );

    let role = ast.enums().next().unwrap();
    assert_eq!(role.docs.as_deref(), Some("A registered account."));
    assert_eq!(role.variants[0].docs.as_deref(), Some("Ordinary user."));
    assert_eq!(role.variants[1].map.as_deref(), Some("admin"));
}

#[test]
fn doc_comments_survive_the_comment_rule() {
    // Trap 1 from the plan: a `COMMENT` rule without the `!"///"` lookahead
    // silently swallows doc comments, producing empty rustdoc and no error.
    let ast = parse("/// kept\nmodel A {\n  id Uuid @id\n}\n");
    let model = ast.models().next().unwrap();
    assert_eq!(model.docs.as_deref(), Some("kept"));
}

#[test]
fn keywords_do_not_swallow_identifier_prefixes() {
    let ast = parse("model modelish {\n  id Uuid @id\n}\n");
    assert_eq!(ast.models().next().unwrap().name, "modelish");
}

#[test]
fn relation_arguments_keep_their_shape() {
    let ast = parse(
        "model Post {\n  author User @relation(\"written\", fields: [authorId], references: [id])\n}\n",
    );
    let field = &ast.models().next().unwrap().fields[0];
    let rel = field.attr("relation").unwrap();
    assert_eq!(
        rel.first_positional().and_then(Value::as_str),
        Some("written")
    );
    assert_eq!(
        rel.named("fields")
            .and_then(Value::as_array)
            .map(<[Value]>::len),
        Some(1)
    );
}

#[test]
fn malformed_input_reports_a_location() {
    let err = parse_ast("grammar.rs", "model User {\n  email @unique\n}\n").unwrap_err();
    let first = err.errors.first().unwrap();
    let src = err.src.inner();
    let span = match first {
        ruprizzle_core::SchemaError::Syntax { span, .. } => span,
        _ => panic!("expected a syntax error"),
    };
    let line = src[..span.offset()].chars().filter(|&c| c == '\n').count() + 1;
    assert_eq!(line, 2, "syntax error should be on line 2");
}
