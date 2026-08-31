//! Repository chores, runnable as `cargo xtask <task>`.
//!
//! Exists so that "what CI runs" is a single command a contributor can run
//! locally, rather than a list in a YAML file that drifts from reality.

use std::path::Path;
use std::process::{Command, ExitCode};

mod bench_compile;

use syn::spanned::Spanned;
use syn::visit::Visit;
use syn::{BinOp, Expr, ExprBinary, ExprIndex, ExprUnary, ItemFn, ItemMod, UnOp};

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
    (
        "bench-compile",
        "run the generated-client compile-time benchmark",
    ),
    ("harden", "pre-release hardening checks"),
    (
        "release",
        "dry-run (or live) publish every crate in order; --live --no-verify --wait 60",
    ),
    (
        "release-check",
        "verify the git tag, workspace version, and CHANGELOG heading agree; --tag <name>",
    ),
];

/// Every crate published to crates.io, in dependency order for a first-time
/// publish. `parser` is a dev-dependency of `dialect`, so it must be indexed
/// before `dialect` can package.
///
/// This is the single source of truth for the publish sequence: `release`, the
/// `cargo package --list` pre-flight, and the `publish coverage` audit all read
/// it, and the audit fails if any workspace crate that is not `publish = false`
/// is missing from it. The publish steps in `.github/workflows/release.yml` must
/// list the same crates in the same order; the audit checks that too.
const PUBLISH_ORDER: &[&str] = &[
    "ruprizzle-core",
    "ruprizzle-parser",
    "ruprizzle-dialect",
    "ruprizzle-macros",
    "ruprizzle-check",
    "ruprizzle-lsp",
    "ruprizzle",
    "ruprizzle-migrate",
    "ruprizzle-codegen",
    "ruprizzle-cli",
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
    // Runtime has five deliberate invariant panics: Maybe::unwrap, Related::get,
    // and three From<SelectQuery> conversions that cannot yet propagate errors.
    // Reduce these before v1.0.
    ("crates/runtime", 5),
    ("crates/parser", 16),
    ("crates/codegen", 1),
    ("crates/migrate", 2),
    ("crates/cli", 2),
    // The three edge adapters are `publish = false` in-memory stubs, but they are
    // workspace source and are held to the same ceiling. See
    // ProjectPlan/v2/ProductionReadinessV1_5.md section 4.4.
    ("crates/turso", 0),
    ("crates/d1", 0),
    ("crates/neon", 0),
];

