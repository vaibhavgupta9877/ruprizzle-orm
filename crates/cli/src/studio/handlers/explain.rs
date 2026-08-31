//! Query execution plan handler.
//!
//! The plan is whatever the database returns for the submitted statement. It used
//! to be a hardcoded three-node Postgres plan with invented costs that did not even
//! read the query — a fabricated diagnostic, which is worse than none.

use askama::Template;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{Html, IntoResponse, Response};
use serde::Deserialize;
use std::sync::Arc;

use ruprizzle_core::ir::Provider;

use super::{AppState, ModelNav};
use crate::studio::db;

#[derive(Debug, Clone)]
pub struct ExplainNode {
    pub depth: usize,
    pub operation: String,
    pub target: String,
    pub cost: String,
    pub rows: String,
    pub is_scan: bool,
}

#[derive(Template)]
#[template(path = "explain/view.html")]
pub struct ExplainViewTemplate<'a> {
    pub models: &'a [ModelNav],
    pub current_model: &'a str,
    pub provider: &'a str,
    pub allow_writes: bool,
    pub nodes: Vec<ExplainNode>,
    /// Set when no plan could be obtained. Suppresses the tree.
    pub error: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ExplainForm {
    pub sql: String,
}

/// Runs `EXPLAIN` for the submitted statement and renders the plan.
pub async fn render_explain_tree(
    State(state): State<Arc<AppState>>,
    axum::extract::Form(form): axum::extract::Form<ExplainForm>,
) -> Response {
    let sql = form.sql.trim().trim_end_matches(';').trim();

    let (nodes, error) = if sql.is_empty() {
        (Vec::new(), Some("No statement was submitted.".to_string()))
    } else if let Some(pool) = state.pool.as_ref() {
        let provider = pool.provider();
        match db::fetch_dynamic(pool, explain_sql(provider, sql), Vec::new()).await {
            Ok((_, rows)) => (parse_plan(provider, &rows), None),
            Err(e) => (Vec::new(), Some(format!("EXPLAIN failed: {e}"))),
        }
    } else {
        (
            Vec::new(),
            Some(
                "Studio has no database connection, so no plan could be obtained. \
                 Start studio with a --database-url (or DATABASE_URL)."
                    .to_string(),
            ),
        )
    };

    let tmpl = ExplainViewTemplate {
        models: &state.models,
        current_model: "",
        provider: state.schema.datasource.provider.as_str(),
        allow_writes: state.config.allow_writes,
        nodes,
        error,
    };

    match tmpl.render() {
        Ok(html) => Html(html).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Template error: {e}"),
        )
            .into_response(),
    }
}

/// Prefixes `sql` with the provider's plan statement.
///
/// `ANALYZE` is never used: it would execute the statement, which the user did not
/// ask for and which is not safe for a mutation.
#[must_use]
pub fn explain_sql(provider: Provider, sql: &str) -> String {
    match provider {
        Provider::Postgres | Provider::Mysql => format!("EXPLAIN {sql}"),
        Provider::Sqlite => format!("EXPLAIN QUERY PLAN {sql}"),
    }
}

/// Turns the driver's plan rows into tree nodes.
///
/// Nothing here invents a value: `cost` and `rows` are only filled in when the plan
/// text actually carries them, and are left empty otherwise.
#[must_use]
fn parse_plan(provider: Provider, rows: &[db::TextRow]) -> Vec<ExplainNode> {
    rows.iter()
        .filter_map(|row| {
            // Postgres returns one text column; SQLite's EXPLAIN QUERY PLAN returns
            // (id, parent, notused, detail); MySQL returns a wide tabular row.
            let line = match provider {
                Provider::Sqlite => row.last(),
                _ => row.first(),
            }?
            .as_deref()?;
            Some(parse_line(line))
        })
        .collect()
}

fn parse_line(line: &str) -> ExplainNode {
    let depth = line.len().saturating_sub(line.trim_start().len()) / 2;
    let text = line.trim_start().trim_start_matches("->").trim();

    let cost = extract_between(text, "cost=", ' ').unwrap_or_default();
    let rows = extract_between(text, "rows=", ' ').unwrap_or_default();

    // The operation is the text up to the first parenthesised annotation.
    let head = text.split(" (").next().unwrap_or(text).trim();
    let (operation, target) = match head.split_once(" on ") {
        Some((op, tgt)) => (op.trim().to_string(), tgt.trim().to_string()),
        None => (head.to_string(), String::new()),
    };

    // A full table scan: Postgres calls it `Seq Scan`, SQLite's query plan opens
    // the line with `SCAN` (an indexed lookup is `SEARCH ... USING INDEX`).
    let upper = operation.to_ascii_uppercase();
    let is_scan = upper.contains("SEQ SCAN") || upper.starts_with("SCAN");

    ExplainNode {
        depth,
        operation,
        target,
        cost,
        rows,
        is_scan,
    }
}

/// Extracts the text between `marker` and the next `end` character.
fn extract_between(text: &str, marker: &str, end: char) -> Option<String> {
    let start = text.find(marker)? + marker.len();
    let rest = text.get(start..)?;
    let stop = rest.find(end).unwrap_or(rest.len());
    Some(rest.get(..stop)?.trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plan_statement_never_analyzes() {
        assert_eq!(
            explain_sql(Provider::Postgres, "SELECT 1"),
            "EXPLAIN SELECT 1"
        );
        assert_eq!(
            explain_sql(Provider::Sqlite, "SELECT 1"),
            "EXPLAIN QUERY PLAN SELECT 1"
        );
        assert!(!explain_sql(Provider::Postgres, "DELETE FROM users").contains("ANALYZE"));
    }

    #[test]
    fn postgres_plan_lines_keep_their_real_numbers() {
        let node = parse_line("  ->  Seq Scan on posts  (cost=0.00..4.50 rows=10 width=8)");
        assert_eq!(node.operation, "Seq Scan");
        assert_eq!(node.target, "posts");
        assert_eq!(node.cost, "0.00..4.50");
        assert_eq!(node.rows, "10");
        assert!(node.is_scan);
    }

    #[test]
    fn a_line_without_costs_reports_no_costs_rather_than_invented_ones() {
        let node = parse_line("SEARCH users USING INDEX ix_email (email=?)");
        assert_eq!(
            node.cost, "",
            "no cost in the plan means no cost on the page"
        );
        assert_eq!(node.rows, "");
        assert!(!node.is_scan, "an indexed search is not a full scan");

        assert!(parse_line("SCAN users").is_scan);
    }
}
