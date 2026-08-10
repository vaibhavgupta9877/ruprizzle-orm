//! Snapshot tests for code generation.
//!
//! Covers the full matrix the G3 gate asks for: all four example schemas, each
//! generated for both dialects. The dialect is taken from the schema's
//! `datasource` block, so the non-native provider is exercised by overriding
//! `provider` after parsing — that is exactly the switch a user makes when they
//! move a schema between engines, and the promise from ImplPlan03 is that the
//! Rust-facing API does not change when they do.

use std::fs;

use ruprizzle_codegen::generate_all;
use ruprizzle_core::ir::Provider;
use ruprizzle_parser::parse;

/// The four example schemas, which between them cover every shape codegen has
/// to handle (see ImplPlan02's example table).
const EXAMPLES: [&str; 4] = ["blog", "ecommerce", "saas", "social"];

fn workspace_root() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf()
}

/// Generates one example under one provider.
fn generate(example: &str, provider: Provider) -> std::collections::BTreeMap<String, String> {
    let path = workspace_root().join(format!("examples/{example}/schema.ruprizzle"));
    let src = fs::read_to_string(&path).unwrap_or_else(|e| panic!("reading {path:?}: {e}"));
    let mut schema =
        parse("schema.ruprizzle", &src).unwrap_or_else(|e| panic!("{example} should parse: {e:?}"));
    schema.datasource.provider = provider;
    generate_all(&schema)
}

#[test]
fn all_examples_both_dialects() {
    for example in EXAMPLES {
        for (label, provider) in [
            ("postgres", Provider::Postgres),
            ("sqlite", Provider::Sqlite),
        ] {
            let files = generate(example, provider);
            assert!(
                !files.is_empty(),
                "{example}/{label} generated no files at all"
            );

            for (name, content) in &files {
                let stem = name.trim_end_matches(".rs");
                insta::assert_snapshot!(format!("{example}__{label}__{stem}"), content);
            }
        }
    }
}

/// The headline cross-dialect promise: switching `provider` changes storage and
/// encode/decode shims, never the application-facing Rust API. Entity structs,
/// column tokens, and relation helpers must be byte-identical; only `enums.rs`
/// may differ, because Postgres uses a native `CREATE TYPE` while SQLite stores
/// enums as `TEXT`.
#[test]
fn rust_api_is_identical_across_dialects() {
    for example in EXAMPLES {
        let pg = generate(example, Provider::Postgres);
        let sqlite = generate(example, Provider::Sqlite);

        assert_eq!(
            pg.keys().collect::<Vec<_>>(),
            sqlite.keys().collect::<Vec<_>>(),
            "{example}: the two dialects emitted a different set of files"
        );

        for (name, pg_content) in &pg {
            if name == "enums.rs" || name == "_generated.rs" {
                continue;
            }
            assert_eq!(
                pg_content, &sqlite[name],
                "{example}/{name} differs between Postgres and SQLite; the \
                 application-facing API must not change when `provider` changes"
            );
        }
    }
}

/// Generating twice must produce byte-identical output (ImplPlan04 P3-05).
#[test]
fn generation_is_idempotent() {
    for example in EXAMPLES {
        for provider in [Provider::Postgres, Provider::Sqlite] {
            assert_eq!(
                generate(example, provider),
                generate(example, provider),
                "{example} generation is not deterministic"
            );
        }
    }
}
