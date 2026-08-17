//! Schema diagnostics.
//!
//! Two rules govern everything in this module, and they are why it exists in P0
//! rather than being retrofitted later:
//!
//! 1. **Every diagnostic points somewhere.** Each variant carries a `#[label]` on
//!    the span that caused it, so the reporter can underline the offending source.
//! 2. **Every diagnostic says what to do.** `help(...)` describes the fix, not
//!    merely the problem. A message that only restates the error has failed.
//!
//! Errors also *accumulate*: the validator collects into [`Diagnostics`] and
//! reports everything it found in one pass. Bailing on the first error makes
//! fixing a schema an exercise in repeated recompilation.

use miette::{Diagnostic, NamedSource, SourceSpan};
use thiserror::Error;

use crate::span::Span;

/// A single problem found in a schema.
///
/// Variants are named after the validation rules in
/// `ProjectPlan/ImplementationPlan/ImplPlan02SchemaDslParser.md`; the rule
/// identifier appears in each doc comment so the two stay traceable.
///
/// Fields are exempted from `missing_docs`: each one is already described by the
/// variant's `#[error]` message and `#[label]` text, which is where a reader
/// actually encounters it. Duplicating that into rustdoc would add noise without
/// adding information, and would drift from the message it restates.
#[derive(Debug, Error, Diagnostic)]
#[allow(missing_docs)]
pub enum SchemaError {
    /// Raw syntax error from the parser, rephrased for humans.
    #[error("{message}")]
    #[diagnostic(code(ruprizzle::parse::syntax))]
    Syntax {
        message: String,
        #[help]
        advice: Option<String>,
        #[label("{context}")]
        span: SourceSpan,
        context: String,
    },

    /// V02 — a field's type is not a scalar, enum, or model.
    #[error("unknown type `{found}`")]
    #[diagnostic(code(ruprizzle::unknown_type))]
    UnknownType {
        found: String,
        #[help]
        advice: Option<String>,
        #[label("not a known scalar, enum, or model")]
        span: SourceSpan,
    },

