//! Validate SQL queries against a `Schema` without a live database.

use std::collections::HashMap;

use ruprizzle_core::ir::{Model, ScalarType, Schema};

use crate::manifest::{QueryEntry, QueryManifest, SourceLocation};

/// An error found while validating a query offline.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QueryCheckError {
    /// A table referenced by the query does not exist in the schema.
    UnknownTable {
        /// SQL that contains the reference.
        sql: String,
        /// Table name that could not be found.
        table: String,
        /// Suggested table name if close.
        suggestion: Option<String>,
        /// Source location if available.
        location: Option<SourceLocation>,
    },
    /// A column referenced for a table does not exist.
    UnknownColumn {
        /// SQL that contains the reference.
        sql: String,
        /// Table that was expected to contain the column.
        table: String,
        /// Column that could not be found.
        column: String,
        /// Suggested column name if close.
        suggestion: Option<String>,
        /// Source location if available.
        location: Option<SourceLocation>,
    },
    /// A bind parameter type mismatch was detected.
    TypeMismatch {
        /// SQL that contains the parameter.
        sql: String,
        /// Table column name.
        column: String,
        /// Expected schema type.
        expected: String,
        /// Received/actual parameter type.
        received: String,
        /// Source location if available.
        location: Option<SourceLocation>,
    },
    /// A non-nullable column was assigned a nullable value without default.
    NullabilityViolation {
        /// SQL statement.
        sql: String,
        /// Column name.
        column: String,
        /// Table name.
        table: String,
        /// Source location if available.
        location: Option<SourceLocation>,
    },
    /// Schema fingerprint in manifest does not match current schema.
    StaleSchemaFingerprint {
        /// Manifest fingerprint.
        manifest_hash: String,
        /// Current schema fingerprint.
        schema_hash: String,
    },
}

impl std::fmt::Display for QueryCheckError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnknownTable {
                sql,
                table,
                suggestion,
                ..
            } => {
                write!(f, "unknown table `{table}` in `{sql}`")?;
                if let Some(sug) = suggestion {
                    write!(f, " (did you mean `{sug}`?)")?;
                }
                Ok(())
            }
            Self::UnknownColumn {
                sql,
                table,
                column,
                suggestion,
                ..
            } => {
                write!(f, "unknown column `{column}` on table `{table}` in `{sql}`")?;
                if let Some(sug) = suggestion {
                    write!(f, " (did you mean `{sug}`?)")?;
                }
                Ok(())
            }
            Self::TypeMismatch {
                sql,
                column,
                expected,
                received,
                ..
            } => {
                write!(
                    f,
                    "type mismatch for column `{column}` in `{sql}`: expected `{expected}`, received `{received}`"
                )
            }
            Self::NullabilityViolation {
                sql, column, table, ..
            } => {
                write!(
                    f,
                    "nullability violation on `{table}.{column}` in `{sql}`: non-nullable column received nullable value"
                )
            }
            Self::StaleSchemaFingerprint {
                manifest_hash,
                schema_hash,
            } => {
                write!(
                    f,
                    "stale query manifest: compiled against schema hash `{manifest_hash}`, current schema hash is `{schema_hash}`"
                )
            }
        }
    }
}

impl std::error::Error for QueryCheckError {}

impl QueryCheckError {
    /// Returns the source location of the error if known.
    #[must_use]
    pub fn location(&self) -> Option<&SourceLocation> {
        match self {
            Self::UnknownTable { location, .. }
            | Self::UnknownColumn { location, .. }
            | Self::TypeMismatch { location, .. }
            | Self::NullabilityViolation { location, .. } => location.as_ref(),
            Self::StaleSchemaFingerprint { .. } => None,
        }
    }

    /// Returns the error title or code.
    #[must_use]
    pub fn title(&self) -> &'static str {
        match self {
            Self::UnknownTable { .. } => "Unknown Table Reference",
            Self::UnknownColumn { .. } => "Unknown Column Reference",
            Self::TypeMismatch { .. } => "Query Parameter Type Mismatch",
            Self::NullabilityViolation { .. } => "Nullability Constraint Violation",
            Self::StaleSchemaFingerprint { .. } => "Stale Query Manifest",
        }
    }
}

/// Validate every query in `manifest` against `schema`.
#[must_use]
pub fn validate_manifest(schema: &Schema, manifest: &QueryManifest) -> Vec<QueryCheckError> {
    let mut errors = Vec::new();

    if !manifest.matches_schema(schema) {
        errors.push(QueryCheckError::StaleSchemaFingerprint {
            manifest_hash: manifest.schema_hash.clone(),
            schema_hash: schema.fingerprint(),
        });
    }

    for entry in &manifest.queries {
        errors.extend(validate_query_entry(schema, entry));
    }

    errors
}

