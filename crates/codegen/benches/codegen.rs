//! Codegen benchmark.
//!
//! Parses a 50-model schema and runs `generate_all` for Postgres. The target is
//! under one second for this step; the benchmark prints the time so the number
//! can be recorded in release notes even if it is not yet a CI gate.

use criterion::{Criterion, criterion_group, criterion_main};
use ruprizzle_codegen::generate_all;
use ruprizzle_parser::parse;

fn fifty_model_schema() -> String {
    let mut s = String::from(
        r#"datasource db {
  provider = "postgres"
  url      = env("DATABASE_URL")
}

generator client {
  output      = "src/db"
  module_name = "db"
}

"#,
    );
    for i in 1..=50 {
        s.push_str(&format!(
            "model M{i} {{\n  id Uuid @id @default(uuid7())\n  name String?\n}}\n\n"
        ));
    }
    s
}

fn codegen(c: &mut Criterion) {
    let src = fifty_model_schema();

    c.bench_function("generate_50_models_postgres", |b| {
        b.iter(|| {
            let schema = parse("schema.ruprizzle", &src).unwrap();
            let files = generate_all(&schema);
            assert!(files.len() >= 50, "expected at least one file per model");
        })
    });
}

criterion_group! {
    name = benches;
    config = Criterion::default().sample_size(10);
    targets = codegen
}
criterion_main!(benches);
