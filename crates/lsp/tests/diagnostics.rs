use ruprizzle_lsp::diagnostics::schema_error_to_diagnostic;

#[test]
fn missing_primary_key_diagnostic_mentions_primary_key() {
    let source = "model User { id String }";
    let errors = match ruprizzle_parser::parse("schema.ruprizzle", source) {
        Ok(_) => panic!("expected parse to fail without @id"),
        Err(errors) => errors,
    };

    let diagnostic = errors
        .errors
        .iter()
        .map(|e| schema_error_to_diagnostic(source, e))
        .find(|d| d.message.to_lowercase().contains("primary key"))
        .expect("expected a primary-key diagnostic");

    assert!(diagnostic.message.to_lowercase().contains("primary key"));
}