/// Validates a single `QueryEntry` against `schema`.
#[must_use]
pub fn validate_query_entry(schema: &Schema, entry: &QueryEntry) -> Vec<QueryCheckError> {
    let mut errors = validate_raw_with_location(schema, &entry.sql, entry.location.as_ref());

    // Validate parameters if specified.
    if !entry.params.is_empty() {
        errors.extend(validate_params(schema, entry));
    }

    errors
}

/// Coarse validation of a raw SQL string against `schema`.
#[must_use]
pub fn validate_raw(schema: &Schema, sql: &str) -> Vec<QueryCheckError> {
    validate_raw_with_location(schema, sql, None)
}

fn validate_raw_with_location(
    schema: &Schema,
    sql: &str,
    location: Option<&SourceLocation>,
) -> Vec<QueryCheckError> {
    let mut errors = Vec::new();
    let tokens = tokenise(sql);

    let table_to_model: HashMap<&str, &Model> = schema
        .models
        .values()
        .map(|m| (m.table.as_str(), m))
        .collect();

    let all_tables: Vec<&str> = table_to_model.keys().copied().collect();

    for (idx, token) in tokens.iter().enumerate() {
        if let Some(model) = table_to_model.get(token.as_str()) {
            // If the next token is `.column`, verify the column exists.
            if let Some(next) = tokens.get(idx + 1) {
                if next == "." {
                    if let Some(column) = tokens.get(idx + 2) {
                        if column != "*" && !model.fields.values().any(|f| f.column == *column) {
                            let all_columns: Vec<&str> =
                                model.fields.values().map(|f| f.column.as_str()).collect();
                            let suggestion = find_best_match(column, &all_columns);
                            errors.push(QueryCheckError::UnknownColumn {
                                sql: sql.to_owned(),
                                table: token.to_owned(),
                                column: column.to_owned(),
                                suggestion,
                                location: location.cloned(),
                            });
                        }
                    }
                }
            }
            continue;
        }

        // A table name that is not a model but is followed by `.` is an
        // explicit schema reference (`schema.table`); skip it.
        if tokens.get(idx + 1).is_some_and(|next| next == ".") {
            continue;
        }

        // If the token is not a known model/table and is not a SQL keyword,
        // flag it as unknown only when it is plausibly a table reference.
        if !is_sql_keyword(token)
            && looks_like_identifier(token)
            && !table_to_model.contains_key(token.as_str())
            && idx > 0
            && is_table_context(&tokens[idx - 1])
        {
            let suggestion = find_best_match(token, &all_tables);
            errors.push(QueryCheckError::UnknownTable {
                sql: sql.to_owned(),
                table: token.to_owned(),
                suggestion,
                location: location.cloned(),
            });
        }
    }

    errors
}

fn validate_params(schema: &Schema, entry: &QueryEntry) -> Vec<QueryCheckError> {
    let mut errors = Vec::new();
    let tokens = tokenise(&entry.sql);

    // Try to find the target table context.
    let target_model = tokens.iter().enumerate().find_map(|(idx, tok)| {
        if idx > 0 && is_table_context(&tokens[idx - 1]) {
            schema
                .models
                .values()
                .find(|m| m.table == *tok || m.name.as_str() == *tok)
        } else {
            None
        }
    });

    if let Some(model) = target_model {
        for param in &entry.params {
            if let Some(name) = &param.name {
                if let Some(field) = model
                    .field(name)
                    .or_else(|| model.fields.values().find(|f| f.column == *name))
                {
                    let expected_scalar = match &field.kind {
                        ruprizzle_core::ir::FieldKind::Scalar(s) => Some(*s),
                        ruprizzle_core::ir::FieldKind::Enum(_) => Some(ScalarType::String),
                        ruprizzle_core::ir::FieldKind::List(inner) => match inner.as_ref() {
                            ruprizzle_core::ir::FieldKind::Scalar(s) => Some(*s),
                            ruprizzle_core::ir::FieldKind::Enum(_) => Some(ScalarType::String),
                            _ => None,
                        },
                        ruprizzle_core::ir::FieldKind::Relation(_) => None,
                    };

                    if let Some(expected) = expected_scalar {
                        if !is_type_compatible(expected, &param.expected_type) {
                            errors.push(QueryCheckError::TypeMismatch {
                                sql: entry.sql.clone(),
                                column: field.column.clone(),
                                expected: expected.as_str().to_owned(),
                                received: param.expected_type.clone(),
                                location: entry.location.clone(),
                            });
                        }
                    }

                    if !field.optional && param.nullable && field.default.is_none() {
                        errors.push(QueryCheckError::NullabilityViolation {
                            sql: entry.sql.clone(),
                            column: field.column.clone(),
                            table: model.table.clone(),
                            location: entry.location.clone(),
                        });
                    }
                }
            }
        }
    }

    errors
}

