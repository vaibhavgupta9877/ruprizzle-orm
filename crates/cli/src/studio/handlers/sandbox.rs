//! SQL sandbox execution handlers.
//!
//! The sandbox runs exactly the statement the user typed, against the connected
//! database, and reports exactly what came back. It previously rendered a fixed
//! `result=1, status=OK` table without executing anything, which taught users that
//! queries they had "tested" here had run when they had not.

use askama::Template;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{Html, IntoResponse, Response};
use serde::Deserialize;
use std::fmt::Write as _;
use std::sync::Arc;
use std::time::Instant;

use super::{AppState, ModelNav};
use crate::studio::db;

/// Rows rendered per sandbox result. The limit is applied after execution, so it
/// bounds the page, not the query.
const MAX_RENDERED_ROWS: usize = 500;

#[derive(Template)]
#[template(path = "sandbox/view.html")]
pub struct SandboxViewTemplate<'a> {
    pub models: &'a [ModelNav],
    pub current_model: &'a str,
    pub provider: &'a str,
    pub allow_writes: bool,
}

#[derive(Debug, Deserialize)]
pub struct ExecuteForm {
    pub sql: String,
}

/// Renders the SQL playground page.
pub async fn render_sandbox_view(State(state): State<Arc<AppState>>) -> Response {
    let tmpl = SandboxViewTemplate {
        models: &state.models,
        current_model: "",
        provider: state.schema.datasource.provider.as_str(),
        allow_writes: state.config.allow_writes,
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

/// Whether a statement only reads.
///
/// Deliberately conservative: anything not recognised as a read is treated as a
/// mutation and needs `--allow-writes`.
#[must_use]
pub fn is_read_only(sql: &str) -> bool {
    let head = sql
        .split_whitespace()
        .next()
        .unwrap_or_default()
        .to_ascii_uppercase();
    matches!(
        head.as_str(),
        "SELECT" | "WITH" | "EXPLAIN" | "SHOW" | "PRAGMA" | "DESCRIBE" | "DESC"
    )
}

/// Executes a raw SQL statement and renders the real result.
pub async fn execute_sandbox_query(
    State(state): State<Arc<AppState>>,
    axum::extract::Form(form): axum::extract::Form<ExecuteForm>,
) -> Response {
    let sql = form.sql.trim().trim_end_matches(';').trim();
    if sql.is_empty() {
        return error_card("No statement was submitted.").into_response();
    }

    let Some(pool) = state.pool.as_ref() else {
        return error_card(
            "Studio has no database connection, so nothing was executed. \
             Start studio with a --database-url (or DATABASE_URL) to use the sandbox.",
        )
        .into_response();
    };

    let read_only = is_read_only(sql);
    if !read_only && !state.config.allow_writes {
        return error_card(
            "Mutating query rejected. Mutations are disabled in read-only mode; \
             start studio with <code>--allow-writes</code> to permit them. Nothing was executed.",
        )
        .into_response();
    }

    let start = Instant::now();
    if read_only {
        let result = db::fetch_dynamic(pool, sql.to_string(), Vec::new()).await;
        let micros = start.elapsed().as_micros();
        match result {
            Ok((columns, rows)) => result_card(&columns, &rows, micros).into_response(),
            Err(e) => error_card(&format!("Query failed: {}", html_escape(&e))).into_response(),
        }
    } else {
        let result = db::execute(pool, sql.to_string(), Vec::new()).await;
        let micros = start.elapsed().as_micros();
        match result {
            Ok(affected) => affected_card(affected, micros).into_response(),
            Err(e) => error_card(&format!("Statement failed: {}", html_escape(&e))).into_response(),
        }
    }
}

fn error_card(message: &str) -> Html<String> {
    Html(format!(
        r#"<div class="card" style="border-color:#f43f5e;">
            <div style="color:#f43f5e; font-weight:bold; margin-bottom:4px;">⛔ Not executed</div>
            <div style="color:#a1a1aa; font-size:13px;">{message}</div>
        </div>"#
    ))
}

fn affected_card(affected: u64, micros: u128) -> Html<String> {
    Html(format!(
        r#"<div class="card">
            <div style="display:flex; justify-content:space-between; align-items:center;">
                <span class="badge badge-safe">✓ {affected} row(s) affected</span>
                <span style="font-size:12px; color:#a1a1aa; font-family:monospace;">Execution time: {micros}µs</span>
            </div>
        </div>"#
    ))
}

fn result_card(columns: &[String], rows: &[db::TextRow], micros: u128) -> Html<String> {
    if columns.is_empty() {
        return Html(format!(
            r#"<div class="card">
                <div style="display:flex; justify-content:space-between; align-items:center;">
                    <span class="badge badge-safe">✓ 0 rows</span>
                    <span style="font-size:12px; color:#a1a1aa; font-family:monospace;">Execution time: {micros}µs</span>
                </div>
            </div>"#
        ));
    }

    let total = rows.len();
    let shown = rows.len().min(MAX_RENDERED_ROWS);

    let mut head = String::new();
    for column in columns {
        let _ = write!(head, "<th>{}</th>", html_escape(column));
    }

    let mut body = String::new();
    for row in rows.iter().take(shown) {
        body.push_str("<tr>");
        for cell in row {
            match cell {
                Some(value) => {
                    let _ = write!(body, "<td>{}</td>", html_escape(value));
                }
                None => body.push_str(r#"<td><span class="cell-null">null</span></td>"#),
            }
        }
        body.push_str("</tr>");
    }

    let truncated = if shown < total {
        format!(
            r#"<div style="margin-top:8px; font-size:12px; color:#a1a1aa;">Showing the first {shown} of {total} rows.</div>"#
        )
    } else {
        String::new()
    };

    Html(format!(
        r#"<div class="card">
            <div style="display:flex; justify-content:space-between; align-items:center; margin-bottom:12px;">
                <span class="badge badge-safe">✓ {total} row(s)</span>
                <span style="font-size:12px; color:#a1a1aa; font-family:monospace;">Execution time: {micros}µs</span>
            </div>
            <div style="overflow-x:auto;">
                <table class="data-table">
                    <thead><tr>{head}</tr></thead>
                    <tbody>{body}</tbody>
                </table>
            </div>
            {truncated}
        </div>"#
    ))
}

/// Escapes text that came from the database or from the user before it is written
/// into the hand-built result HTML.
fn html_escape(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    for ch in raw.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(ch),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_are_recognised_and_everything_else_is_not() {
        assert!(is_read_only("select 1"));
        assert!(is_read_only("  WITH t AS (SELECT 1) SELECT * FROM t"));
        assert!(is_read_only("EXPLAIN SELECT 1"));
        assert!(is_read_only("PRAGMA table_info('users')"));

        assert!(!is_read_only("INSERT INTO users VALUES (1)"));
        assert!(!is_read_only("update users set a = 1"));
        assert!(!is_read_only("DROP TABLE users"));
        assert!(!is_read_only("TRUNCATE users"));
        assert!(!is_read_only("VACUUM"));
        assert!(!is_read_only(""));
    }

    #[test]
    fn database_text_is_escaped_before_it_reaches_the_page() {
        assert_eq!(
            html_escape("<script>alert('x')</script>"),
            "&lt;script&gt;alert(&#39;x&#39;)&lt;/script&gt;"
        );
    }
}
