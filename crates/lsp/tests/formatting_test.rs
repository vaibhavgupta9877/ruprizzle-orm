use ruprizzle_lsp::format::format_schema;

#[test]
fn test_format_aligns_model_columns() {
    let unformatted = r#"model User {
id String @id @default(uuid())
email String @unique
createdAt DateTime @default(now())
}
"#;

    let formatted = format_schema(unformatted);
    assert!(formatted.contains("  id        String   @id @default(uuid())"));
    assert!(formatted.contains("  email     String   @unique"));
    assert!(formatted.contains("  createdAt DateTime @default(now())"));
}

#[test]
fn test_format_is_idempotent() {
    let source = r#"datasource db {
  provider = "postgres"
  url      = env("DATABASE_URL")
}

model User {
  id        String   @id @default(uuid())
  email     String   @unique
  createdAt DateTime @default(now())
}
"#;

    let pass1 = format_schema(source);
    let pass2 = format_schema(&pass1);
    assert_eq!(pass1, pass2);
}
