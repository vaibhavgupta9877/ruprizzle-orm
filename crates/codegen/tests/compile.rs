//! Compile-test for generated code.
//!
//! Parses the blog example, writes the generated files to a throw-away crate
//! in `target/generated-check`, and runs `cargo check`.

use std::fs;
use std::process::Command;

use ruprizzle_codegen::generate_all;
use ruprizzle_parser::parse;

#[test]
#[ignore = "runs cargo check; expensive"]
fn blog_generated_compiles() {
    let manifest = env!("CARGO_MANIFEST_DIR");
    let workspace = std::path::Path::new(manifest)
        .parent()
        .unwrap()
        .parent()
        .unwrap();
    let schema_src = fs::read_to_string(workspace.join("examples/blog/schema.ruprizzle")).unwrap();
    let schema = parse("schema.ruprizzle", &schema_src).unwrap();
    let files = generate_all(&schema);

    let out = workspace.join("target/generated-check");
    if out.exists() {
        let _ = fs::remove_dir_all(&out);
    }
    fs::create_dir_all(out.join("src/db")).unwrap();

    let cargo_toml = r#"[package]
name = "generated-check"
version = "0.0.0"
edition = "2024"
rust-version = "1.85"

[workspace]

[dependencies]
ruprizzle = { path = "../../crates/runtime" }
sqlx = { version = "0.8", default-features = false, features = ["runtime-tokio", "macros", "uuid", "chrono", "json", "rust_decimal", "any", "postgres", "sqlite"] }
"#
    .to_string();
    fs::write(out.join("Cargo.toml"), cargo_toml).unwrap();
    fs::write(out.join("src/lib.rs"), "pub mod db;").unwrap();

    for (path, content) in files {
        fs::write(out.join(format!("src/db/{path}")), content).unwrap();
    }

    let status = Command::new("cargo")
        .arg("check")
        .arg("--manifest-path")
        .arg(out.join("Cargo.toml"))
        .status()
        .expect("cargo should be on PATH");

    assert!(status.success(), "generated crate did not compile");
}