fn is_type_compatible(scalar: ScalarType, type_name: &str) -> bool {
    let norm = type_name.trim().to_lowercase();
    match scalar {
        ScalarType::Int => matches!(norm.as_str(), "int" | "i32" | "integer" | "i16" | "i8"),
        ScalarType::BigInt => matches!(norm.as_str(), "bigint" | "i64" | "int" | "i32"),
        ScalarType::Float => matches!(norm.as_str(), "float" | "f64" | "f32" | "double"),
        ScalarType::Decimal => matches!(
            norm.as_str(),
            "decimal" | "rust_decimal::decimal" | "string" | "str"
        ),
        ScalarType::String => matches!(norm.as_str(), "string" | "str" | "&str"),
        ScalarType::Boolean => matches!(norm.as_str(), "bool" | "boolean"),
        ScalarType::DateTime => matches!(
            norm.as_str(),
            "datetime" | "chrono::datetime<utc>" | "string" | "str"
        ),
        ScalarType::Date => matches!(norm.as_str(), "date" | "naivedate" | "string" | "str"),
        ScalarType::Time => matches!(norm.as_str(), "time" | "naivetime" | "string" | "str"),
        ScalarType::Uuid => matches!(norm.as_str(), "uuid" | "uuid::uuid" | "string" | "str"),
        ScalarType::Json => true,
        ScalarType::Bytes => matches!(norm.as_str(), "bytes" | "vec<u8>" | "&[u8]" | "blob"),
    }
}

fn tokenise(sql: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut in_string = false;
    let mut string_quote = '\0';

    for c in sql.chars() {
        if in_string {
            current.push(c);
            if c == string_quote {
                in_string = false;
            }
            continue;
        }

        if c == '\'' || c == '"' {
            if !current.is_empty() {
                tokens.push(std::mem::take(&mut current));
            }
            in_string = true;
            string_quote = c;
            current.push(c);
            continue;
        }

        if c.is_alphanumeric() || c == '_' || c == '*' {
            current.push(c);
        } else {
            if !current.is_empty() {
                tokens.push(std::mem::take(&mut current));
            }
            if c == '.' {
                tokens.push(".".to_owned());
            }
        }
    }

    if !current.is_empty() {
        tokens.push(current);
    }

    tokens
}

fn is_sql_keyword(token: &str) -> bool {
    const KEYWORDS: &[&str] = &[
        "SELECT",
        "FROM",
        "WHERE",
        "AND",
        "OR",
        "NOT",
        "INSERT",
        "UPDATE",
        "DELETE",
        "JOIN",
        "INNER",
        "LEFT",
        "RIGHT",
        "FULL",
        "OUTER",
        "ON",
        "GROUP",
        "BY",
        "ORDER",
        "LIMIT",
        "OFFSET",
        "HAVING",
        "VALUES",
        "SET",
        "AS",
        "WITH",
        "UNION",
        "ALL",
        "DISTINCT",
        "IS",
        "NULL",
        "TRUE",
        "FALSE",
        "IN",
        "BETWEEN",
        "LIKE",
        "EXISTS",
        "CASE",
        "WHEN",
        "THEN",
        "ELSE",
        "END",
        "RETURNING",
    ];
    KEYWORDS.iter().any(|&k| k == token.to_ascii_uppercase())
}

fn looks_like_identifier(token: &str) -> bool {
    token
        .chars()
        .next()
        .is_some_and(|c| c.is_alphabetic() || c == '_' || c == '"')
}

fn is_table_context(prev: &str) -> bool {
    matches!(
        prev.to_ascii_uppercase().as_str(),
        "FROM" | "JOIN" | "INTO" | "UPDATE" | "TABLE"
    )
}

fn find_best_match(candidate: &str, options: &[&str]) -> Option<String> {
    let mut best: Option<(&str, usize)> = None;
    for &opt in options {
        let dist = levenshtein(candidate, opt);
        if dist <= 3 {
            if let Some((_, best_dist)) = best {
                if dist < best_dist {
                    best = Some((opt, dist));
                }
            } else {
                best = Some((opt, dist));
            }
        }
    }
    best.map(|(opt, _)| opt.to_owned())
}

fn levenshtein(a: &str, b: &str) -> usize {
    let a_len = a.chars().count();
    let b_len = b.chars().count();
    if a_len == 0 {
        return b_len;
    }
    if b_len == 0 {
        return a_len;
    }

    let mut prev_row: Vec<usize> = (0..=b_len).collect();
    let mut curr_row = vec![0; b_len + 1];

    for (i, ca) in a.chars().enumerate() {
        curr_row[0] = i + 1;
        for (j, cb) in b.chars().enumerate() {
            let cost = usize::from(!ca.eq_ignore_ascii_case(&cb));
            curr_row[j + 1] = (curr_row[j] + 1)
                .min(prev_row[j + 1] + 1)
                .min(prev_row[j] + cost);
        }
        prev_row.copy_from_slice(&curr_row);
    }

    curr_row[b_len]
}
