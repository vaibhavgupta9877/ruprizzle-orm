//! Integration test for `ruprizzle check`.

use std::io::Write;
use std::process::Command;

const SCHEMA: &str = r#"
datasource db {
  provider = "sqlite"
  url      = "file:app.db"
}

model User {
  id    Int    @id
  email String @unique
}
"#;

fn write(dir: &std::path::Path, name: &str, contents: &str) -> std::path::PathBuf {
    let path = dir.join(name);
    let mut file = std::fs::File::create(&path).unwrap();
    file.write_all(contents.as_bytes()).unwrap();
    path
}

#[test]
fn check_passes_for_valid_queries() {
    let dir = std::env::temp_dir().join(format!("ruprizzle_check_ok_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();

    let schema_path = write(&dir, "schema.ruprizzle", SCHEMA);
    let manifest =
        r#"{"schema_hash":"","queries":[{"sql":"SELECT * FROM users","dialect":"sqlite"}]}"#;
    let manifest_path = write(&dir, "manifest.json", manifest);

    let bin = env!("CARGO_BIN_EXE_ruprizzle");
    let status = Command::new(bin)
        .arg("check")
        .arg("--schema")
        .arg(&schema_path)
        .arg("--manifest")
        .arg(&manifest_path)
        .status()
        .unwrap();

    let _ = std::fs::remove_dir_all(&dir);
    assert!(status.success());
}

#[test]
fn check_fails_for_unknown_table() {
    let dir = std::env::temp_dir().join(format!("ruprizzle_check_err_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();

    let schema_path = write(&dir, "schema.ruprizzle", SCHEMA);
    let manifest =
        r#"{"schema_hash":"","queries":[{"sql":"SELECT * FROM not_a_table","dialect":"sqlite"}]}"#;
    let manifest_path = write(&dir, "manifest.json", manifest);

    let bin = env!("CARGO_BIN_EXE_ruprizzle");
    let output = Command::new(bin)
        .arg("check")
        .arg("--schema")
        .arg(&schema_path)
        .arg("--manifest")
        .arg(&manifest_path)
        .output()
        .unwrap();

    let _ = std::fs::remove_dir_all(&dir);
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("unknown table `not_a_table`"),
        "stderr: {stderr}"
    );
}
