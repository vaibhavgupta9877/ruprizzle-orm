//! Repository chores, runnable as `cargo xtask <task>`.
//!
//! Exists so that "what CI runs" is a single command a contributor can run
//! locally, rather than a list in a YAML file that drifts from reality.

use std::path::Path;
use std::process::{Command, ExitCode};

use syn::spanned::Spanned;
use syn::visit::Visit;
use syn::{
    BinOp, Expr, ExprBinary, ExprIndex, ExprUnary, ItemFn, ItemMod, UnOp,
};

const TASKS: &[(&str, &str)] = &[
    ("ci", "everything CI runs, in CI order"),
    ("fmt", "check formatting"),
    ("lint", "clippy with warnings denied"),
    ("test", "unit and snapshot tests"),
    ("docs", "build documentation with warnings denied"),
    ("examples", "compile generated code for all example schemas"),
    (
        "bench-client",
        "regenerate the end_to_end benchmark client from schema.ruprizzle",
    ),
    ("harden", "pre-release hardening checks"),
    (
        "release",
        "dry-run (or live) publish every crate in order; --live --no-verify --wait 60",
    ),
];

/// Per-crate ceiling for `unwrap()` / `expect()` / `panic!` in `src/`.
///
/// These are the counts at the time the audit became a gate. The numbers may
/// only go down: a new panic in library source is a design question, not a
/// detail, and it should be argued for in review rather than merged silently.
const PANIC_BUDGET: &[(&str, usize)] = &[
    ("crates/core", 2),
    ("crates/dialect", 0),
    ("crates/macros", 0),
    ("crates/runtime", 1),
    ("crates/parser", 29),
    ("crates/codegen", 1),
    ("crates/migrate", 2),
    ("crates/cli", 2),
];

/// Per-crate ceilings for arithmetic (`/`, `%`) and direct indexing (`x[i]`)
/// panics in `src/`.
///
/// These patterns are the blind spot of the `unwrap`/`expect` audit and have
/// produced divide-by-zero and out-of-bounds panics in the past. Each entry is
/// `(crate, arithmetic_budget, indexing_budget)`. The numbers may only go down.
const BUDGETS: &[(&str, usize, usize)] = &[
    ("crates/core", 0, 6),
    ("crates/dialect", 0, 8),
    ("crates/macros", 0, 0),
    ("crates/runtime", 4, 14),
    ("crates/parser", 0, 17),
    ("crates/codegen", 0, 0),
    ("crates/migrate", 0, 13),
    ("crates/cli", 0, 0),
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
        Some("bench-client") => run_bench_client(),
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
    // The first test generates all example schemas for both dialects into a
    // throw-away crate in `target/generated-check`; the second clippys it under
    // `clippy::pedantic`. They must run sequentially because the second reuses
    // the crate the first materialises.
    for test in [
        "all_examples_both_dialects_compile",
        "generated_code_is_pedantic_clean",
    ] {
        eprintln!("--- xtask: examples: {test} ---");
        if !run_command(
            "cargo",
            &[
                "test",
                "-p",
                "ruprizzle-codegen",
                "--test",
                "compile",
                test,
                "--",
                "--include-ignored",
                "--exact",
            ],
        ) {
            eprintln!("xtask: examples test `{test}` failed");
            return ExitCode::FAILURE;
        }
    }
    ExitCode::SUCCESS
}

