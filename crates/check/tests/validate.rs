use ruprizzle_check::{QueryCheckError, QueryEntry, QueryManifest, validate_manifest};

const SCHEMA: &str = r#"
datasource db {
  provider = "sqlite"
  url      = "file:app.db"
}

model User {
  id    Int      @id
  email String   @unique
  posts Post[]
}

model Post {
  id     Int    @id
  title  String
  userId Int
  user   User   @relation(fields: [userId], references: [id])
}
"#;

#[test]
fn validates_known_table_and_column() {
    let schema = ruprizzle_parser::parse("schema.ruprizzle", SCHEMA).unwrap();
    let manifest = QueryManifest {
        schema_hash: String::new(),
        queries: vec![QueryEntry {
            sql: "SELECT * FROM users".to_owned(),
            source: None,
            line: None,
            dialect: "sqlite".to_owned(),
        }],
    };
    assert!(validate_manifest(&schema, &manifest).is_empty());
}

#[test]
fn rejects_unknown_table() {
    let schema = ruprizzle_parser::parse("schema.ruprizzle", SCHEMA).unwrap();
    let manifest = QueryManifest {
        schema_hash: String::new(),
        queries: vec![QueryEntry {
            sql: "SELECT * FROM not_a_table".to_owned(),
            source: None,
            line: None,
            dialect: "sqlite".to_owned(),
        }],
    };
    let errors = validate_manifest(&schema, &manifest);
    assert_eq!(errors.len(), 1);
    assert!(matches!(errors[0], QueryCheckError::UnknownTable { .. }));
}

#[test]
fn rejects_unknown_column() {
    let schema = ruprizzle_parser::parse("schema.ruprizzle", SCHEMA).unwrap();
    let manifest = QueryManifest {
        schema_hash: String::new(),
        queries: vec![QueryEntry {
            sql: "SELECT users.not_a_column FROM users".to_owned(),
            source: None,
            line: None,
            dialect: "sqlite".to_owned(),
        }],
    };
    let errors = validate_manifest(&schema, &manifest);
    assert_eq!(errors.len(), 1);
    assert!(matches!(errors[0], QueryCheckError::UnknownColumn { .. }));
}
