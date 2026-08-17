use std::process::{Command, ExitCode};

/// Runs the generated-client compile-time benchmark via
/// `local/cross-orm-bench/compile_time.py`.
///
/// The Python script creates synthetic 50/200-model `schema.ruprizzle`
/// files, generates a temporary `generated_client` crate under
/// `target/bench-compile/`, builds it, and records wall time and binary size
/// in `docs/BenchmarkResults.md`.
pub fn bench_compile() -> ExitCode {
    let repo_root = std::env::current_dir().unwrap_or_default();
    let script = repo_root.join("local/cross-orm-bench/compile_time.py");

    for python in ["python", "python3"] {
        let probe = Command::new(python).arg("--version").output();
        if probe.map(|o| o.status.success()).unwrap_or(false) {
            match Command::new(python)
                .arg(&script)
                .current_dir(&repo_root)
                .status()
            {
                Ok(status) if status.success() => return ExitCode::SUCCESS,
                Ok(status) => {
                    eprintln!("xtask: bench_compile failed with {status}");
                    return ExitCode::FAILURE;
                }
                Err(e) => {
                    eprintln!("xtask: could not run {python} {}: {e}", script.display());
                    return ExitCode::FAILURE;
                }
            }
        }
    }

    eprintln!("xtask: python not found; cannot run {}", script.display());
    ExitCode::FAILURE
}
