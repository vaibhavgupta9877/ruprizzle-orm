//! Repository chores, runnable as `cargo xtask <task>`.
//!
//! Exists so that "what CI runs" is a single command a contributor can run
//! locally, rather than a list in a YAML file that drifts from reality.

use std::process::{Command, ExitCode};

const TASKS: &[(&str, &str)] = &[
    ("ci", "everything CI runs, in CI order"),
    ("fmt", "check formatting"),
    ("lint", "clippy with warnings denied"),
    ("test", "unit and snapshot tests"),
    ("docs", "build documentation with warnings denied"),
];

fn main() -> ExitCode {
    let task = std::env::args().nth(1);
    match task.as_deref() {
        Some("ci") => run_all(&["fmt", "lint", "test", "docs"]),
        Some("fmt") => run_all(&["fmt"]),
        Some("lint") => run_all(&["lint"]),
        Some("test") => run_all(&["test"]),
        Some("docs") => run_all(&["docs"]),
        other => {
            if let Some(t) = other {
                eprintln!(
                    "unknown task `{t}`
"
                );
            }
            eprintln!(
                "usage: cargo xtask <task>

tasks:"
            );
            for (name, desc) in TASKS {
                eprintln!("  {name:<6} {desc}");
            }
            ExitCode::FAILURE
        }
    }
}

fn run_all(tasks: &[&str]) -> ExitCode {
    for task in tasks {
        let (program, args): (&str, Vec<&str>) = match *task {
            "fmt" => ("cargo", vec!["fmt", "--all", "--check"]),
            "lint" => (
                "cargo",
                vec![
                    "clippy",
                    "--workspace",
                    "--all-targets",
                    "--",
                    "-D",
                    "warnings",
                ],
            ),
            "test" => ("cargo", vec!["test", "--workspace"]),
            "docs" => ("cargo", vec!["doc", "--workspace", "--no-deps"]),
            other => unreachable!("unhandled task {other}"),
        };

        eprintln!("--- xtask: {task} ---");
        let mut cmd = Command::new(program);
        cmd.args(&args);
        if *task == "docs" {
            cmd.env("RUSTDOCFLAGS", "-D warnings");
        }

        match cmd.status() {
            Ok(status) if status.success() => {}
            Ok(status) => {
                eprintln!("xtask: `{task}` failed with {status}");
                return ExitCode::FAILURE;
            }
            Err(e) => {
                eprintln!("xtask: could not run `{task}`: {e}");
                return ExitCode::FAILURE;
            }
        }
    }
    ExitCode::SUCCESS
}