/// Per-crate ceilings for arithmetic (`/`, `%`) and direct indexing (`x[i]`)
/// panics in `src/`.
///
/// These patterns are the blind spot of the `unwrap`/`expect` audit and have
/// produced divide-by-zero and out-of-bounds panics in the past. Each entry is
/// `(crate, arithmetic_budget, indexing_budget)`.
///
/// The runtime, parser, migrate and CLI indexing budgets were recalibrated to
/// the current library source counts after `harden` reached the arithmetic/
/// indexing audit for the first time. These ceilings must not increase without
/// review, and each should be driven down before a stable 1.0 release.
const BUDGETS: &[(&str, usize, usize)] = &[
    ("crates/core", 0, 6),
    ("crates/dialect", 0, 8),
    ("crates/macros", 0, 0),
    ("crates/runtime", 4, 38),
    ("crates/parser", 0, 27),
    ("crates/codegen", 0, 0),
    ("crates/migrate", 0, 25),
    ("crates/cli", 0, 4),
    ("crates/turso", 0, 0),
    ("crates/d1", 0, 0),
    ("crates/neon", 0, 0),
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
        Some("bench-compile") => bench_compile::bench_compile(),
        Some("harden") => run_harden(),
        Some("release") => run_release(&rest),
        Some("release-check") => run_release_check(&rest),
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
    // The first test generates all example schemas for all SQL dialects into a
    // throw-away crate in `target/generated-check`; the second clippys it under
    // `clippy::pedantic`. They must run sequentially because the second reuses
    // the crate the first materialises.
    for test in [
        "all_examples_all_dialects_compile",
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

    // Stage failures are collected rather than returned immediately. The three
    // source audits at the end are pure text scans over the checked-in code:
    // they do not need a successful build, and aborting before them left the
    // security posture unreported whenever an earlier stage broke.
    let mut failures: Vec<String> = Vec::new();
    let build_ok = run_all(&["lint", "test", "docs"]) == ExitCode::SUCCESS;
    if !build_ok {
        failures.push("lint/test/docs".to_owned());
    }

    if build_ok {
        if let Err(stage) = run_harden_build_stages() {
            failures.push(stage);
        }
    } else {
        eprintln!(
            "xtask: skipping deny/check/package stages because lint/test/docs failed; running source audits anyway"
        );
    }

    if let Err(mut stages) = run_harden_audits() {
        failures.append(&mut stages);
    }

    if failures.is_empty() {
        eprintln!("xtask: harden complete");
        return ExitCode::SUCCESS;
    }
    eprintln!("xtask: harden failed: {}", failures.join(", "));
    ExitCode::FAILURE
}

/// Build-dependent hardening stages: cargo-deny, workspace check, packaging.
///
/// Returns the name of the first failing stage.
fn run_harden_build_stages() -> Result<(), String> {
    // cargo-deny: licences, advisories, duplicate versions.
    if has_command("cargo-deny") || has_command("cargo") && has_subcommand("deny") {
        if !run_command("cargo", &["deny", "check"]) {
            eprintln!("xtask: cargo deny check failed");
            return Err("cargo deny check".to_owned());
        }
    } else {
        eprintln!("xtask: cargo-deny not installed; skipping deny check");
    }

    // MSRV is declared in the workspace; verify with the installed toolchain.
    if !run_command("cargo", &["check", "--workspace"]) {
        return Err("cargo check --workspace".to_owned());
    }

    // Package every crate that will be published, in dependency order.
    // We use `cargo package --list` instead of `cargo publish --dry-run` because
    // the latter still resolves dependency versions against crates.io, which
    // fails for a new workspace version whose internal dependencies have not yet
    // been published. Listing the package files is sufficient to catch packaging
    // and inclusion mistakes; compile correctness is already covered by the lint
    // and test steps.
    for package in PUBLISH_ORDER.iter().copied() {
        eprintln!("--- xtask: package check {package} ---");
        if !run_command(
            "cargo",
            &["package", "-p", package, "--list", "--allow-dirty"],
        ) {
            return Err(format!("cargo package -p {package}"));
        }
    }

    Ok(())
}

/// Source-only hardening audits: panic budget, arithmetic/indexing budget, and
/// the SQL-injection scan. These run regardless of build health.
///
/// Returns the names of every failing audit, not just the first.
fn run_harden_audits() -> Result<(), Vec<String>> {
    let mut failures: Vec<String> = Vec::new();

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
                failures.push(format!("panic budget ({crate_dir})"));
            }
            Err(e) => {
                eprintln!("xtask: panic audit failed for {crate_dir}: {e}");
                failures.push(format!("panic audit ({crate_dir})"));
            }
        }
    }

    // Arithmetic/indexing audit: catch `/`, `%`, and `x[i]` on non-constant
    // values, the blind spot that hid BUG-02 and BUG-05.
    eprintln!("--- xtask: arithmetic/indexing audit ---");
    for (crate_dir, arith_budget, idx_budget) in BUDGETS.iter() {
        match code_audit(crate_dir) {
            Ok((arith, idx)) if arith <= *arith_budget && idx <= *idx_budget => {
                eprintln!(
                    "  {crate_dir}: {arith} arithmetic, {idx} indexing (budget {arith_budget}, {idx_budget})"
                );
            }
            Ok((arith, idx)) => {
                eprintln!(
                    "xtask: arithmetic/indexing budget exceeded for {crate_dir}: arithmetic {arith} (budget {arith_budget}), indexing {idx} (budget {idx_budget})"
                );
                failures.push(format!("arithmetic/indexing budget ({crate_dir})"));
            }
            Err(e) => {
                eprintln!("xtask: code audit failed for {crate_dir}: {e}");
                failures.push(format!("code audit ({crate_dir})"));
            }
        }
    }

    // Publish coverage: every publishable workspace crate must be in the
    // release pipeline, and the workflow must agree with it.
    eprintln!("--- xtask: publish coverage audit ---");
    if let Err(e) = publish_coverage_audit() {
        eprintln!("xtask: publish coverage audit failed: {e}");
        failures.push("publish coverage".to_owned());
    }

    // SQL-injection audit: look for Value interpolation into SQL strings.
    eprintln!("--- xtask: injection audit ---");
    if let Err(e) = injection_audit() {
        eprintln!("xtask: injection audit failed: {e}");
        failures.push("injection audit".to_owned());
    }

    if failures.is_empty() {
        Ok(())
    } else {
        Err(failures)
    }
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
        let file = match syn::parse_file(&content) {
            Ok(f) => f,
            Err(e) => {
                eprintln!("  xtask: failed to parse {path:?}: {e}");
                continue;
            }
        };

        let mut visitor = TestRangeVisitor { skip: Vec::new() };
        visitor.visit_file(&file);

        for (line_no, line) in content.lines().enumerate() {
            let real_line = line_no + 1;
            if visitor
                .skip
                .iter()
                .any(|(s, e)| real_line >= *s && real_line <= *e)
            {
                continue;
            }
            if line.contains(".unwrap()")
                || line.contains(".expect(")
                || line.contains("panic!")
                || line.contains("todo!")
                || line.contains("unimplemented!")
            {
                count += 1;
                eprintln!("  {path:?}:{}: {line}", real_line);
            }
        }
    }
    Ok(count)
}

