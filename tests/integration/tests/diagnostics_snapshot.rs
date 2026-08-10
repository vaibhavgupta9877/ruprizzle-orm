//! P0-03 acceptance: several mistakes are reported in one pass, each pointing at
//! the right source, each saying what to do.
//!
//! Rendered through `insta` because diagnostic output is the product surface a
//! user actually sees. A wording or layout regression should show up as a
//! reviewable diff (`cargo insta review`), not go unnoticed until someone
//! complains.
//!
//! `NarratableReportHandler` is used rather than the graphical one so the
//! snapshot carries no ANSI escapes and does not depend on terminal width.

use miette::{Diagnostic, NamedSource, NarratableReportHandler};
use ruprizzle_core::diagnostic::{Diagnostics, SchemaError, SchemaErrors};
use ruprizzle_core::span::Span;

/// A schema with three independent mistakes, at known byte offsets.
const BAD_SCHEMA: &str = r"model User {
  id    Uuid   @id
  email Strng
}

model Post {
  title String
}

enum Role {
}
";

fn render(d: &dyn Diagnostic) -> String {
    let mut out = String::new();
    NarratableReportHandler::new()
        .render_report(&mut out, d)
        .expect("rendering a diagnostic cannot fail");
    out
}

fn source() -> NamedSource<String> {
    NamedSource::new("schema.ruprizzle", BAD_SCHEMA.to_owned())
}

fn span_of(needle: &str) -> Span {
    let start = BAD_SCHEMA.find(needle).expect("fixture contains the token");
    Span::new(start, start + needle.len())
}

#[test]
fn reports_every_error_in_one_pass() {
    let mut d = Diagnostics::new();

    d.push(SchemaError::UnknownType {
        found: "Strng".into(),
        advice: Some("did you mean `String`?".into()),
        span: span_of("Strng").into(),
    });
    d.push(SchemaError::MissingPrimaryKey {
        model: "Post".into(),
        span: span_of("model Post").into(),
    });
    d.push(SchemaError::EmptyEnum {
        name: "Role".into(),
        span: span_of("enum Role").into(),
    });

    let bundle = d
        .into_result("schema.ruprizzle", BAD_SCHEMA)
        .expect_err("three fatal errors were recorded");

    assert_eq!(
        bundle.errors.len(),
        3,
        "validation must not bail on the first error"
    );

    insta::assert_snapshot!("three_errors_one_pass", render(bundle.as_ref()));
}

#[test]
fn every_error_offers_a_fix() {
    // The standard from ImplPlan01-P0-03: a diagnostic that only restates the
    // problem has failed. This asserts the property mechanically rather than
    // trusting review to catch a missing `help(...)`.
    let cases: Vec<SchemaError> = vec![
        SchemaError::MissingPrimaryKey {
            model: "Post".into(),
            span: span_of("model Post").into(),
        },
        SchemaError::EmptyEnum {
            name: "Role".into(),
            span: span_of("enum Role").into(),
        },
        SchemaError::ScalarListUnsupported {
            found: "String".into(),
            span: span_of("title").into(),
        },
        SchemaError::UnknownProvider {
            found: "mysql".into(),
            supported: "postgres, sqlite".into(),
            span: span_of("model User").into(),
        },
    ];

    for case in cases {
        assert!(
            case.help().is_some(),
            "`{}` renders no help text",
            case.code().map(|c| c.to_string()).unwrap_or_default()
        );
    }
}

#[test]
fn warnings_render_but_do_not_fail() {
    let mut d = Diagnostics::new();
    d.push(SchemaError::ReservedKeyword {
        field: "type".into(),
        span: span_of("title").into(),
    });
    d.push(SchemaError::DialectDegraded {
        construct: "Decimal".into(),
        provider: "sqlite".into(),
        advice: Some("store minor units in an `Int` instead".into()),
        span: span_of("Uuid").into(),
        consequence: "stored as TEXT, so ordering is lexicographic".into(),
    });

    assert!(!d.has_errors(), "warnings must not fail a build");
    assert_eq!(d.warnings().len(), 2);

    let bundle = SchemaErrors {
        src: source(),
        errors: d.take_warnings(),
    };
    insta::assert_snapshot!("warnings", render(&bundle));
}
