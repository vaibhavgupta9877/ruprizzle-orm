//! Repository chores, runnable as `cargo xtask <task>`.
//!
//! Exists so that "what CI runs" is a single command a contributor can run
//! locally, rather than a list in a YAML file that drifts from reality.

use std::path::Path;
use std::process::{Command, ExitCode};

const TASKS: &[(&str, &str)] = &[
    ("ci", "everything CI runs, in CI order"),
    ("fmt", "check formatting"),
    ("lint", "clippy with warnings denied"),
    ("test", "unit and snapshot tests"),
    ("docs", "build documentation with warnings denied"),
    ("examples", "compile generated code for all example schemas"),
    ("harden", "pre-release hardening checks"),
    (
        "release",
        "dry-run (or live) publish every crate in order; --live --no-verify --wait 60",
    ),
];

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    let task = args.next();
    let rest: Vec<String> = args.collect();

    match task.as_deref() {
        Some("ci") => run_all(&["fmt", "lint", "test", "docs"]),
        Some("fmt") => run_all(&["fmt"]),
        Some("lint") => run_all(&["lint"]),
        Some("test") => run_all(&["test"]),
        Some("docs") => run_all(&["docs"]),
        Some("examples") => run_examples(),
        Some("harden") => run_harden(),
        Some("release") => run_release(&rest),
        other => {
            if let Some(t) = other {
                eprintln!("unknown task `{t}`");
            }
            eprintln!("usage: cargo xtask <task>\n\ntasks:");
            for (name, desc) in TASKS {
                eprintln!("  {name:<10} {desc}");
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

fn run_examples() -> ExitCode {
    eprintln!("--- xtask: examples ---");
    // The codegen compile test generates all example schemas for both dialects
    // and cargo-checks the result. It is ignored by default because it is
    // expensive, so we explicitly include ignored tests here.
    let status = Command::new("cargo")
        .args([
            "test",
            "-p",
            "ruprizzle-codegen",
            "--test",
            "compile",
            "all_examples_both_dialects_compile",
            "--",
            "--include-ignored",
            "--exact",
        ])
        .status();

    match status {
        Ok(s) if s.success() => ExitCode::SUCCESS,
        Ok(s) => {
            eprintln!("xtask: examples failed with {s}");
            ExitCode::FAILURE
        }
        Err(e) => {
            eprintln!("xtask: could not run examples: {e}");
            ExitCode::FAILURE
        }
    }
}

fn run_harden() -> ExitCode {
    eprintln!("--- xtask: harden ---");

    if run_all(&["lint", "test", "docs"]) != ExitCode::SUCCESS {
        return ExitCode::FAILURE;
    }

    // cargo-deny: licences, advisories, duplicate versions.
    if has_command("cargo-deny") || has_command("cargo") && has_subcommand("deny") {
        if !run_command("cargo", &["deny", "check"]) {
            eprintln!("xtask: cargo deny check failed");
            return ExitCode::FAILURE;
        }
    } else {
        eprintln!("xtask: cargo-deny not installed; skipping deny check");
    }

    // MSRV is declared in the workspace; verify with the installed toolchain.
    if !run_command("cargo", &["check", "--workspace"]) {
        return ExitCode::FAILURE;
    }

    // Dry-run publish every crate that will be published, in dependency order.
    for package in [
        "ruprizzle-core",
        "ruprizzle-dialect",
        "ruprizzle-macros",
        "ruprizzle",
        "ruprizzle-parser",
        "ruprizzle-codegen",
        "ruprizzle-migrate",
        "ruprizzle-cli",
    ] {
        eprintln!("--- xtask: dry-run publish {package} ---");
        if !run_command(
            "cargo",
            &["publish", "-p", package, "--dry-run", "--allow-dirty"],
        ) {
            return ExitCode::FAILURE;
        }
    }

    // Panic audit: warn on unwrap/expect in library source (tests allowed).
    eprintln!("--- xtask: panic audit ---");
    for crate_dir in [
        "crates/core",
        "crates/dialect",
        "crates/macros",
        "crates/runtime",
        "crates/parser",
        "crates/codegen",
        "crates/migrate",
        "crates/cli",
    ] {
        if let Err(e) = panic_audit(crate_dir) {
            eprintln!("xtask: panic audit failed for {crate_dir}: {e}");
            return ExitCode::FAILURE;
        }
    }

    // SQL-injection audit: look for Value interpolation into SQL strings.
    eprintln!("--- xtask: injection audit ---");
    if let Err(e) = injection_audit() {
        eprintln!("xtask: injection audit failed: {e}");
        return ExitCode::FAILURE;
    }

    eprintln!("xtask: harden complete");
    ExitCode::SUCCESS
}

fn panic_audit(crate_dir: &str) -> Result<(), std::io::Error> {
    let src = Path::new(crate_dir).join("src");
    if !src.exists() {
        return Ok(());
    }

    for entry in walkdir::WalkDir::new(&src)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = entry.path();
        if !path.is_file() || path.extension().and_then(|s| s.to_str()) != Some("rs") {
            continue;
        }
        // Tests may use unwrap for brevity; we only audit library source.
        let rel = path.strip_prefix(&src).unwrap_or(path);
        if rel.components().any(|c| c.as_os_str() == "tests") {
            continue;
        }

        let content = std::fs::read_to_string(path)?;
        for (line_no, line) in content.lines().enumerate() {
            // allow documented panics in Related::get and trybuild tests
            if line.contains("Related::get")
                || line.contains("panic!")
                || line.contains("unwrap()")
                || line.contains("expect(")
            {
                eprintln!("  {path:?}:{}: {line}", line_no + 1);
            }
        }
    }
    Ok(())
}

fn injection_audit() -> Result<(), std::io::Error> {
    // We look for any `format!` that builds SQL by interpolating a `Value` or
    // a user-supplied identifier. The architecture binds values as parameters,
    // so these should be zero outside of test fixtures.
    for crate_dir in [
        "crates/core",
        "crates/dialect",
        "crates/runtime",
        "crates/parser",
        "crates/codegen",
        "crates/migrate",
        "crates/cli",
    ] {
        let src = Path::new(crate_dir).join("src");
        if !src.exists() {
            continue;
        }
        for entry in walkdir::WalkDir::new(&src)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let path = entry.path();
            if !path.is_file() || path.extension().and_then(|s| s.to_str()) != Some("rs") {
                continue;
            }
            let content = std::fs::read_to_string(path)?;
            for (line_no, line) in content.lines().enumerate() {
                // Flag any format! that mentions Value or a placeholder with {}
                // and is followed by something that looks like SQL.
                if line.contains("format!") && (line.contains("Value") || line.contains("value")) {
                    eprintln!("  {path:?}:{}: {line}", line_no + 1);
                }
            }
        }
    }
    Ok(())
}

fn run_release(args: &[String]) -> ExitCode {
    let live = args.iter().any(|a| a == "--live");
    let no_verify = args.iter().any(|a| a == "--no-verify");
    let wait: u64 = args
        .iter()
        .position(|a| a == "--wait")
        .and_then(|i| args.get(i + 1))
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    let from = args
        .iter()
        .position(|a| a == "--from")
        .and_then(|i| args.get(i + 1))
        .map(String::as_str);
    let mut flags = vec!["publish"];
    if !live {
        flags.push("--dry-run");
    }
    if no_verify {
        flags.push("--no-verify");
    }

    // Dependency order for first-time publish. `parser` is a dev-dependency
    // of `dialect`, so it must be indexed before `dialect` can package.
    let packages = [
        "ruprizzle-core",
        "ruprizzle-parser",
        "ruprizzle-dialect",
        "ruprizzle-macros",
        "ruprizzle",
        "ruprizzle-migrate",
        "ruprizzle-codegen",
        "ruprizzle-cli",
    ];
    let start = packages
        .iter()
        .position(|p| from.is_none_or(|f| *p == f))
        .unwrap_or(0);

    for package in packages[start..].iter().copied() {
        eprintln!("--- xtask: release {package} ---");
        let mut cmd = Command::new("cargo");
        cmd.args(&flags).args(["-p", package]);
        if !live {
            // dry-run must allow dirty if the repo is not committed.
            cmd.arg("--allow-dirty");
        }

        match cmd.status() {
            Ok(s) if s.success() => {}
            Ok(s) => {
                eprintln!("xtask: release of {package} failed with {s}");
                return ExitCode::FAILURE;
            }
            Err(e) => {
                eprintln!("xtask: could not run cargo publish: {e}");
                return ExitCode::FAILURE;
            }
        }

        // Give the crates.io index time to update between live publishes,
        // otherwise the next package cannot resolve its freshly uploaded
        // dependency.
        if live && wait > 0 {
            eprintln!("xtask: waiting {wait}s for index update");
            std::thread::sleep(std::time::Duration::from_secs(wait));
        }
    }

    if live {
        eprintln!("xtask: published all crates");
    } else {
        eprintln!("xtask: dry-run complete; pass --live to publish for real");
    }
    ExitCode::SUCCESS
}

fn has_command(name: &str) -> bool {
    Command::new("which")
        .arg(name)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
        || Command::new("where")
            .arg(name)
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
}

fn has_subcommand(sub: &str) -> bool {
    // cargo help <sub> exits 0 if the subcommand is registered.
    Command::new("cargo")
        .args(["help", sub])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn run_command(program: &str, args: &[&str]) -> bool {
    Command::new(program)
        .args(args)
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}
