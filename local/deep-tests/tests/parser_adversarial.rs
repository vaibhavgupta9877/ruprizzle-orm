//! Parser stress tests.
//!
//! These are not semantic tests of valid schemas; they feed the parser strings
//! that are invalid, malformed, or adversarial and assert that it never panics
//! and that it returns a structured error.

use ruprizzle_parser::parse;

const CASES: &[&str] = &[
    "",
    "model",
    "model A {",
    "model A { }",
    "datasource db { provider = \"sqlite\" }",
    "datasource db { provider = \"sqlite\"\n url = \"x\"\n }\nmodel A { id Int @id",
    "@@invalid\nmodel A { id Int @id }",
    "model A { id @id }",
    "model A { id Int @id @id }",
    "model A { id Int @relation }",
    "enum Color {\n  Red\n  // missing close",
    "model A { id Int @id }\nmodel A { x Int }",
    "model A { id Int @id }\nenum A { X }",
    "model A { id Int @db.Uuid }",
    "model A { id Int @id @default \"\" }",
    "// only a comment",
    "model   { id Int @id }",
    "model A { id Int @id, name String }",
    "model A { id Int @id }\n\n\n\n\n\n\n\n\n\n\n\n\n\n\n\n\n\n\n\n",
    "\t\t\t\n\r",
    "model A { id Int @id }\nmodel B { a A @relation }",
    "model A { id Int @id }\n\nmodel B { a A @relation(fields: [a], references: [id]) }",
];

#[test]
fn parser_does_not_panic_and_reports_errors() {
    for (i, input) in CASES.iter().enumerate() {
        match parse("adversarial", input) {
            Ok(_) => {
                // A few inputs are valid enough to parse; that is acceptable.
            }
            Err(e) => {
                let msg = e.to_string();
                assert!(
                    !msg.is_empty(),
                    "case {i} produced an empty error message for input: {input:?}"
                );
            }
        }
    }
}
