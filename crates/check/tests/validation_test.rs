use ruprizzle_check::{
    ParamSpec, QueryCheckError, QueryEntry, QueryManifest, ReportFormat, SourceLocation,
    format_report, validate_manifest,
};

const SCHEMA: &str = r#"
datasource db {
  provider = "postgres"
  url      = env("DATABASE_URL")
}

model User {
  id        Int      @id @default(autoincrement())
  email     String   @unique
  isActive  Boolean  @default(true)
  posts     Post[]
}

model Post {
  id        Int      @id @default(autoincrement())
  title     String
  content   String?
  authorId  Int
  author    User     @relation(fields: [authorId], references: [id])
}
"#;

#[test]
fn test_valid_manifest_passes() {
    let schema = ruprizzle_parser::parse("schema.ruprizzle", SCHEMA).unwrap();
    let manifest = QueryManifest {
        version: 1,
        schema_hash: String::new(),
        queries: vec![
            QueryEntry {
                id: Some("q1".to_owned()),
                sql: "SELECT users.id, users.email FROM users WHERE users.id = $1".to_owned(),
                dialect: "postgres".to_owned(),
                params: vec![ParamSpec {
                    name: Some("id".to_owned()),
                    position: 1,
                    expected_type: "Int".to_owned(),
                    nullable: false,
                }],
                result_columns: Vec::new(),
                source: Some("src/main.rs".to_owned()),
                line: Some(10),
                location: Some(SourceLocation {
                    file: "src/main.rs".to_owned(),
                    line: 10,
                    column: 5,
                }),
            },
            QueryEntry {
                id: Some("q2".to_owned()),
                sql: "SELECT * FROM posts INNER JOIN users ON posts.author_id = users.id"
                    .to_owned(),
                dialect: "postgres".to_owned(),
                params: Vec::new(),
                result_columns: Vec::new(),
                source: None,
                line: None,
                location: None,
            },
        ],
    };

    let errors = validate_manifest(&schema, &manifest);
    assert!(errors.is_empty(), "expected 0 errors, got {errors:?}");
}

#[test]
fn test_unknown_table_with_suggestion() {
    let schema = ruprizzle_parser::parse("schema.ruprizzle", SCHEMA).unwrap();
    let manifest = QueryManifest {
        version: 1,
        schema_hash: String::new(),
        queries: vec![QueryEntry {
            id: None,
            sql: "SELECT * FROM userz".to_owned(),
            dialect: "postgres".to_owned(),
            params: Vec::new(),
            result_columns: Vec::new(),
            source: None,
            line: None,
            location: None,
        }],
    };

    let errors = validate_manifest(&schema, &manifest);
    assert_eq!(errors.len(), 1);
    match &errors[0] {
        QueryCheckError::UnknownTable {
            table, suggestion, ..
        } => {
            assert_eq!(table, "userz");
            assert_eq!(suggestion.as_deref(), Some("users"));
        }
        other => panic!("expected UnknownTable, got {other:?}"),
    }
}

#[test]
fn test_unknown_column_with_suggestion() {
    let schema = ruprizzle_parser::parse("schema.ruprizzle", SCHEMA).unwrap();
    let manifest = QueryManifest {
        version: 1,
        schema_hash: String::new(),
        queries: vec![QueryEntry {
            id: None,
            sql: "SELECT users.emaiil FROM users".to_owned(),
            dialect: "postgres".to_owned(),
            params: Vec::new(),
            result_columns: Vec::new(),
            source: None,
            line: None,
            location: None,
        }],
    };

    let errors = validate_manifest(&schema, &manifest);
    assert_eq!(errors.len(), 1);
    match &errors[0] {
        QueryCheckError::UnknownColumn {
            column, suggestion, ..
        } => {
            assert_eq!(column, "emaiil");
            assert_eq!(suggestion.as_deref(), Some("email"));
        }
        other => panic!("expected UnknownColumn, got {other:?}"),
    }
}

#[test]
fn test_bind_parameter_type_mismatch() {
    let schema = ruprizzle_parser::parse("schema.ruprizzle", SCHEMA).unwrap();
    let manifest = QueryManifest {
        version: 1,
        schema_hash: String::new(),
        queries: vec![QueryEntry {
            id: None,
            sql: "SELECT * FROM users WHERE users.id = $1".to_owned(),
            dialect: "postgres".to_owned(),
            params: vec![ParamSpec {
                name: Some("id".to_owned()),
                position: 1,
                expected_type: "String".to_owned(), // Expected Int on model
                nullable: false,
            }],
            result_columns: Vec::new(),
            source: None,
            line: None,
            location: Some(SourceLocation {
                file: "src/queries.rs".to_owned(),
                line: 42,
                column: 8,
            }),
        }],
    };

    let errors = validate_manifest(&schema, &manifest);
    assert_eq!(errors.len(), 1);
    match &errors[0] {
        QueryCheckError::TypeMismatch {
            column,
            expected,
            received,
            ..
        } => {
            assert_eq!(column, "id");
            assert_eq!(expected, "Int");
            assert_eq!(received, "String");
        }
        other => panic!("expected TypeMismatch, got {other:?}"),
    }
}

#[test]
fn test_github_workflow_annotation_reporting() {
    let err = QueryCheckError::UnknownTable {
        sql: "SELECT * FROM ghost".to_owned(),
        table: "ghost".to_owned(),
        suggestion: None,
        location: Some(SourceLocation {
            file: "src/db/users.rs".to_owned(),
            line: 88,
            column: 12,
        }),
    };

    let output = format_report(&[err], ReportFormat::Github, "manifest.json");
    assert!(output.contains("::error file=src/db/users.rs,line=88,col=12,title=Ruprizzle Check: Unknown Table Reference::unknown table `ghost` in `SELECT * FROM ghost`"));
}
