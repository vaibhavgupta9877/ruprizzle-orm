//! Snapshot tests for code generation.

use std::fs;

use ruprizzle_codegen::generate_all;
use ruprizzle_parser::parse;

#[test]
fn blog_snapshot() {
    let manifest = env!("CARGO_MANIFEST_DIR");
    let src =
        fs::read_to_string(format!("{manifest}/../../examples/blog/schema.ruprizzle")).unwrap();
    let schema = parse("schema.ruprizzle", &src).unwrap();
    let files = generate_all(&schema);

    let mod_rs = files.get("mod.rs").unwrap();
    let enums_rs = files.get("enums.rs").unwrap();
    let user_rs = files.get("user.rs").unwrap();
    let post_rs = files.get("post.rs").unwrap();

    insta::assert_snapshot!(mod_rs);
    insta::assert_snapshot!(enums_rs);
    insta::assert_snapshot!(user_rs);
    insta::assert_snapshot!(post_rs);
}