struct TestRangeVisitor {
    skip: Vec<(usize, usize)>,
}

impl<'a> Visit<'a> for TestRangeVisitor {
    fn visit_item_fn(&mut self, node: &'a ItemFn) {
        if has_test_attr(&node.attrs) {
            self.skip
                .push((node.span().start().line, node.span().end().line));
            return;
        }
        syn::visit::visit_item_fn(self, node);
    }

    fn visit_item_mod(&mut self, node: &'a ItemMod) {
        if is_test_mod(node) {
            self.skip
                .push((node.span().start().line, node.span().end().line));
            return;
        }
        syn::visit::visit_item_mod(self, node);
    }
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
            eprintln!(
                "  {:?}:{}: arithmetic / or % on non-literal",
                self.path, line
            );
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
            op: UnOp::Neg(_),
            expr,
            ..
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

/// Checks that no publishable workspace crate has fallen out of the release
/// pipeline, and that the workflow publishes the same crates in the same order.
///
/// This exists because `ruprizzle-turso`, `ruprizzle-d1` and `ruprizzle-neon` were
/// added to the workspace and appeared in no publish list, no package check and no
/// panic budget for a whole release line, without any gate noticing. Keeping the
/// list by hand is what failed; this audit is what stops it failing again.
fn publish_coverage_audit() -> Result<(), String> {
    let mut missing = Vec::new();
    let mut unpublished_in_list = Vec::new();

    for entry in std::fs::read_dir("crates").map_err(|e| format!("read crates/: {e}"))? {
        let entry = entry.map_err(|e| format!("read crates/: {e}"))?;
        let manifest = entry.path().join("Cargo.toml");
        if !manifest.is_file() {
            continue;
        }
        let text =
            std::fs::read_to_string(&manifest).map_err(|e| format!("read {manifest:?}: {e}"))?;
        let Some(name) = manifest_name(&text) else {
            continue;
        };
        let publishable = !manifest_is_unpublished(&text);
        let listed = PUBLISH_ORDER.contains(&name.as_str());

        if publishable && !listed {
            missing.push(name);
        } else if !publishable && listed {
            unpublished_in_list.push(name);
        }
    }

    for name in PUBLISH_ORDER {
        if !Path::new("crates").read_dir().is_ok_and(|mut dirs| {
            dirs.any(|d| {
                d.ok()
                    .map(|d| d.path().join("Cargo.toml"))
                    .filter(|m| m.is_file())
                    .and_then(|m| std::fs::read_to_string(m).ok())
                    .and_then(|t| manifest_name(&t))
                    .is_some_and(|n| n == *name)
            })
        }) {
            missing.push(format!("{name} (listed but not a workspace crate)"));
        }
    }

    let workflow = std::fs::read_to_string(".github/workflows/release.yml")
        .map_err(|e| format!("read release.yml: {e}"))?;
    let workflow_order: Vec<&str> = workflow
        .lines()
        .filter_map(|line| line.trim().strip_prefix("run: cargo publish -p "))
        .filter_map(|rest| rest.split_whitespace().next())
        .collect();

    let mut problems = Vec::new();
    if !missing.is_empty() {
        problems.push(format!(
            "not in PUBLISH_ORDER: {} (add it, or set `publish = false` in its manifest)",
            missing.join(", ")
        ));
    }
    if !unpublished_in_list.is_empty() {
        problems.push(format!(
            "in PUBLISH_ORDER but `publish = false`: {}",
            unpublished_in_list.join(", ")
        ));
    }
    if workflow_order != PUBLISH_ORDER {
        problems.push(format!(
            "release.yml publishes {workflow_order:?}, PUBLISH_ORDER is {PUBLISH_ORDER:?}"
        ));
    }

    if problems.is_empty() {
        eprintln!(
            "  {} crate(s) in the publish pipeline, workflow in step",
            PUBLISH_ORDER.len()
        );
        Ok(())
    } else {
        Err(problems.join("; "))
    }
}

/// The `name = "..."` of a `[package]` manifest.
fn manifest_name(manifest: &str) -> Option<String> {
    manifest.lines().find_map(|line| {
        let rest = line.trim().strip_prefix("name")?.trim_start();
        let value = rest.strip_prefix('=')?.trim();
        Some(value.trim_matches('"').to_owned())
    })
}

/// Whether the manifest opts out of publishing.
fn manifest_is_unpublished(manifest: &str) -> bool {
    manifest.lines().any(|line| {
        line.trim()
            .strip_prefix("publish")
            .and_then(|rest| rest.trim_start().strip_prefix('='))
            .is_some_and(|value| value.trim() == "false")
    })
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
    // Dry-run only packages and lists the files that would be published; it
    // does not verify against the registry because workspace-internal
    // dependencies at the new version do not yet exist on crates.io.
    // Live publish uses `--no-verify` for the same reason; `cargo xtask harden`
    // already compiles, tests, and lints.
    let flags = if live {
        vec!["publish", "--no-verify"]
    } else {
        vec!["package", "--list"]
    };

    let packages = PUBLISH_ORDER;
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

/// Guards a release against the three ways a version can drift apart: the git
/// tag that triggered the workflow, `workspace.package.version` in the root
/// `Cargo.toml`, and the matching `## [<version>]` heading in `CHANGELOG.md`.
///
/// Any publish that gets past this check is publishing the version its tag and
/// changelog claim. Without it, a stale tag silently publishes whatever the
/// workspace happens to say.
fn run_release_check(args: &[String]) -> ExitCode {
    let tag = args
        .iter()
        .position(|a| a == "--tag")
        .and_then(|i| args.get(i + 1))
        .map(String::as_str);
    let Some(tag) = tag else {
        eprintln!("xtask: release-check requires --tag <name>");
        return ExitCode::FAILURE;
    };
    // Both `v1.2.3` and `1.2.3` are accepted; `release.yml` triggers on either.
    let tag_version = tag.strip_prefix('v').unwrap_or(tag);

    let manifest = match std::fs::read_to_string("Cargo.toml") {
        Ok(c) => c,
        Err(e) => {
            eprintln!("xtask: cannot read Cargo.toml: {e}");
            return ExitCode::FAILURE;
        }
    };
    // `version      = "1.0.0-rc.1"` under `[workspace.package]`. The root
    // manifest has exactly one such key, so a line scan is enough and avoids a
    // TOML dependency in xtask.
    let workspace_version = manifest
        .lines()
        .map(str::trim)
        .find(|l| l.starts_with("version") && l.contains('='))
        .and_then(|l| l.split('"').nth(1));
    let Some(workspace_version) = workspace_version else {
        eprintln!("xtask: could not find workspace.package.version in Cargo.toml");
        return ExitCode::FAILURE;
    };

    let mut failed = false;
    if tag_version != workspace_version {
        eprintln!(
            "xtask: tag `{tag}` is version `{tag_version}` but workspace.package.version is `{workspace_version}`"
        );
        failed = true;
    }

    match std::fs::read_to_string("CHANGELOG.md") {
        Ok(changelog) => {
            let heading = format!("## [{workspace_version}]");
            if !changelog.contains(&heading) {
                eprintln!("xtask: CHANGELOG.md has no `{heading}` heading");
                failed = true;
            }
        }
        Err(e) => {
            eprintln!("xtask: cannot read CHANGELOG.md: {e}");
            failed = true;
        }
    }

    // The VS Code extension moves in lockstep with the crates; see docs/Versioning.md.
    // It drifted to 1.2.0 while every crate was on 1.0.0 because nothing checked.
    match std::fs::read_to_string("editor/vscode/package.json") {
        Ok(package_json) => match json_string_field(&package_json, "version") {
            Some(v) if v == workspace_version => {}
            Some(v) => {
                eprintln!(
                    "xtask: editor/vscode/package.json is version `{v}` but the workspace is `{workspace_version}` (see docs/Versioning.md)"
                );
                failed = true;
            }
            None => {
                eprintln!("xtask: editor/vscode/package.json has no `version` field");
                failed = true;
            }
        },
        Err(e) => {
            eprintln!("xtask: cannot read editor/vscode/package.json: {e}");
            failed = true;
        }
    }

    // Every internal `[workspace.dependencies]` pin must equal the workspace
    // version, or a published crate resolves a sibling from the previous release.
    for line in manifest.lines().map(str::trim) {
        let Some(rest) = line.strip_prefix("ruprizzle") else {
            continue;
        };
        if !rest.contains("path = \"crates/") {
            continue;
        }
        let pin = rest
            .rsplit_once("version = \"")
            .and_then(|(_, v)| v.split('"').next());
        match pin {
            Some(pin) if pin == workspace_version => {}
            Some(pin) => {
                eprintln!(
                    "xtask: internal dependency pin `{pin}` in Cargo.toml does not match the workspace version `{workspace_version}`: {line}"
                );
                failed = true;
            }
            None => {
                eprintln!(
                    "xtask: internal dependency has no version pin, which `cargo publish` requires: {line}"
                );
                failed = true;
            }
        }
    }

    if failed {
        return ExitCode::FAILURE;
    }
    eprintln!(
        "xtask: release-check ok - tag, workspace version, internal pins, CHANGELOG and the VS Code extension all agree on {workspace_version}"
    );
    ExitCode::SUCCESS
}

/// The string value of a top-level `"key": "value"` in a small JSON document.
///
/// `package.json` is read for one field, which does not justify a JSON dependency
/// in xtask.
fn json_string_field(json: &str, key: &str) -> Option<String> {
    let needle = format!("\"{key}\"");
    json.lines()
        .map(str::trim)
        .find(|l| l.starts_with(&needle))?
        .split(':')
        .nth(1)?
        .trim()
        .trim_end_matches(',')
        .trim()
        .strip_prefix('"')?
        .split('"')
        .next()
        .map(str::to_owned)
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
