//! P1-03 acceptance: one fixture per validation rule, each asserting the specific
//! diagnostic code and the rendered span.
//!
//! The table below is the rule table from
//! `ProjectPlan/ImplementationPlan/ImplPlan02SchemaDslParser.md`, executable. A
//! rule that stops firing fails here rather than silently letting a broken schema
//! through.

use miette::{Diagnostic, NarratableReportHandler};
use ruprizzle_parser::parse_with_warnings;

/// `(fixture, expected diagnostic code, is a warning)`
const RULES: &[(&str, &str, bool)] = &[
    (
        "v01_missing_primary_key",
        "ruprizzle::missing_primary_key",
        false,
    ),
    (
        "v01_multiple_primary_keys",
        "ruprizzle::multiple_primary_keys",
        false,
    ),
    ("v02_unknown_type", "ruprizzle::unknown_type", false),
    (
        "v03_duplicate_declaration",
        "ruprizzle::duplicate_declaration",
        false,
    ),
    ("v04_duplicate_field", "ruprizzle::duplicate_field", false),
    (
        "v05_unknown_relation_field",
        "ruprizzle::unknown_relation_field",
        false,
    ),
    (
        "v06_invalid_relation_target",
        "ruprizzle::invalid_relation_target",
        false,
    ),
    (
        "v07_relation_type_mismatch",
        "ruprizzle::relation_type_mismatch",
        false,
    ),
    (
        "v08_ambiguous_relation",
        "ruprizzle::relation::ambiguous",
        false,
    ),
    (
        "v08_missing_relation_owner",
        "ruprizzle::relation::no_owner",
        false,
    ),
    (
        "v08_missing_back_relation",
        "ruprizzle::relation::missing_back_reference",
        false,
    ),
    (
        "v08_through_on_non_list",
        "ruprizzle::relation::through_non_list",
        false,
    ),
    (
        "v09_default_type_mismatch",
        "ruprizzle::default_type_mismatch",
        false,
    ),
    (
        "v10_invalid_attribute_target",
        "ruprizzle::invalid_attribute_target",
        false,
    ),
    (
        "v11_unknown_index_field",
        "ruprizzle::unknown_index_field",
        false,
    ),
    (
        "v12_scalar_list_unsupported",
        "ruprizzle::scalar_list_unsupported",
        false,
    ),
    (
        "v13_relation_nullability_mismatch",
        "ruprizzle::relation_nullability_mismatch",
        false,
    ),
    ("v14_empty_enum", "ruprizzle::empty_enum", false),
    (
        "v14_duplicate_variant",
        "ruprizzle::duplicate_variant",
        false,
    ),
    ("v15_unknown_provider", "ruprizzle::unknown_provider", false),
    ("v16_name_collision", "ruprizzle::name_collision", false),
    ("v17_reserved_keyword", "ruprizzle::reserved_keyword", true),
];

fn read(fixture: &str) -> String {
    let path = format!(
        "{}/tests/invalid/{fixture}.ruprizzle",
        env!("CARGO_MANIFEST_DIR")
    );
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("cannot read {path}: {e}"))
}

fn render(d: &dyn Diagnostic) -> String {
    let mut out = String::new();
    NarratableReportHandler::new()
        .render_report(&mut out, d)
        .expect("rendering cannot fail");
    out
}

#[test]
fn every_rule_fires_on_its_fixture() {
    for (fixture, code, is_warning) in RULES {
        let source = read(fixture);
        let result = parse_with_warnings(fixture, &source);

        let (found, rendered) = if *is_warning {
            let (_, warnings) = result
                .map_err(|e| format!("{fixture} should only warn:\n{:?}", miette::Report::new(*e)))
                .expect("warnings must not fail the parse");
            let codes = codes(&warnings);
            (
                codes,
                warnings.iter().map(|w| render(w)).collect::<String>(),
            )
        } else {
            let bundle = result
                .err()
                .unwrap_or_else(|| panic!("{fixture} unexpectedly validated"));
            (codes(&bundle.errors), render(bundle.as_ref()))
        };

        assert!(
            found.iter().any(|c| c == code),
            "{fixture}: expected `{code}`, got {found:?}\n{rendered}"
        );
        insta::assert_snapshot!(*fixture, rendered);
    }
}

fn codes(errors: &[ruprizzle_core::SchemaError]) -> Vec<String> {
    errors
        .iter()
        .map(|e| e.code().map(|c| c.to_string()).unwrap_or_default())
        .collect()
}

#[test]
fn every_error_points_somewhere_and_says_what_to_do() {
    // The P0-03 standard, enforced against real parser output rather than
    // hand-built variants: a diagnostic that only restates the problem has failed.
    for (fixture, _, is_warning) in RULES {
        if *is_warning {
            continue;
        }
        let source = read(fixture);
        let bundle = parse_with_warnings(fixture, &source)
            .err()
            .unwrap_or_else(|| panic!("{fixture} unexpectedly validated"));

        for error in &bundle.errors {
            assert!(
                error.help().is_some(),
                "{fixture}: `{}` offers no fix",
                error
            );
            assert!(
                error.labels().is_some_and(|mut l| l.next().is_some()),
                "{fixture}: `{error}` points nowhere"
            );
        }
    }
}

#[test]
fn several_mistakes_are_reported_in_one_pass() {
    // The G1 requirement: three errors produce three diagnostics from one run,
    // not one error three recompiles apart.
    let source = r#"
datasource db {
  provider = "postgres"
  url      = env("DATABASE_URL")
}

model User {
  id    Uuid  @id
  email Strng
}

model Post {
  title String
}

enum Role {
}
"#;
    let bundle = ruprizzle_parser::parse("schema.ruprizzle", source)
        .expect_err("three independent mistakes");

    let found = codes(&bundle.errors);
    assert_eq!(bundle.errors.len(), 3, "got {found:?}");
    assert!(found.contains(&"ruprizzle::unknown_type".to_owned()));
    assert!(found.contains(&"ruprizzle::missing_primary_key".to_owned()));
    assert!(found.contains(&"ruprizzle::empty_enum".to_owned()));

    insta::assert_snapshot!("three_errors_one_pass", render(bundle.as_ref()));
}