    /// V15 — `datasource.provider` names a dialect this build does not support.
    #[error("unknown provider `{found}`")]
    #[diagnostic(
        code(ruprizzle::unknown_provider),
        help("supported providers are: {supported}")
    )]
    UnknownProvider {
        found: String,
        supported: String,
        #[label("not a supported database provider")]
        span: SourceSpan,
    },

    /// V01 — a model declares no primary key.
    #[error("model `{model}` has no primary key")]
    #[diagnostic(
        code(ruprizzle::missing_primary_key),
        help("add `@id` to a field, or `@@id([a, b])` for a composite key")
    )]
    MissingPrimaryKey {
        model: String,
        #[label("this model needs a primary key")]
        span: SourceSpan,
    },

    /// V01 — a model declares more than one primary key.
    #[error("model `{model}` declares more than one primary key")]
    #[diagnostic(
        code(ruprizzle::multiple_primary_keys),
        help("use a single `@@id([a, b])` for a composite key instead of several `@id` fields")
    )]
    MultiplePrimaryKeys {
        model: String,
        #[label("second primary key declared here")]
        span: SourceSpan,
        #[label("first one is here")]
        first: SourceSpan,
    },

    /// V03 — two declarations share a name.
    #[error("`{name}` is declared more than once")]
    #[diagnostic(
        code(ruprizzle::duplicate_declaration),
        help("model and enum names share one namespace; rename one of them")
    )]
    DuplicateDecl {
        name: String,
        #[label("redeclared here")]
        span: SourceSpan,
        #[label("first declared here")]
        first: SourceSpan,
    },

    /// V04 — two fields in one model share a name.
    #[error("field `{field}` is declared twice on model `{model}`")]
    #[diagnostic(
        code(ruprizzle::duplicate_field),
        help(
            "remove one of them, or rename it and add `@map(\"{field}\")` if both must map to the same column name"
        )
    )]
    DuplicateField {
        model: String,
        field: String,
        #[label("redeclared here")]
        span: SourceSpan,
        #[label("first declared here")]
        first: SourceSpan,
    },

    /// V05 — `@relation(fields: [...])` names a field that does not exist.
    #[error("relation on `{model}.{field}` refers to unknown field `{missing}`")]
    #[diagnostic(
        code(ruprizzle::unknown_relation_field),
        help("`fields:` must list scalar fields of `{model}` that hold the foreign key")
    )]
    UnknownRelationField {
        model: String,
        field: String,
        missing: String,
        #[label("no such field on this model")]
        span: SourceSpan,
    },

    /// V06 — `@relation(references: [...])` names an invalid target.
    #[error("relation on `{model}.{field}` cannot reference `{target}.{missing}`")]
    #[diagnostic(
        code(ruprizzle::invalid_relation_target),
        help(
            "`references:` must name fields that are unique or form the primary key of `{target}`"
        )
    )]
    InvalidRelationTarget {
        model: String,
        field: String,
        target: String,
        missing: String,
        #[label("not a valid reference target")]
        span: SourceSpan,
    },

    /// V07 — foreign key type does not match the referenced column.
    #[error("foreign key `{model}.{field}` is `{found}` but references a `{expected}`")]
    #[diagnostic(
        code(ruprizzle::relation_type_mismatch),
        help("change `{model}.{field}` to `{expected}` so the join types line up")
    )]
    RelationTypeMismatch {
        model: String,
        field: String,
        found: String,
        expected: String,
        #[label("type does not match the referenced column")]
        span: SourceSpan,
    },

    /// V08 — two relations connect the same pair of models without names.
    #[error("two relations from `{model}` to `{target}` need explicit names")]
    #[diagnostic(
        code(ruprizzle::relation::ambiguous),
        help(
            "name each relation, e.g. `@relation(\"{model}{target}\", fields: [...], references: [...])`, \
              and give the matching name on the other side"
        )
    )]
    AmbiguousRelation {
        model: String,
        target: String,
        #[label("second relation to `{target}`")]
        span: SourceSpan,
        #[label("first relation to `{target}`")]
        first: SourceSpan,
    },

    /// V08 — a relation has no owning side.
    #[error("relation between `{model}` and `{target}` has no owning side")]
    #[diagnostic(
        code(ruprizzle::relation::no_owner),
        help(
            "exactly one side must declare `@relation(fields: [...], references: [...])` \
              to say which table holds the foreign key"
        )
    )]
    MissingRelationOwner {
        model: String,
        target: String,
        #[label("neither this field nor its counterpart declares `fields:`")]
        span: SourceSpan,
    },

    /// V08 — the other side of a relation was never declared.
    #[error("`{model}.{field}` points at `{target}`, which has no matching field")]
    #[diagnostic(
        code(ruprizzle::relation::missing_back_reference),
        help("add the other side to `{target}`, e.g. `{back_name} {model}[]`")
    )]
    MissingBackRelation {
        model: String,
        field: String,
        target: String,
        back_name: String,
        #[label("no counterpart on `{target}`")]
        span: SourceSpan,
    },

    /// V08 — `through:` is used on a non-list relation field.
    #[error("`through` can only be used on a list relation field")]
    #[diagnostic(code(ruprizzle::relation::through_non_list))]
    ThroughOnNonList {
        model: String,
        field: String,
        #[help]
        advice: Option<String>,
        #[label("this relation is not a list")]
        span: SourceSpan,
    },

    /// V08 — `through:` is combined with `fields:`.
    #[error("`through` cannot be combined with `fields:`")]
    #[diagnostic(code(ruprizzle::relation::through_with_fields))]
    ThroughWithFields {
        model: String,
        field: String,
        #[help]
        advice: Option<String>,
        #[label("remove `fields:` from this `through` relation")]
        span: SourceSpan,
    },

    /// V08 — the join model named in `through:` does not exist.
    #[error("join model `{through}` for `{model}.{field}` does not exist")]
    #[diagnostic(code(ruprizzle::relation::missing_through_model))]
    MissingThroughModel {
        model: String,
        field: String,
        through: String,
        #[help]
        advice: Option<String>,
        #[label("unknown join model")]
        span: SourceSpan,
    },

    /// V08 — the join model is not a valid many-to-many join.
    #[error(
        "join model `{through}` is not a valid many-to-many join between `{owner}` and `{target}`"
    )]
    #[diagnostic(code(ruprizzle::relation::invalid_join_model))]
    InvalidJoinModel {
        through: String,
        owner: String,
        target: String,
        #[help]
        advice: Option<String>,
        #[label("not a valid join model")]
        span: SourceSpan,
    },

    /// V09 — a `@default(...)` does not match the field's type.
    #[error("default value does not match type `{expected}`")]
    #[diagnostic(code(ruprizzle::default_type_mismatch))]
    DefaultTypeMismatch {
        expected: String,
        #[help]
        advice: Option<String>,
        #[label("this default is not a valid `{expected}`")]
        span: SourceSpan,
    },

    /// V10 — an attribute is applied to a field it cannot apply to.
    #[error("`@{attribute}` cannot be used on a `{found}` field")]
    #[diagnostic(code(ruprizzle::invalid_attribute_target))]
    InvalidAttributeTarget {
        attribute: String,
        found: String,
        #[help]
        advice: Option<String>,
        #[label("not valid here")]
        span: SourceSpan,
    },

    /// V11 — `@@index`/`@@unique` names a field that does not exist.
    #[error("`@@{attribute}` on `{model}` refers to unknown field `{missing}`")]
    #[diagnostic(code(ruprizzle::unknown_index_field))]
    UnknownIndexField {
        model: String,
        attribute: String,
        missing: String,
        #[help]
        advice: Option<String>,
        #[label("no such field on this model")]
        span: SourceSpan,
    },

    /// V13 — an optional relation backed by a non-nullable foreign key.
    #[error("optional relation `{model}.{field}` has a required foreign key")]
    #[diagnostic(
        code(ruprizzle::relation_nullability_mismatch),
        help("make `{model}.{fk}` optional (`{fk} {fk_type}?`) so the relation can be absent")
    )]
    RelationNullabilityMismatch {
        model: String,
        field: String,
        fk: String,
        fk_type: String,
        #[label("relation is optional but its foreign key is not")]
        span: SourceSpan,
    },

    /// V14 — an enum with no variants.
    #[error("enum `{name}` has no variants")]
    #[diagnostic(
        code(ruprizzle::empty_enum),
        help("an enum needs at least one variant to be usable as a column type")
    )]
    EmptyEnum {
        name: String,
        #[label("no variants declared")]
        span: SourceSpan,
    },

    /// V14 — duplicate enum variant.
    #[error("enum `{name}` declares variant `{variant}` twice")]
    #[diagnostic(
        code(ruprizzle::duplicate_variant),
        help("remove the repeat, or give one variant a distinct name and `@map(\"{variant}\")`")
    )]
    DuplicateVariant {
        name: String,
        variant: String,
        #[label("redeclared here")]
        span: SourceSpan,
        #[label("first declared here")]
        first: SourceSpan,
    },

    /// V16 — two declarations map to the same physical name.
    #[error("`{a}` and `{b}` both map to `{physical}`")]
    #[diagnostic(
        code(ruprizzle::name_collision),
        help(
            "use `@@map(\"...\")` or `@map(\"...\")` to give one of them a distinct database name"
        )
    )]
    NameCollision {
        a: String,
        b: String,
        physical: String,
        #[label("collides with an earlier declaration")]
        span: SourceSpan,
    },

    /// V18 — the chosen provider degrades or rejects this construct.
    #[error("`{construct}` is not fully supported on {provider}")]
    #[diagnostic(code(ruprizzle::dialect::degraded_type), severity(Warning))]
    DialectDegraded {
        construct: String,
        provider: String,
        #[help]
        advice: Option<String>,
        #[label("{consequence}")]
        span: SourceSpan,
        consequence: String,
    },

    /// V17 — a field name that needs escaping in generated Rust.
    #[error("field name `{field}` is a Rust keyword")]
    #[diagnostic(
        code(ruprizzle::reserved_keyword),
        severity(Warning),
        help(
            "generated code will refer to it as `r#{field}`; rename it or add \
              `@map(\"{field}\")` with a different field name to avoid the escape"
        )
    )]
    ReservedKeyword {
        field: String,
        #[label("escaped as `r#{field}` in generated code")]
        span: SourceSpan,
    },
}

