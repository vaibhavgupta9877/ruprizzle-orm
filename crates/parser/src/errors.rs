//! Turning Pest's mechanical errors into ones a schema author can act on (P1-04).
//!
//! Pest reports what the *grammar* wanted: `expected field_type`. That names an
//! implementation detail the user never wrote and cannot look up. Every message
//! produced here instead names the thing the user was writing, shows what was
//! found there, and gives the shape of the correct line.
//!
//! The mapping is keyed on the set of expected rules rather than on a single
//! rule, because Pest reports alternatives and the *combination* is what
//! identifies the situation: `string | number | boolean | env_call` only ever
//! co-occur on the right-hand side of a configuration entry.

use pest::error::{ErrorVariant, InputLocation};
use ruprizzle_core::diagnostic::SchemaError;
use ruprizzle_core::span::Span;

use crate::grammar::Rule;

/// A human phrasing of the syntax error, ready to become a diagnostic.
struct Phrasing {
    message: String,
    context: String,
    advice: String,
}

/// Converts a Pest parse failure into a [`SchemaError::Syntax`].
pub(crate) fn from_pest(err: &pest::error::Error<Rule>, source: &str) -> SchemaError {
    let start = match err.location {
        InputLocation::Pos(p) => p,
        InputLocation::Span((s, _)) => s,
    };
    let found = found_token(source, start);
    let end = match err.location {
        InputLocation::Span((_, e)) => e.max(start + 1),
        InputLocation::Pos(_) if found.is_empty() => start + 1,
        InputLocation::Pos(_) => start + found.len(),
    };
    let span = Span::new(start.min(source.len()), end.min(source.len().max(1)));

    let phrasing = match &err.variant {
        ErrorVariant::ParsingError { positives, .. } => phrase(positives, &found),
        ErrorVariant::CustomError { message } => Phrasing {
            message: message.clone(),
            context: "here".to_owned(),
            advice: "check the syntax of this declaration".to_owned(),
        },
    };

    SchemaError::Syntax {
        message: phrasing.message,
        advice: Some(phrasing.advice),
        span: span.into(),
        context: phrasing.context,
    }
}

/// The token at `offset`, for quoting back in the message.
fn found_token(source: &str, offset: usize) -> String {
    let rest = source.get(offset..).unwrap_or("");
    let trimmed = rest.trim_start();
    if trimmed.is_empty() {
        return String::new();
    }
    // Skipping whitespace would move the span off the reported location, so only
    // read a token when the error points directly at one.
    if trimmed.len() != rest.len() {
        return String::new();
    }
    let token: String = rest
        .chars()
        .take_while(|c| !c.is_whitespace())
        .take(24)
        .collect();
    token
}

fn phrase(positives: &[Rule], found: &str) -> Phrasing {
    let found_desc = if found.is_empty() {
        "end of input".to_owned()
    } else {
        format!("`{found}`")
    };

    inside_a_declaration(positives, &found_desc)
        .or_else(|| at_top_level(positives, found, &found_desc))
        .or_else(|| inside_an_attribute(positives, &found_desc))
        .unwrap_or_else(|| Phrasing {
            message: format!("unexpected {found_desc}"),
            context: "this is not valid here".to_owned(),
            advice: "check for a missing `{`, `}`, or `)` earlier in the file".to_owned(),
        })
}

/// Failures inside a `model`, `enum`, `datasource`, or `generator` body.
fn inside_a_declaration(positives: &[Rule], found_desc: &str) -> Option<Phrasing> {
    let has = |r: Rule| positives.contains(&r);

    if has(Rule::field_type) {
        return Some(Phrasing {
            message: "expected a field type".to_owned(),
            context: format!("expected a type here, found {found_desc}"),
            advice: "fields are written `name Type @attrs`, e.g. `email String @unique`".to_owned(),
        });
    }

    if has(Rule::env_call) || has(Rule::boolean) && has(Rule::string) && !has(Rule::arg) {
        return Some(Phrasing {
            message: "expected a configuration value".to_owned(),
            context: format!("expected a value here, found {found_desc}"),
            advice: "configuration values are quoted strings, numbers, `true`/`false`, \
                     or `env(\"VAR\")` — e.g. `provider = \"postgres\"`"
                .to_owned(),
        });
    }

    if has(Rule::field) || has(Rule::block_attr) {
        return Some(Phrasing {
            message: "expected a field or the end of the model".to_owned(),
            context: format!("expected a field, `@@`-attribute, or `}}`, found {found_desc}"),
            advice: "each model member is either `name Type @attrs` or a block attribute \
                     such as `@@index([email])`"
                .to_owned(),
        });
    }

    if has(Rule::config_kv) {
        return Some(Phrasing {
            message: "expected a configuration entry".to_owned(),
            context: format!("expected `key = value` or `}}`, found {found_desc}"),
            advice: "blocks contain `key = value` entries, e.g. `provider = \"postgres\"`"
                .to_owned(),
        });
    }

    if has(Rule::enum_variant) {
        return Some(Phrasing {
            message: "expected an enum variant".to_owned(),
            context: format!("expected a variant name or `}}`, found {found_desc}"),
            advice: "enum variants are bare names, one per line, e.g. `ADMIN`".to_owned(),
        });
    }

    None
}

