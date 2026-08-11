//! `schema.ruprizzle` parser.
//!
//! The public surface is deliberately one function, [`parse`]. Everything else —
//! the Pest grammar, the loose AST, the five lowering passes, the validation rule
//! table — is an implementation detail behind it. That narrowness is what makes
//! replacing the parser a contained change rather than a schedule risk (see the
//! fallback note in `ImplPlan02SchemaDslParser.md`).
//!
//! ```
//! let schema = ruprizzle_parser::parse(
//!     "schema.ruprizzle",
//!     r#"
//!     datasource db {
//!       provider = "postgres"
//!       url      = env("DATABASE_URL")
//!     }
//!
//!     model User {
//!       id    Uuid   @id @default(uuid7())
//!       email String @unique
//!     }
//!     "#,
//! )
//! .expect("valid schema");
//!
//! assert_eq!(schema.model("User").unwrap().table, "users");
//! ```
//!
//! Errors accumulate: one call reports every problem it can find, each with a
//! span and a suggested fix.

#![forbid(unsafe_code)]
#![warn(missing_docs, clippy::pedantic)]

pub mod ast;
mod errors;
mod grammar;
mod lower;
pub mod naming;
mod validate;

use ruprizzle_core::diagnostic::{Diagnostics, SchemaErrors};
use ruprizzle_core::ir::Schema;

pub use ast::Ast;

/// Parses and validates a schema.
///
/// `file_name` is used only to label diagnostics; nothing is read from disk.
///
/// # Errors
///
/// Returns every problem found in one bundle — syntax errors, unresolved types,
/// and broken validation rules alike — with the source attached so each can
/// render its own span. Warnings (V17, and the dialect notes added in P2) never
/// fail: they are returned to the caller through [`parse_with_warnings`].
pub fn parse(file_name: &str, source: &str) -> Result<Schema, Box<SchemaErrors>> {
    parse_with_warnings(file_name, source).map(|(schema, _)| schema)
}

/// Like [`parse`], but also returns the advisory diagnostics.
///
/// The CLI prints these on the success path; tests assert on them.
///
/// # Errors
///
/// As [`parse`].
pub fn parse_with_warnings(
    file_name: &str,
    source: &str,
) -> Result<(Schema, Vec<ruprizzle_core::SchemaError>), Box<SchemaErrors>> {
    let mut diags = Diagnostics::new();

    let ast = match grammar::parse_ast(source) {
        Ok(ast) => ast,
        Err(err) => {
            // A syntax error means there is no tree to lower, so this is the one
            // place the parser cannot keep going.
            diags.push(errors::from_pest(&err, source));
            diags.into_result(file_name, source)?;
            unreachable!("a syntax error is always fatal");
        }
    };

    let schema = lower::lower(&ast, &mut diags);
    let warnings = diags.take_warnings();
    diags.into_result(file_name, source)?;
    Ok((schema, warnings))
}

/// Parses a schema without validating it, for tests and tooling that need the
/// syntax tree rather than the IR.
///
/// # Errors
///
/// Returns the syntax error, phrased for humans.
pub fn parse_ast(file_name: &str, source: &str) -> Result<Ast, Box<SchemaErrors>> {
    grammar::parse_ast(source).map_err(|err| {
        let mut diags = Diagnostics::new();
        diags.push(errors::from_pest(&err, source));
        diags
            .into_result(file_name, source)
            .expect_err("a syntax error is fatal")
    })
}