impl SchemaError {
    /// Whether this diagnostic is advisory rather than fatal.
    #[must_use]
    pub fn is_warning(&self) -> bool {
        matches!(
            self.severity(),
            Some(miette::Severity::Warning | miette::Severity::Advice)
        )
    }
}

/// The complete set of problems found in one schema, with its source attached.
///
/// Related diagnostics inherit this type's `source_code`, so individual
/// [`SchemaError`] variants carry spans without each needing their own copy of
/// the schema text.
#[derive(Debug, Error, Diagnostic)]
#[error("{} problem(s) found in {}", .errors.len(), .src.name())]
#[diagnostic(code(ruprizzle::schema_invalid))]
pub struct SchemaErrors {
    /// The schema text, so every related diagnostic can render its span.
    #[source_code]
    pub src: NamedSource<String>,
    /// Every problem found, in the order the validator encountered them.
    #[related]
    pub errors: Vec<SchemaError>,
}

/// Accumulates diagnostics during parsing and validation.
///
/// Callers push freely and check once at the end; nothing here short-circuits.
#[derive(Debug, Default)]
pub struct Diagnostics {
    errors: Vec<SchemaError>,
    warnings: Vec<SchemaError>,
}

impl Diagnostics {
    /// An empty collector.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Records a diagnostic, routing it by its declared severity.
    pub fn push(&mut self, d: SchemaError) {
        if d.is_warning() {
            self.warnings.push(d);
        } else {
            self.errors.push(d);
        }
    }