/// Failures between declarations.
fn at_top_level(positives: &[Rule], found: &str, found_desc: &str) -> Option<Phrasing> {
    let has = |r: Rule| positives.contains(&r);

    // `schema` and `EOI` appear when the failure is at the top level: either
    // nothing matched at all (`schema`) or a declaration ended and the next one
    // did not begin (`EOI` alongside the declaration alternatives).
    if !(has(Rule::schema)
        || has(Rule::EOI)
        || has(Rule::kw_model)
        || has(Rule::kw_enum)
        || has(Rule::kw_datasource)
        || has(Rule::model_def)
        || has(Rule::enum_def)
        || has(Rule::datasource))
    {
        return None;
    }

    let keywords = ["datasource", "generator", "enum", "model"];
    let advice = ruprizzle_core::suggest::closest(found, keywords.iter()).map_or_else(
        || {
            "a schema contains only top-level `datasource`, `generator`, `enum`, and `model` blocks"
                .to_owned()
        },
        |k| format!("did you mean `{k}`?"),
    );

    Some(Phrasing {
        message: "expected a declaration".to_owned(),
        context: format!(
            "expected `datasource`, `generator`, `enum`, or `model`, found {found_desc}"
        ),
        advice,
    })
}

/// Failures inside an `@attribute(...)`.
fn inside_an_attribute(positives: &[Rule], found_desc: &str) -> Option<Phrasing> {
    let has = |r: Rule| positives.contains(&r);

    if has(Rule::arg) || has(Rule::value) || has(Rule::named_arg) {
        return Some(Phrasing {
            message: "expected an attribute argument".to_owned(),
            context: format!("expected an argument or `)`, found {found_desc}"),
            advice: "arguments are positional (`@default(now())`) or named \
                     (`@relation(fields: [authorId], references: [id])`)"
                .to_owned(),
        });
    }

    if has(Rule::attr_path) {
        return Some(Phrasing {
            message: "expected an attribute name".to_owned(),
            context: format!("expected a name after `@`, found {found_desc}"),
            advice: "write the attribute name directly after `@`, e.g. `@id` or `@db.VarChar(200)`"
                .to_owned(),
        });
    }

    if has(Rule::ident) {
        return Some(Phrasing {
            message: "expected a name".to_owned(),
            context: format!("expected an identifier here, found {found_desc}"),
            advice: "names start with a letter or `_` and contain letters, digits, and `_`"
                .to_owned(),
        });
    }

    None
}

#[cfg(test)]
mod tests {
    use miette::Diagnostic;

    use crate::parse;

    /// The five mistakes from P1-04: each gets a tailored message, and none
    /// mentions a grammar rule name.
    #[test]
    fn common_mistakes_get_tailored_messages() {
        let cases: &[(&str, &str)] = &[
            // 1. a field with no type
            (
                "model User {\n  email @unique\n}\n",
                "expected a field type",
            ),
            // 2. an unquoted configuration value
            (
                "datasource db {\n  provider = postgres\n}\n",
                "expected a configuration value",
            ),
            // 3. stray text where a declaration should start
            ("modle User {\n}\n", "expected a declaration"),
            // 4. a missing closing brace
            (
                "model User {\n  id Uuid @id\n",
                "expected a field or the end of the model",
            ),
            // 5. `@` with nothing after it
            ("model User {\n  id Uuid @\n}\n", "expected"),
        ];

        for (src, expected) in cases {
            let err = parse("schema.ruprizzle", src).expect_err("fixture is malformed");
            let rendered = format!("{:?}", miette::Report::new(*err));
            assert!(
                rendered.contains(expected),
                "expected {expected:?} in:\n{rendered}"
            );
            assert!(
                !rendered.contains("field_type") && !rendered.contains("model_member"),
                "raw grammar rule leaked into:\n{rendered}"
            );
        }
    }

    #[test]
    fn syntax_errors_carry_help_and_a_span() {
        let err = parse("schema.ruprizzle", "model User {\n  email @unique\n}\n")
            .expect_err("fixture is malformed");
        let first = &err.errors[0];
        assert!(first.help().is_some(), "syntax errors must suggest a fix");
    }
}