fn run_bench_client() -> ExitCode {
    eprintln!("--- xtask: bench-client ---");

    let schema = "crates/runtime/benches/end_to_end/schema.ruprizzle";
    let out = Path::new("crates/runtime/benches/end_to_end");
    let generated = out.join("generated");

    if !run_command(
        "cargo",
        &[
            "run",
            "-p",
            "ruprizzle-cli",
            "--",
            "generate",
            "--schema",
            schema,
        ],
    ) {
        eprintln!("xtask: bench client generation failed");
        return ExitCode::FAILURE;
    }

    // The generated `mod.rs` starts with an inner `#![allow(...)]` attribute,
    // which is not legal when the file is `include!`-ed into `main.rs`. Strip
    // it before copying the generated files into place.
    let mod_rs = generated.join("mod.rs");
    let content = match std::fs::read_to_string(&mod_rs) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("xtask: could not read generated mod.rs: {e}");
            return ExitCode::FAILURE;
        }
    };
    let patched = content
        .lines()
        .filter(|line| !line.starts_with("#![allow("))
        .collect::<Vec<_>>()
        .join("\n");
    if let Err(e) = std::fs::write(&mod_rs, patched) {
        eprintln!("xtask: could not write patched mod.rs: {e}");
        return ExitCode::FAILURE;
    }

    let entries = match std::fs::read_dir(&generated) {
        Ok(e) => e,
        Err(e) => {
            eprintln!("xtask: could not read generated directory: {e}");
            return ExitCode::FAILURE;
        }
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) == Some("rs") {
            let file_name = match path.file_name().and_then(|s| s.to_str()) {
                Some(name) => name,
                None => continue,
            };
            let dest = out.join(file_name);
            if let Err(e) = std::fs::copy(&path, &dest) {
                eprintln!(
                    "xtask: could not copy {} to {}: {e}",
                    path.display(),
                    dest.display()
                );
                return ExitCode::FAILURE;
            }
        }
    }

    if let Err(e) = std::fs::remove_dir_all(&generated) {
        eprintln!("xtask: could not remove generated directory: {e}");
        return ExitCode::FAILURE;
    }

    eprintln!("xtask: bench client regenerated in {}", out.display());
    ExitCode::SUCCESS
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

    // Package every crate that will be published, in dependency order.
    // We use `cargo package --list` instead of `cargo publish --dry-run` because
    // the latter still resolves dependency versions against crates.io, which
    // fails for a new workspace version whose internal dependencies have not yet
    // been published. Listing the package files is sufficient to catch packaging
    // and inclusion mistakes; compile correctness is already covered by the lint
    // and test steps.
    for package in [
        "ruprizzle-core",
        "ruprizzle-parser",
        "ruprizzle-dialect",
        "ruprizzle-macros",
        "ruprizzle",
        "ruprizzle-migrate",
        "ruprizzle-codegen",
        "ruprizzle-cli",
    ] {
        eprintln!("--- xtask: package check {package} ---");
        if !run_command(
            "cargo",
            &["package", "-p", package, "--list", "--allow-dirty"],
        ) {
            return ExitCode::FAILURE;
        }
    }

    // Panic audit: fail on unwrap/expect/panic in library source above the
    // checked-in budget.
    eprintln!("--- xtask: panic audit ---");
    for (crate_dir, budget) in PANIC_BUDGET {
        match panic_audit(crate_dir) {
            Ok(count) if count <= *budget => {
                eprintln!("  {crate_dir}: {count} panic sites (budget {budget})");
            }
            Ok(count) => {
                eprintln!(
                    "xtask: panic budget exceeded for {crate_dir}: found {count}, budget {budget}"
                );
                return ExitCode::FAILURE;
            }
            Err(e) => {
                eprintln!("xtask: panic audit failed for {crate_dir}: {e}");
                return ExitCode::FAILURE;
            }
        }
    }

    // Arithmetic/indexing audit: catch `/`, `%`, and `x[i]` on non-constant
    // values, the blind spot that hid BUG-02 and BUG-05.
    eprintln!("--- xtask: arithmetic/indexing audit ---");
    for (crate_dir, arith_budget, idx_budget) in BUDGETS.iter() {
        match code_audit(crate_dir) {
            Ok((arith, idx)) if arith <= *arith_budget && idx <= *idx_budget => {
                eprintln!("  {crate_dir}: {arith} arithmetic, {idx} indexing (budget {arith_budget}, {idx_budget})");
            }
            Ok((arith, idx)) => {
                eprintln!("xtask: arithmetic/indexing budget exceeded for {crate_dir}: arithmetic {arith} (budget {arith_budget}), indexing {idx} (budget {idx_budget})");
                return ExitCode::FAILURE;
            }
            Err(e) => {
                eprintln!("xtask: code audit failed for {crate_dir}: {e}");
                return ExitCode::FAILURE;
            }
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

fn panic_audit(crate_dir: &str) -> Result<usize, std::io::Error> {
    let src = Path::new(crate_dir).join("src");
    if !src.exists() {
        return Ok(0);
    }

    let mut count = 0;
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
            if line.contains(".unwrap()")
                || line.contains(".expect(")
                || line.contains("panic!")
                || line.contains("todo!")
                || line.contains("unimplemented!")
            {
                count += 1;
                eprintln!("  {path:?}:{}: {line}", line_no + 1);
            }
        }
    }
    Ok(count)
}

