//! Diagnostic reporters for offline query checking.

use std::fmt::Write as _;

use crate::validate::QueryCheckError;

/// Output format for reporting check errors.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ReportFormat {
    /// Human-readable colored output.
    #[default]
    Pretty,
    /// Machine-readable JSON array.
    Json,
    /// GitHub Actions workflow command annotations (`::error file=...::`).
    Github,
}

impl std::str::FromStr for ReportFormat {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "pretty" => Ok(Self::Pretty),
            "json" => Ok(Self::Json),
            "github" | "gh" => Ok(Self::Github),
            other => Err(format!(
                "invalid format `{other}`; expected one of: pretty, json, github"
            )),
        }
    }
}

/// Render a slice of `QueryCheckError`s using the specified `ReportFormat`.
#[must_use]
pub fn format_report(errors: &[QueryCheckError], format: ReportFormat, file_hint: &str) -> String {
    match format {
        ReportFormat::Pretty => format_pretty(errors, file_hint),
        ReportFormat::Json => format_json(errors, file_hint),
        ReportFormat::Github => format_github(errors, file_hint),
    }
}

fn format_pretty(errors: &[QueryCheckError], file_hint: &str) -> String {
    if errors.is_empty() {
        return format!("✓ {file_hint} is valid (0 errors found)");
    }

    let mut out = String::new();
    let _ = writeln!(
        out,
        "Found {} query validation error{}:",
        errors.len(),
        if errors.len() == 1 { "" } else { "s" }
    );

    for (idx, err) in errors.iter().enumerate() {
        let _ = write!(out, "\n  {}. [{}] ", idx + 1, err.title());
        if let Some(loc) = err.location() {
            let _ = write!(out, "{}:{}: ", loc.file, loc.line);
        } else {
            let _ = write!(out, "{file_hint}: ");
        }
        let _ = writeln!(out, "{err}");
    }

    out
}

fn format_json(errors: &[QueryCheckError], file_hint: &str) -> String {
    #[derive(serde::Serialize)]
    struct JsonEntry<'a> {
        file: &'a str,
        line: Option<u32>,
        column: Option<u32>,
        title: &'static str,
        message: String,
    }

    let entries: Vec<JsonEntry<'_>> = errors
        .iter()
        .map(|err| {
            let loc = err.location();
            JsonEntry {
                file: loc.map_or(file_hint, |l| l.file.as_str()),
                line: loc.map(|l| l.line),
                column: loc.map(|l| l.column),
                title: err.title(),
                message: err.to_string(),
            }
        })
        .collect();

    serde_json::to_string_pretty(&entries).unwrap_or_else(|_| "[]".to_owned())
}

fn format_github(errors: &[QueryCheckError], file_hint: &str) -> String {
    let mut out = String::new();
    for err in errors {
        let file = err.location().map_or(file_hint, |l| l.file.as_str());
        let line = err.location().map_or(1, |l| l.line);
        let col = err.location().map_or(1, |l| l.column);
        let title = err.title();
        let message = err.to_string();

        let _ = writeln!(
            out,
            "::error file={file},line={line},col={col},title=Ruprizzle Check: {title}::{message}"
        );
    }
    out
}
