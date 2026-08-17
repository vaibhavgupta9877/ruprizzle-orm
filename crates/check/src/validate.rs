//! Validate SQL queries against a `Schema` without a live database.

use std::collections::HashMap;

use ruprizzle_core::ir::Schema;

use crate::manifest::QueryManifest;

/// An error found while validating a query offline.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QueryCheckError {
    /// A table referenced by the query does not exist in the schema.
    UnknownTable {
        /// SQL that contains the reference.
        sql: String,
        /// Table name that could not be found.
        table: String,
    },
    /// A column referenced for a table does not exist.
    UnknownColumn {
        /// SQL that contains the reference.
        sql: String,
        /// Table that was expected to contain the column.
        table: String,
        /// Column that could not be found.
        column: String,
    },
}

impl std::fmt::Display for QueryCheckError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnknownTable { sql, table } => {
                write!(f, "unknown table `{table}` in `{sql}`")
            }
            Self::UnknownColumn { sql, table, column } => {
                write!(f, "unknown column `{column}` on table `{table}` in `{sql}`")
            }
        }
    }
}

impl std::error::Error for QueryCheckError {}

/// Validate every query in `manifest` against `schema`.
#[must_use]
pub fn validate_manifest(schema: &Schema, manifest: &QueryManifest) -> Vec<QueryCheckError> {
    manifest
        .queries
        .iter()
        .flat_map(|entry| validate_raw(schema, &entry.sql))
        .collect()
}

/// Coarse validation of a raw SQL string against `schema`.
///
/// This is not a full SQL parser; it tokenises the SQL and checks that any
/// identifier that matches a known model/table or column exists.
#[must_use]
pub fn validate_raw(schema: &Schema, sql: &str) -> Vec<QueryCheckError> {
    let mut errors = Vec::new();
    let tokens = tokenise(sql);

    let table_to_model: HashMap<&str, &ruprizzle_core::ir::Model> = schema
        .models
        .values()
        .map(|m| (m.table.as_str(), m))
        .collect();

    for (idx, token) in tokens.iter().enumerate() {
        if let Some(model) = table_to_model.get(token.as_str()) {
            // If the next token is `.column`, verify the column exists.
            if let Some(next) = tokens.get(idx + 1) {
                if next == "." {
                    if let Some(column) = tokens.get(idx + 2) {
                        if !model.fields.values().any(|f| f.column == *column) {
                            errors.push(QueryCheckError::UnknownColumn {
                                sql: sql.to_owned(),
                                table: token.to_owned(),
                                column: column.to_owned(),
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
            errors.push(QueryCheckError::UnknownTable {
                sql: sql.to_owned(),
                table: token.to_owned(),
            });
        }
    }

    errors
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
        "SELECT", "FROM", "WHERE", "AND", "OR", "NOT", "INSERT", "UPDATE", "DELETE", "JOIN",
        "INNER", "LEFT", "RIGHT", "FULL", "OUTER", "ON", "GROUP", "BY", "ORDER", "LIMIT", "OFFSET",
        "HAVING", "VALUES", "SET", "AS", "WITH", "UNION", "ALL", "DISTINCT", "IS", "NULL", "TRUE",
        "FALSE", "IN", "BETWEEN", "LIKE", "EXISTS", "CASE", "WHEN", "THEN", "ELSE", "END",
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