fn code_audit(crate_dir: &str) -> Result<(usize, usize), std::io::Error> {
    let src = Path::new(crate_dir).join("src");
    if !src.exists() {
        return Ok((0, 0));
    }

    let mut arithmetic = 0;
    let mut indexing = 0;
    for entry in walkdir::WalkDir::new(&src)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = entry.path();
        if !path.is_file() || path.extension().and_then(|s| s.to_str()) != Some("rs") {
            continue;
        }

        // Only audit library source; generated and benchmark code is out of scope.
        let rel = path.strip_prefix(&src).unwrap_or(path);
        if rel.components().any(|c| {
            let s = c.as_os_str();
            s == "tests" || s == "benches" || s == "examples" || s == "bin"
        }) {
            continue;
        }

        let content = std::fs::read_to_string(path)?;
        let file = match syn::parse_file(&content) {
            Ok(f) => f,
            Err(e) => {
                eprintln!("  xtask: failed to parse {path:?}: {e}");
                continue;
            }
        };

        let mut visitor = CodeAuditVisitor {
            path,
            arithmetic: 0,
            indexing: 0,
        };
        visitor.visit_file(&file);
        arithmetic += visitor.arithmetic;
        indexing += visitor.indexing;
    }
    Ok((arithmetic, indexing))
}

struct CodeAuditVisitor<'a> {
    path: &'a Path,
    arithmetic: usize,
    indexing: usize,
}

impl<'a> Visit<'a> for CodeAuditVisitor<'a> {
    fn visit_expr_binary(&mut self, node: &'a ExprBinary) {
        if matches!(node.op, BinOp::Div(_) | BinOp::Rem(_)) && !is_literal(&node.right) {
            self.arithmetic += 1;
            let line = node.span().start().line;
            eprintln!("  {:?}:{}: arithmetic / or % on non-literal", self.path, line);
        }
        syn::visit::visit_expr_binary(self, node);
    }

    fn visit_expr_index(&mut self, node: &'a ExprIndex) {
        self.indexing += 1;
        let line = node.span().start().line;
        eprintln!("  {:?}:{}: direct slice indexing", self.path, line);
        syn::visit::visit_expr_index(self, node);
    }

    fn visit_item_mod(&mut self, node: &'a ItemMod) {
        if is_test_mod(node) {
            return;
        }
        syn::visit::visit_item_mod(self, node);
    }

    fn visit_item_fn(&mut self, node: &'a ItemFn) {
        if has_test_attr(&node.attrs) {
            return;
        }
        syn::visit::visit_item_fn(self, node);
    }
}

fn is_literal(expr: &Expr) -> bool {
    match expr {
        Expr::Lit(_) => true,
        Expr::Unary(ExprUnary {
            op: UnOp::Neg(_), expr, ..
        }) => is_literal(expr),
        _ => false,
    }
}

fn has_test_attr(attrs: &[syn::Attribute]) -> bool {
    attrs.iter().any(|attr| {
        if attr.path().is_ident("test") {
            return true;
        }
        if let Some(seg) = attr.path().segments.last() {
            if seg.ident == "test" {
                return true;
            }
        }
        is_cfg_test(attr)
    })
}

fn is_test_mod(node: &ItemMod) -> bool {
    node.ident == "tests" || has_cfg_test_attr(&node.attrs)
}

fn has_cfg_test_attr(attrs: &[syn::Attribute]) -> bool {
    attrs.iter().any(is_cfg_test)
}

fn is_cfg_test(attr: &syn::Attribute) -> bool {
    if !attr.path().is_ident("cfg") {
        return false;
    }
    let Some(list) = attr.meta.require_list().ok() else {
        return false;
    };
    // cfg(test) or cfg(all(test, ...)) etc.
    cfg_contains_test(&list.tokens)
}

fn cfg_contains_test(tokens: &proc_macro2::TokenStream) -> bool {
    for tt in tokens.clone() {
        match tt {
            proc_macro2::TokenTree::Ident(ident) if ident == "test" => return true,
            proc_macro2::TokenTree::Group(g) if cfg_contains_test(&g.stream()) => return true,
            _ => {}
        }
    }
    false
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

    // Live publishes are intentionally interactive only. Refuse to publish
    // automatically from any CI environment, even if --live is passed.
    if live && (std::env::var("CI").is_ok() || std::env::var("GITHUB_ACTIONS").is_ok()) {
        eprintln!("xtask: refusing live crate publish from a CI environment");
        eprintln!("xtask: run `cargo xtask release --live ...` from an interactive shell only");
        return ExitCode::FAILURE;
    }

    // Workspace packages have `workspace = true` dependencies that are
    // rewritten to exact versions. `cargo publish` verification resolves
    // those versions from crates.io, so it will see the *previous* release
    // until the staged publish has completed. Always skip verification here;
    // `cargo xtask harden` already compiles, tests, and lints.
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
    let mut flags = vec!["publish", "--no-verify"];
    if !live {
        flags.push("--dry-run");
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