    /// Whether any fatal diagnostic has been recorded.
    #[must_use]
    pub fn has_errors(&self) -> bool {
        !self.errors.is_empty()
    }

    /// The fatal diagnostics recorded so far.
    #[must_use]
    pub fn errors(&self) -> &[SchemaError] {
        &self.errors
    }

    /// The advisory diagnostics recorded so far.
    #[must_use]
    pub fn warnings(&self) -> &[SchemaError] {
        &self.warnings
    }

    /// Total count of both kinds.
    #[must_use]
    pub fn len(&self) -> usize {
        self.errors.len() + self.warnings.len()
    }

    /// Whether nothing at all was recorded.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Takes the warnings out, leaving errors in place.
    ///
    /// Callers print these even on the success path.
    pub fn take_warnings(&mut self) -> Vec<SchemaError> {
        std::mem::take(&mut self.warnings)
    }

    /// Converts accumulated errors into a reportable bundle.
    ///
    /// `Ok(())` when only warnings were recorded — warnings never fail a build.
    ///
    /// # Errors
    ///
    /// Returns every fatal diagnostic recorded, bundled with the schema source
    /// so spans can be rendered.
    pub fn into_result(self, file_name: &str, source: &str) -> Result<(), Box<SchemaErrors>> {
        if self.errors.is_empty() {
            return Ok(());
        }
        Err(Box::new(SchemaErrors {
            src: NamedSource::new(file_name, source.to_owned()).with_language("prisma"),
            errors: self.errors,
        }))
    }
}

/// Convenience conversion so IR spans can be pushed into diagnostics directly.
#[must_use]
pub fn label(span: Span) -> SourceSpan {
    span.into()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_source() -> &'static str {
        "model User {\n  id Uuid @id\n  email Strng\n}\n"
    }

    #[test]
    fn collects_every_error_rather_than_bailing() {
        let mut d = Diagnostics::new();
        d.push(SchemaError::UnknownType {
            found: "Strng".into(),
            advice: Some("did you mean `String`?".into()),
            span: Span::new(32, 37).into(),
        });
        d.push(SchemaError::MissingPrimaryKey {
            model: "Post".into(),
            span: Span::new(0, 5).into(),
        });
        d.push(SchemaError::EmptyEnum {
            name: "Role".into(),
            span: Span::new(1, 4).into(),
        });

        assert_eq!(d.errors().len(), 3);
        assert!(d.has_errors());

        let bundle = d
            .into_result("schema.ruprizzle", sample_source())
            .expect_err("three errors were pushed");
        assert_eq!(bundle.errors.len(), 3);
    }

    #[test]
    fn warnings_do_not_fail_the_build() {
        let mut d = Diagnostics::new();
        d.push(SchemaError::ReservedKeyword {
            field: "type".into(),
            span: Span::new(0, 4).into(),
        });

        assert!(!d.has_errors());
        assert_eq!(d.warnings().len(), 1);
        assert!(d.into_result("schema.ruprizzle", sample_source()).is_ok());
    }

    #[test]
    fn every_error_renders_with_a_span_and_help() {
        let mut d = Diagnostics::new();
        d.push(SchemaError::MissingPrimaryKey {
            model: "Post".into(),
            span: Span::new(0, 5).into(),
        });
        let bundle = d
            .into_result("schema.ruprizzle", sample_source())
            .unwrap_err();

        // The rendered report must mention the fix, not just the problem.
        let rendered = format!("{:?}", miette::Report::new(*bundle));
        assert!(rendered.contains("@@id"), "help text missing:\n{rendered}");
    }
}
