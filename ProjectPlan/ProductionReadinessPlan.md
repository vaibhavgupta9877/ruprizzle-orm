# Production Readiness Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use `superpowers:subagent-driven-development` (recommended) or `superpowers:executing-plans` to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Close every gap identified in [ProductionReadiness.md](ProductionReadiness.md) so ruprizzle can be deployed to a production service with data that matters, moving the production-readiness score from 52/100 to a defensible 85+.

**Architecture:** Four sequential phases, ordered by risk rather than by effort. Phase A fixes confirmed defects in the migration engine — the only component whose bugs are unrecoverable. Phase B makes the runtime observable and tunable, which is what currently makes the library un-operable rather than merely incomplete. Phase C moves existing-but-dormant quality gates into automated CI. Phase D replaces assumption with measurement.

**Tech Stack:** Rust 2024 (MSRV 1.85), `sqlx` 0.8.6 over the `Any` driver, `tracing` 0.1, `criterion` 0.5, `proptest` 1, `insta`, `trybuild`, GitHub Actions, `cargo-deny`.

## Global Constraints

- **MSRV is 1.85.** Every change must compile on it. No feature requiring a later toolchain.
- **`#![forbid(unsafe_code)]` stays in all eight crates.** No exceptions, no `allow`.
- **Zero clippy warnings.** CI runs `cargo clippy --workspace --all-targets -- -D warnings`. `crates/migrate` and `crates/parser` additionally enable `clippy::pedantic`.
- **No `unwrap()` or `expect()` in new library source.** `cargo xtask harden` audits for these. Tests may use them freely.
- **No new panics on any path reachable from user input.** `Related::get()` remains the single sanctioned panic.
- **Every value reaching SQL is a bind parameter.** No `format!` may interpolate a `Value` or user-supplied identifier into a SQL string; `cargo xtask harden` greps for this.
- **Dual-database parity.** Any runtime or migration behaviour must be verified on both Postgres and SQLite via the `both_dbs!` macro from `ruprizzle-testkit`.
- **Version stays `0.1.0-alpha.1`** throughout this plan. Bump only at the Phase D exit gate.
- **Run DB-backed tests with `RUPRIZZLE_REQUIRE_DB=1`.** Without it an unreachable Postgres is silently skipped and the suite reports green while testing nothing. Local URL: `postgres://ruprizzle:ruprizzle@localhost:5432/ruprizzle_test`.

## Verification Command

The full gate used at every commit step in this plan:

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
RUPRIZZLE_REQUIRE_DB=1 \
  RUPRIZZLE_TEST_PG_URL=postgres://ruprizzle:ruprizzle@localhost:5432/ruprizzle_test \
  cargo test --workspace
```

Baseline at plan start: **167 tests passing, 0 failing, 0 clippy warnings.** Any task that reduces the passing count without explanation is a regression.

## File Structure

| File | Responsibility | Tasks |
|---|---|---|
| `crates/migrate/src/runner.rs` | Statement splitting, checksum verification, application | PR-01, PR-02 |
| `crates/migrate/tests/splitter.rs` | **new** — splitter unit tests | PR-01 |
| `crates/migrate/tests/concurrency.rs` | **new** — concurrent-deploy tests | PR-02, PR-14 |
| `crates/migrate/tests/roundtrip_prop.rs` | **new** — proptest diff round-trip | PR-13 |
| `crates/migrate/src/error.rs` | Migration error taxonomy | PR-03 |
| `crates/runtime/src/error.rs` | Runtime error taxonomy, PII policy | PR-03, PR-07 |
| `crates/runtime/src/executor.rs` | Query execution — the tracing choke point | PR-04 |
| `crates/runtime/src/tx.rs` | Transaction execution and tracing | PR-04 |
| `crates/runtime/src/pool.rs` | Pool construction, configuration, metrics | PR-05, PR-06 |
| `crates/runtime/benches/end_to_end.rs` | **new** — DB-backed benchmarks | PR-12 |
| `.github/workflows/ci.yml` | CI job matrix | PR-08, PR-09, PR-10 |
| `.github/dependabot.yml` | **new** — dependency updates | PR-10 |
| `SECURITY.md`, `CONTRIBUTING.md`, `CHANGELOG.md` | **new** — governance | PR-11 |
| `ProjectPlan/ImplementationPlan/ImplPlan10AppendixDecisions.md` | ADR record | PR-16 |

---

# Phase A — Correctness blockers

**Exit gate:** No known defect in the migration engine. A migration containing non-ASCII text or a `plpgsql` body applies byte-for-byte correctly on both backends, and two concurrent `migrate deploy` invocations both succeed.

---

## PR-01 · Fix the migration statement splitter

**Est:** 4h · **Severity:** CRITICAL — silent data corruption

`split_statements` scans SQL as raw bytes and casts each to `char`. `u8 as char` is a
Latin-1 widening, not UTF-8 decoding, so every multi-byte sequence is torn into
separate characters. `'café'` becomes `'cafÃ©'`, is written to the database, and the
migration is recorded as applied with a valid checksum. Nothing errors. Separately, the
scanner has no dollar-quote state, so a `plpgsql` function body splits at the
semicolons inside it.

**Files:**
- Modify: `crates/migrate/src/runner.rs:450-512` (`split_statements`, plus two new private helpers)
- Create: `crates/migrate/tests/splitter.rs`

**Interfaces:**
- Consumes: nothing from earlier tasks.
- Produces: `split_statements(sql: &str) -> Vec<String>` — unchanged public signature. Two new private helpers `dollar_tag_len(chars: &[char], i: usize) -> Option<usize>` and `matches_at(chars: &[char], i: usize, tag: &[char]) -> bool`, both file-private.

> **Note:** the implementation below has been prototyped against this working tree. It compiles, passes all eight tests, is clippy-clean under `clippy::pedantic`, and leaves the existing 167-test suite green.

- [x] **Step 1: Write the failing tests**

Create `crates/migrate/tests/splitter.rs`:

```rust
//! Tests for the migration statement splitter.

use ruprizzle_migrate::runner::split_statements;

#[test]
fn preserves_non_ascii_text() {
    let out = split_statements("INSERT INTO t (name) VALUES ('café');");
    assert_eq!(out, vec!["INSERT INTO t (name) VALUES ('café')"]);
}

#[test]
fn preserves_multibyte_outside_literals() {
    let out = split_statements("COMMENT ON TABLE t IS 'naïve — 日本語';");
    assert_eq!(out, vec!["COMMENT ON TABLE t IS 'naïve — 日本語'"]);
}

#[test]
fn keeps_dollar_quoted_body_intact() {
    let sql = "CREATE FUNCTION f() RETURNS trigger AS $$ BEGIN RETURN NEW; END; $$ LANGUAGE plpgsql;";
    let out = split_statements(sql);
    assert_eq!(out.len(), 1, "got {out:?}");
    assert!(out[0].contains("BEGIN RETURN NEW; END;"));
}

#[test]
fn keeps_tagged_dollar_quote_intact() {
    let sql = "CREATE FUNCTION f() RETURNS text AS $body$ SELECT 'a;b'; $body$ LANGUAGE sql;";
    let out = split_statements(sql);
    assert_eq!(out.len(), 1, "got {out:?}");
}

#[test]
fn comment_inside_dollar_quote_is_not_stripped() {
    let sql = "CREATE FUNCTION f() RETURNS int AS $$ -- keep me\n SELECT 1; $$ LANGUAGE sql;";
    let out = split_statements(sql);
    assert_eq!(out.len(), 1, "got {out:?}");
    assert!(out[0].contains("-- keep me"));
}

#[test]
fn bind_placeholder_is_not_a_dollar_quote() {
    let out = split_statements("SELECT * FROM t WHERE a = $1 AND b = $2; SELECT 1;");
    assert_eq!(
        out,
        vec!["SELECT * FROM t WHERE a = $1 AND b = $2", "SELECT 1"]
    );
}

#[test]
fn still_splits_plain_statements_and_strips_comments() {
    let sql = "CREATE TABLE a (id int); -- note\n/* block */ CREATE TABLE b (id int);";
    let out = split_statements(sql);
    assert_eq!(out.len(), 2, "got {out:?}");
    assert!(!out[1].contains("note"));
    assert!(!out[1].contains("block"));
}

#[test]
fn semicolon_inside_string_literal_does_not_split() {
    let out = split_statements("INSERT INTO t (s) VALUES ('a;b');");
    assert_eq!(out, vec!["INSERT INTO t (s) VALUES ('a;b')"]);
}
```

- [x] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p ruprizzle-migrate --test splitter`

Expected: 3 failures — `preserves_non_ascii_text` (`'cafÃ©'` vs `'café'`),
`preserves_multibyte_outside_literals`, `keeps_dollar_quoted_body_intact` (3 statements
vs 1). `keeps_tagged_dollar_quote_intact` and `comment_inside_dollar_quote_is_not_stripped`
also fail. The remaining three pass, which confirms the tests do not over-constrain
behaviour that already works.

- [x] **Step 3: Replace the scanner body**

In `crates/migrate/src/runner.rs`, replace the body of `split_statements` from
`let mut statements = Vec::new();` through the closing brace of the `while` loop:

```rust
    let mut statements = Vec::new();
    let mut current = String::new();
    // Scanned as `char`s, not bytes: `u8 as char` is a Latin-1 widening, which
    // silently turns any multi-byte UTF-8 sequence into mojibake.
    let chars: Vec<char> = sql.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        match chars[i] {
            '\'' => {
                current.push('\'');
                i += 1;
                while i < chars.len() {
                    current.push(chars[i]);
                    if chars[i] == '\'' {
                        i += 1;
                        break;
                    }
                    i += 1;
                }
            }
            // Dollar-quoted body (`$$ … $$` or `$tag$ … $tag$`): copied verbatim,
            // so a `;` or `--` inside a plpgsql function cannot split the statement.
            '$' if dollar_tag_len(&chars, i).is_some() => {
                let tag_len = dollar_tag_len(&chars, i).unwrap_or(0);
                let tag: Vec<char> = chars[i..i + tag_len].to_vec();
                current.extend(tag.iter());
                i += tag_len;
                while i < chars.len() {
                    if chars[i] == '$' && matches_at(&chars, i, &tag) {
                        current.extend(tag.iter());
                        i += tag_len;
                        break;
                    }
                    current.push(chars[i]);
                    i += 1;
                }
            }
            '-' if i + 1 < chars.len() && chars[i + 1] == '-' => {
                i += 2;
                while i < chars.len() && chars[i] != '\n' {
                    i += 1;
                }
                // The newline is preserved to act as whitespace.
                current.push(' ');
                i += 1;
            }
            '/' if i + 1 < chars.len() && chars[i + 1] == '*' => {
                i += 2;
                while i + 1 < chars.len() && !(chars[i] == '*' && chars[i + 1] == '/') {
                    i += 1;
                }
                if i + 1 < chars.len() {
                    i += 2;
                }
                current.push(' ');
            }
            ';' => {
                if !current.trim().is_empty() {
                    statements.push(current.trim().to_owned());
                }
                current.clear();
                i += 1;
            }
            c => {
                current.push(c);
                i += 1;
            }
        }
    }
```

- [x] **Step 4: Add the two private helpers**

Append immediately after `split_statements` in the same file:

```rust
/// If a dollar-quote tag (`$$` or `$name$`) starts at `i`, returns its length.
///
/// Returns `None` for a bind placeholder such as `$1`, because a Postgres tag
/// may not start with a digit.
fn dollar_tag_len(chars: &[char], i: usize) -> Option<usize> {
    if chars.get(i) != Some(&'$') {
        return None;
    }
    let mut j = i + 1;
    if chars.get(j).is_some_and(|c| *c != '$') {
        if !chars.get(j).is_some_and(|c| c.is_alphabetic() || *c == '_') {
            return None;
        }
        while chars
            .get(j)
            .is_some_and(|c| c.is_alphanumeric() || *c == '_')
        {
            j += 1;
        }
    }
    if chars.get(j) == Some(&'$') {
        Some(j - i + 1)
    } else {
        None
    }
}

/// Whether `tag` occurs at `i` in `chars`.
fn matches_at(chars: &[char], i: usize, tag: &[char]) -> bool {
    chars.len() >= i + tag.len() && chars[i..i + tag.len()] == *tag
}
```

- [x] **Step 5: Run the tests to verify they pass**

Run: `cargo test -p ruprizzle-migrate`

Expected: `splitter.rs` — 8 passed, 0 failed. `diff.rs` — 5 passed (unchanged).

- [x] **Step 6: Run the full gate**

Run the Verification Command from the plan header.

Expected: 175 tests passing (167 baseline + 8 new), 0 clippy warnings.

- [x] **Step 7: Commit**

```bash
git add crates/migrate/src/runner.rs crates/migrate/tests/splitter.rs
git commit -m "fix(migrate): scan migration SQL as chars and honour dollar quoting

split_statements cast each byte to a char, which is a Latin-1 widening
rather than UTF-8 decoding. Any multi-byte sequence in a migration was
torn into separate characters, written to the database as mojibake, and
recorded as applied under a valid checksum -- silent data corruption on
the one path where silent failure is least acceptable.

The scanner also had no dollar-quote state, so a plpgsql function body
split at the semicolons inside it, making triggers and stored procedures
inexpressible in a migration.

Scan chars, and copy dollar-quoted bodies verbatim. Bind placeholders
(\$1) are excluded from tag detection, since a Postgres tag may not
start with a digit."
```

---

## PR-02 · Make migration application safe under concurrency

**Est:** 6h · **Severity:** HIGH

`apply_all` computes the pending set at `runner.rs:209-214` but does not take the
advisory lock until `runner.rs:238`, inside each per-migration transaction. Two
deployers starting together — a rolling deploy, two replicas, a CI re-run — both
compute the same pending list. The lock correctly serialises the transactions, but once
the first commits, the second re-runs the same DDL and fails with *"relation already
exists"*. Integrity is preserved by the transaction; a deploy fails for no reason.

Two further defects in the same function are fixed here because they are the same
review: `execution_ms` records `start.elapsed()` — cumulative time since the loop began,
so the third migration reports the total of all three — and the advisory lock key is the
hardcoded literal `42`, which shares a global per-database namespace with every other
tool that picked the most obvious magic number.

The fix is a re-check inside the lock rather than a longer-lived lock. Holding a
session-scoped `pg_advisory_lock` across the whole run would require managing the lock's
lifetime against a pooled connection that may be reset on release; re-checking is
smaller, has no lifetime hazard, and makes the operation idempotent by construction.

**Files:**
- Modify: `crates/migrate/src/runner.rs:200-287` (`apply_all`), plus one new private helper
- Create: `crates/migrate/tests/concurrency.rs`

**Interfaces:**
- Consumes: `split_statements` from PR-01 (unchanged signature).
- Produces: `apply_all(&self, pool: &AnyPool, accept_data_loss: bool) -> Result<Report, Error>` — unchanged signature, now idempotent. New private `fn advisory_lock_key() -> i64`.

- [ ] **Step 1: Write the failing test**

Create `crates/migrate/tests/concurrency.rs`:

```rust
//! Concurrent `apply_all` must be idempotent, not merely serialised.

use ruprizzle_migrate::Migrator;
use ruprizzle_testkit::{TestDb, both_dbs};

/// Writes a two-migration directory into a temp dir and returns its path.
fn fixture() -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("tempdir");
    for (id, sql) in [
        ("20260101000000_first", "CREATE TABLE conc_a (id INTEGER PRIMARY KEY);"),
        ("20260101000001_second", "CREATE TABLE conc_b (id INTEGER PRIMARY KEY);"),
    ] {
        let m = dir.path().join(id);
        std::fs::create_dir_all(&m).expect("create migration dir");
        std::fs::write(m.join("up.sql"), sql).expect("write up.sql");
        std::fs::write(m.join("down.sql"), "").expect("write down.sql");
    }
    dir
}

both_dbs!(apply_all_twice_is_idempotent, |db: TestDb| async move {
    let dir = fixture();
    let migrator = Migrator::new(dir.path());

    let first = migrator.apply_all(db.pool(), false).await.expect("first apply");
    assert_eq!(first.applied.len(), 2);

    // The second run models a concurrent deployer that computed the same
    // pending set before the first one committed.
    let second = migrator
        .apply_all(db.pool(), false)
        .await
        .expect("second apply must not error");
    assert!(
        second.applied.is_empty(),
        "second run should be a no-op, applied {:?}",
        second.applied
    );
});
```

- [ ] **Step 2: Run the test to verify current behaviour**

Run:
```bash
RUPRIZZLE_REQUIRE_DB=1 \
  RUPRIZZLE_TEST_PG_URL=postgres://ruprizzle:ruprizzle@localhost:5432/ruprizzle_test \
  cargo test -p ruprizzle-migrate --test concurrency
```

Expected: **PASS.** This test does not fail today, and that is the point — it is a
regression guard for the sequential path, which the outer `applied_ids` filter already
handles. The defect is only reachable when one process's pending set was computed before
another process committed, which cannot be produced sequentially: after the first
`apply_all` returns, the tracking table *does* contain the record, so any later call
filters it out before reaching the lock.

**The re-check added in Step 4 is therefore proved by PR-14, not here.** That is why
PR-14 exists as a separate task with ten racing tokio tasks. Do not attempt to fake the
race by deleting rows from `_ruprizzle_migrations` — that produces a genuinely empty
tracking table, which the re-check reads correctly and which no concurrent deployer ever
sees. It would test a state that cannot occur.

- [ ] **Step 3: Add a unit test for the lock key**

Append to the bottom of `crates/migrate/src/runner.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::advisory_lock_key;

    #[test]
    fn lock_key_is_stable_and_not_a_small_literal() {
        let k = advisory_lock_key();
        assert_eq!(k, advisory_lock_key(), "key must be deterministic");
        assert!(
            k.unsigned_abs() > u64::from(u16::MAX),
            "key {k} is small enough to collide with a hand-picked literal"
        );
    }
}
```

Run: `cargo test -p ruprizzle-migrate --lib`

Expected: fails to compile — `advisory_lock_key` does not exist yet.

- [ ] **Step 4: Add the in-lock re-check and the derived lock key**

In `crates/migrate/src/runner.rs`, inside the `for m in pending` loop, immediately after
the advisory-lock query and before `let statements = split_statements(&m.up);`:

```rust
            // Re-read inside the lock. Our pending set was computed before the
            // lock was held, so a concurrent deployer may have applied this
            // migration in between; re-running its DDL would fail on
            // "already exists" for what is really a no-op.
            let already: Option<(String,)> = sqlx::query_as(
                "SELECT id FROM _ruprizzle_migrations \
                 WHERE id = $1 AND rolled_back_at IS NULL",
            )
            .bind(&m.id)
            .fetch_optional(&mut *tx)
            .await?;
            if already.is_some() {
                tx.rollback().await?;
                continue;
            }

            let stmt_start = Instant::now();
```

Replace the hardcoded lock query:

```rust
            if is_postgres {
                sqlx::query("SELECT pg_advisory_xact_lock($1)")
                    .bind(advisory_lock_key())
                    .execute(&mut *tx)
                    .await?;
            }
```

Replace the timing line (`let elapsed = start.elapsed().as_millis() as i64;`) with:

```rust
            let elapsed = stmt_start.elapsed().as_millis() as i64;
```

Append the key helper next to `compute_checksum`:

```rust
/// The advisory lock key for migration application.
///
/// Derived from the tracking table name rather than a literal, because advisory
/// lock keys share one namespace per database: a hardcoded small integer will
/// eventually contend with an unrelated application that picked the same one.
fn advisory_lock_key() -> i64 {
    let digest = Sha256::digest(b"_ruprizzle_migrations");
    i64::from_be_bytes(digest[..8].try_into().unwrap_or([0; 8]))
}
```

- [ ] **Step 5: Run the tests to verify they pass**

Run:
```bash
RUPRIZZLE_REQUIRE_DB=1 \
  RUPRIZZLE_TEST_PG_URL=postgres://ruprizzle:ruprizzle@localhost:5432/ruprizzle_test \
  cargo test -p ruprizzle-migrate
```

Expected: all concurrency tests pass on both backends; the 6 pre-existing
`tests/integration/tests/migrations.rs` tests still pass.

- [ ] **Step 6: Run the full gate**

Run the Verification Command. Expected: 0 failures, 0 clippy warnings.

- [ ] **Step 7: Commit**

```bash
git add crates/migrate/src/runner.rs crates/migrate/tests/concurrency.rs
git commit -m "fix(migrate): make apply_all idempotent, and fix lock key and timing

The pending set was computed before the advisory lock was taken, so two
concurrent deployers both saw the same work. The lock serialised their
transactions but the second still re-ran the DDL and failed on
'relation already exists' -- a failed deploy for what was a no-op.

Re-read the tracking table inside the lock and skip anything a
concurrent run already applied.

Also: derive the advisory lock key from the tracking table name instead
of the literal 42, which shares a per-database namespace with every
other tool that picked the most obvious number; and time each migration
from its own start rather than reporting cumulative elapsed time for
every row."
```

---

## PR-03 · Make public error enums `#[non_exhaustive]`

**Est:** 1h · **Severity:** MEDIUM — semver hygiene, cheap now and expensive later

`ruprizzle::Error` and `ruprizzle_migrate::Error` are public enums without
`#[non_exhaustive]`. Downstream `match` arms are therefore exhaustive, so adding a
variant — inevitable for a database library that will meet new constraint classes — is
a breaking change. The attribute costs nothing before 0.1.0 and a major version after.

Verified safe: no crate in this workspace matches exhaustively on either enum, and no
test asserts on their variant set.

**Files:**
- Modify: `crates/runtime/src/error.rs:6`
- Modify: `crates/migrate/src/error.rs:8`

**Interfaces:**
- Produces: both enums gain `#[non_exhaustive]`. Downstream matches must add a `_ => …` arm; in-crate matches are unaffected.

- [ ] **Step 1: Add the attribute to the runtime error**

In `crates/runtime/src/error.rs`, above `pub enum Error {`:

```rust
/// Errors returned by the runtime.
///
/// `#[non_exhaustive]`: new database backends and new constraint classes will
/// add variants, and that must not be a breaking change. Match with a trailing
/// `_ =>` arm.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
```

- [ ] **Step 2: Add the attribute to the migration error**

In `crates/migrate/src/error.rs`, replace the derive block above `pub enum Error {`:

```rust
/// Errors from the migration engine.
///
/// `#[non_exhaustive]`: see the note on `ruprizzle::Error`.
#[derive(Debug, thiserror::Error)]
#[allow(missing_docs)]
#[non_exhaustive]
pub enum Error {
```

- [ ] **Step 3: Verify nothing in the workspace broke**

Run: `cargo build --workspace --all-targets`

Expected: clean build. If any in-workspace match fails, it was cross-crate and needs a
`_ =>` arm — add one that returns the error unchanged rather than swallowing it.

- [ ] **Step 4: Run the full gate**

Run the Verification Command. Expected: 0 failures.

- [ ] **Step 5: Commit**

```bash
git add crates/runtime/src/error.rs crates/migrate/src/error.rs
git commit -m "fix: mark public error enums non_exhaustive

New backends and new constraint classes will add variants. Without this
attribute every downstream match is exhaustive, so each addition is a
breaking change. Free before 0.1.0; a major version after."
```

---

# Phase B — Operability

**Exit gate:** A running service can see every query it issues with timing, tune its pool to its database, expose pool saturation to a metrics endpoint, and answer a readiness probe. No user data reaches logs by default.

---

## PR-04 · Instrument query execution with `tracing`

**Est:** 2d · **Severity:** CRITICAL for operations

A workspace search for `tracing::` and `log::` across `crates/runtime/src` and
`crates/migrate/src` returns zero results. In production you cannot answer *which query
is slow*, *which request issued it*, or *how long that migration took*. `.to_sql()` is a
good development affordance and not a substitute for runtime telemetry.

`tracing` 0.1.44 is **already in the dependency tree** — sqlx-core depends on it — so
this adds no new crates. It is also a bonus: sqlx already emits its own slow-acquire
warnings through `tracing` (`acquire_slow_threshold` defaults to 2s), which become
visible for free the moment a subscriber exists.

The `Executor` trait is the correct choke point: every builder runs through
`fetch_all_raw` / `execute_raw` / `stream_raw`, and both `Pool` and `Tx` implement it.

**Files:**
- Modify: `crates/runtime/Cargo.toml` (add `tracing`)
- Modify: `crates/runtime/src/executor.rs:58-89` (the `impl Executor for Pool` block)
- Modify: `crates/runtime/src/tx.rs:242+` (the `impl Executor for Tx` block)
- Modify: `crates/migrate/Cargo.toml` and `crates/migrate/src/runner.rs` (migration events)
- Create: `crates/runtime/tests/tracing.rs`

**Interfaces:**
- Consumes: `Executor` trait as defined at `crates/runtime/src/executor.rs:23-47` — `fetch_all_raw(&self, sql: String, binds: Vec<Value>) -> BoxFuture<'_, Result<Vec<AnyRow>, Error>>`, `execute_raw(&self, sql: String, binds: Vec<Value>) -> BoxFuture<'_, Result<u64, Error>>`, `stream_raw(&self, sql: String, binds: Vec<Value>) -> BoxRowStream<'_>`.
- Produces: events on target `ruprizzle::query` at `DEBUG` (success) and `WARN` (failure), with fields `sql`, `binds`, `rows`, `elapsed_ms`, `error`. Events on target `ruprizzle::migrate` at `INFO` per migration.

- [ ] **Step 1: Add the dependency**

In `crates/runtime/Cargo.toml`, under `[dependencies]`:

```toml
# Already in the tree via sqlx-core, so this adds no new crates. Near-zero cost
# with no subscriber installed.
tracing           = { version = "0.1", default-features = false, features = ["std"] }
```

Add the same line to `crates/migrate/Cargo.toml`.

- [ ] **Step 2: Write the failing test**

Create `crates/runtime/tests/tracing.rs`:

```rust
//! Every statement that reaches the database must emit a `ruprizzle::query` event.

use std::sync::{Arc, Mutex};

use ruprizzle::Executor;
use tracing::{Event, Subscriber};
use tracing_subscriber::layer::{Context, Layer, SubscriberExt};
use tracing_subscriber::registry::Registry;

/// Collects the `ruprizzle::query` events emitted while it is installed.
#[derive(Default, Clone)]
struct Captured(Arc<Mutex<Vec<String>>>);

impl<S: Subscriber> Layer<S> for Captured {
    fn on_event(&self, event: &Event<'_>, _ctx: Context<'_, S>) {
        if event.metadata().target() == "ruprizzle::query" {
            self.0
                .lock()
                .expect("lock")
                .push(event.metadata().name().to_owned());
        }
    }
}

#[test]
fn query_emits_a_tracing_event() {
    let captured = Captured::default();
    let subscriber = Registry::default().with(captured.clone());

    // `with_default` scopes the subscriber to this thread, so the runtime must
    // stay on it: a multi-thread runtime would execute the query on a worker
    // that has no subscriber installed and capture nothing.
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime");

    tracing::subscriber::with_default(subscriber, || {
        rt.block_on(async {
            let pool = ruprizzle::connect("sqlite::memory:").await.expect("connect");
            pool.execute_raw("CREATE TABLE t (id INTEGER)".to_owned(), Vec::new())
                .await
                .expect("create table");
        });
    });

    assert!(
        !captured.0.lock().expect("lock").is_empty(),
        "no ruprizzle::query event was emitted"
    );
}

#[test]
fn failed_query_emits_an_event_too() {
    let captured = Captured::default();
    let subscriber = Registry::default().with(captured.clone());
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime");

    tracing::subscriber::with_default(subscriber, || {
        rt.block_on(async {
            let pool = ruprizzle::connect("sqlite::memory:").await.expect("connect");
            let _ = pool
                .execute_raw("THIS IS NOT SQL".to_owned(), Vec::new())
                .await;
        });
    });

    assert!(
        !captured.0.lock().expect("lock").is_empty(),
        "a failing query must still be observable"
    );
}
```

Add to `crates/runtime/Cargo.toml` under `[dev-dependencies]`:

```toml
tracing-subscriber = { version = "0.3", default-features = false, features = ["registry"] }
```

- [ ] **Step 3: Run the test to verify it fails**

Run: `cargo test -p ruprizzle --test tracing`

Expected: FAIL — `no ruprizzle::query event was emitted`.

- [ ] **Step 4: Instrument the `Pool` implementation**

In `crates/runtime/src/executor.rs`, replace the three method bodies in
`impl Executor for Pool`:

```rust
    fn fetch_all_raw(
        &self,
        sql: String,
        binds: Vec<Value>,
    ) -> BoxFuture<'_, Result<Vec<AnyRow>, Error>> {
        Box::pin(async move {
            let bind_count = binds.len();
            let started = std::time::Instant::now();
            let mut q = sqlx::query::<sqlx::Any>(&sql);
            for b in binds {
                q = q.bind(b);
            }
            let result = q.fetch_all(self).await.map_err(Error::from);
            let elapsed_ms = started.elapsed().as_millis() as u64;
            match &result {
                Ok(rows) => tracing::debug!(
                    target: "ruprizzle::query",
                    sql = %sql,
                    binds = bind_count,
                    rows = rows.len(),
                    elapsed_ms,
                    "query"
                ),
                Err(e) => tracing::warn!(
                    target: "ruprizzle::query",
                    sql = %sql,
                    binds = bind_count,
                    elapsed_ms,
                    error = %e,
                    "query failed"
                ),
            }
            result
        })
    }

    fn execute_raw(&self, sql: String, binds: Vec<Value>) -> BoxFuture<'_, Result<u64, Error>> {
        Box::pin(async move {
            let bind_count = binds.len();
            let started = std::time::Instant::now();
            let mut q = sqlx::query::<sqlx::Any>(&sql);
            for b in binds {
                q = q.bind(b);
            }
            let result = q
                .execute(self)
                .await
                .map(|r| r.rows_affected())
                .map_err(Error::from);
            let elapsed_ms = started.elapsed().as_millis() as u64;
            match &result {
                Ok(n) => tracing::debug!(
                    target: "ruprizzle::query",
                    sql = %sql,
                    binds = bind_count,
                    rows_affected = n,
                    elapsed_ms,
                    "execute"
                ),
                Err(e) => tracing::warn!(
                    target: "ruprizzle::query",
                    sql = %sql,
                    binds = bind_count,
                    elapsed_ms,
                    error = %e,
                    "execute failed"
                ),
            }
            result
        })
    }
```

`stream_raw` delegates to `fetch_all_raw` and therefore needs no separate event.

**Bind values are deliberately not logged.** They are user data; see PR-07. Only the
count is emitted.

- [ ] **Step 5: Instrument the `Tx` implementation**

Apply the identical treatment to the `impl Executor for Tx` block in
`crates/runtime/src/tx.rs`, and add one event each to `Tx::commit` and `Tx::rollback`:

```rust
        tracing::debug!(target: "ruprizzle::query", "transaction committed");
```

```rust
        tracing::debug!(target: "ruprizzle::query", "transaction rolled back");
```

- [ ] **Step 6: Instrument migration application**

In `crates/migrate/src/runner.rs`, inside the `for m in pending` loop, immediately after
`let statements = split_statements(&m.up);` — the count is not in scope before that line,
and PR-02 inserts `stmt_start` above it:

```rust
            tracing::info!(
                target: "ruprizzle::migrate",
                migration = %m.id,
                statements = statements.len(),
                "applying migration"
            );
```

and immediately before `applied_ids.push(m.id);`:

```rust
            tracing::info!(
                target: "ruprizzle::migrate",
                migration = %m.id,
                elapsed_ms = elapsed,
                "migration applied"
            );
```

- [ ] **Step 7: Run the test to verify it passes**

Run: `cargo test -p ruprizzle --test tracing`

Expected: PASS.

- [ ] **Step 8: Document it**

Add a `## Observability` section to `docs/query-guide.md` showing a `tracing-subscriber`
setup and the two targets:

```rust
tracing_subscriber::fmt()
    .with_env_filter("ruprizzle::query=debug,ruprizzle::migrate=info")
    .init();
```

Remove the *"connection pool metrics and query logging"* bullet from the 0.2 deferral
list in `docs/known-limitations.md`.

- [ ] **Step 9: Run the full gate and commit**

```bash
git add crates/runtime crates/migrate docs/
git commit -m "feat: emit tracing events for every query, transaction, and migration

The runtime had no logging of any kind, so a production deployment could
not see a slow query, correlate a query to a request, or time a
migration. The Executor trait is the choke point every builder runs
through, so instrumenting it covers the pool and transaction paths alike.

tracing was already in the tree via sqlx-core, so this adds no crates --
and sqlx's own slow-acquire warnings become visible for free once a
subscriber exists.

Bind values are not logged: they are user data. Only the count is."
```

---

## PR-05 · Make the connection pool configurable

**Est:** 1d · **Severity:** HIGH

`crates/runtime/src/pool.rs` is seventeen lines. `connect(url)` accepts a URL and
nothing else, so a deployment cannot size the pool to its database's `max_connections`,
cannot recycle connections through a failover, and cannot set a statement timeout — one
pathological query holds a connection indefinitely.

Defaults below mirror sqlx's own (`max_connections` 10, `min_connections` 0,
`acquire_timeout` 30s, `idle_timeout` 10min, `max_lifetime` 30min,
`test_before_acquire` true), so this is purely additive: existing `connect` callers see
identical behaviour.

**Files:**
- Modify: `crates/runtime/src/pool.rs`
- Modify: `crates/runtime/src/lib.rs:52` (re-export)
- Create: `crates/runtime/tests/pool_config.rs`

**Interfaces:**
- Consumes: `sqlx::any::AnyPoolOptions` (verified present in sqlx 0.8.6 at `sqlx-core/src/any/mod.rs:52`), whose builder exposes `max_connections(u32)`, `min_connections(u32)`, `acquire_timeout(Duration)`, `idle_timeout(impl Into<Option<Duration>>)`, `max_lifetime(impl Into<Option<Duration>>)`, `test_before_acquire(bool)`, and `connect(self, url: &str)`.
- Produces: `PoolConfig` struct with public fields, `PoolConfig::default()`, and `connect_with(url: &str, config: &PoolConfig) -> Result<Pool, Error>`. `connect(url)` is retained and delegates.

- [ ] **Step 1: Write the failing test**

Create `crates/runtime/tests/pool_config.rs`:

```rust
//! The pool must be configurable, and its defaults must match sqlx's.

use std::time::Duration;

use ruprizzle::pool::{PoolConfig, connect_with};

#[test]
fn defaults_match_sqlx() {
    let c = PoolConfig::default();
    assert_eq!(c.max_connections, 10);
    assert_eq!(c.min_connections, 0);
    assert_eq!(c.acquire_timeout, Duration::from_secs(30));
    assert_eq!(c.idle_timeout, Some(Duration::from_secs(600)));
    assert_eq!(c.max_lifetime, Some(Duration::from_secs(1800)));
    assert!(c.test_before_acquire);
}

#[tokio::test]
async fn honours_max_connections() {
    let config = PoolConfig {
        max_connections: 3,
        ..PoolConfig::default()
    };
    let pool = connect_with("sqlite::memory:", &config)
        .await
        .expect("connect");
    // sqlx reports the configured ceiling through the pool options it was built
    // with; opening one connection proves the pool is live and configured.
    assert!(pool.options().get_max_connections() <= 3);
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p ruprizzle --test pool_config`

Expected: FAIL to compile — `PoolConfig` and `connect_with` do not exist.

- [ ] **Step 3: Implement the configuration surface**

Replace `crates/runtime/src/pool.rs` entirely:

```rust
//! Connection pool construction, configuration, and metrics.

use std::time::Duration;

use sqlx::any::AnyPoolOptions;

/// A `sqlx` pool over the `Any` driver.
pub type Pool = sqlx::Pool<sqlx::Any>;

/// How a [`Pool`] is built.
///
/// Every default here matches sqlx's own, so moving from [`connect`] to
/// [`connect_with`] changes nothing until you change a field.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct PoolConfig {
    /// Maximum connections held open. Size this against your database's own
    /// `max_connections`, divided by the number of application instances.
    pub max_connections: u32,
    /// Connections kept warm even when idle.
    pub min_connections: u32,
    /// How long `acquire` waits before giving up.
    pub acquire_timeout: Duration,
    /// How long an idle connection survives. `None` keeps it forever.
    pub idle_timeout: Option<Duration>,
    /// Hard lifetime cap, after which a connection is recycled. Set this below
    /// any load balancer or proxy idle cut-off.
    pub max_lifetime: Option<Duration>,
    /// Ping a connection before handing it out.
    pub test_before_acquire: bool,
}

impl Default for PoolConfig {
    fn default() -> Self {
        Self {
            max_connections: 10,
            min_connections: 0,
            acquire_timeout: Duration::from_secs(30),
            idle_timeout: Some(Duration::from_secs(10 * 60)),
            max_lifetime: Some(Duration::from_secs(30 * 60)),
            test_before_acquire: true,
        }
    }
}

/// Connect to a database by URL using default pool settings.
///
/// The URL scheme selects the driver (`postgres://`, `sqlite://`, etc.).
///
/// # Errors
///
/// Returns an error if the URL cannot be parsed or the connection fails.
pub async fn connect(url: &str) -> Result<Pool, crate::Error> {
    connect_with(url, &PoolConfig::default()).await
}

/// Connect to a database by URL with explicit pool settings.
///
/// # Errors
///
/// Returns an error if the URL cannot be parsed or the connection fails.
pub async fn connect_with(url: &str, config: &PoolConfig) -> Result<Pool, crate::Error> {
    sqlx::any::install_default_drivers();
    AnyPoolOptions::new()
        .max_connections(config.max_connections)
        .min_connections(config.min_connections)
        .acquire_timeout(config.acquire_timeout)
        .idle_timeout(config.idle_timeout)
        .max_lifetime(config.max_lifetime)
        .test_before_acquire(config.test_before_acquire)
        .connect(url)
        .await
        .map_err(Into::into)
}
```

- [ ] **Step 4: Re-export from the crate root**

In `crates/runtime/src/lib.rs`, change the pool re-export line:

```rust
pub use pool::{Pool, PoolConfig, connect, connect_with};
```

- [ ] **Step 5: Run the test to verify it passes**

Run: `cargo test -p ruprizzle --test pool_config`

Expected: PASS. If `pool.options()` is not public in sqlx 0.8.6, drop that assertion and
assert only that the pool connects — the defaults test carries the contract.

- [ ] **Step 6: Document and commit**

Add a `## Connection pooling` section to `docs/query-guide.md` with a worked example
sizing `max_connections` against Postgres's own limit.

```bash
git add crates/runtime/src/pool.rs crates/runtime/src/lib.rs \
        crates/runtime/tests/pool_config.rs docs/query-guide.md
git commit -m "feat(pool): expose pool configuration

connect(url) took a URL and nothing else, so a deployment could not size
the pool to its database, recycle connections through a failover, or cap
connection lifetime below a proxy's idle cut-off.

PoolConfig defaults mirror sqlx's exactly, so this is additive: existing
connect() callers see identical behaviour."
```

---

## PR-06 · Expose pool metrics and a health check

**Est:** 1d · **Severity:** MEDIUM

A readiness probe needs to know the database is reachable; an autoscaler and a dashboard
need to know whether the pool is saturated. Neither is currently answerable.

**Files:**
- Modify: `crates/runtime/src/pool.rs` (append)
- Modify: `crates/runtime/src/lib.rs` (re-export)
- Modify: `crates/runtime/tests/pool_config.rs` (append)

**Interfaces:**
- Consumes: `PoolConfig` and `connect_with` from PR-05; `sqlx::Pool::size() -> u32` and `Pool::num_idle() -> usize` (verified at `sqlx-core/src/pool/mod.rs:535,540`).
- Produces: `PoolStats { size: u32, idle: usize, in_use: usize }`, `stats(pool: &Pool) -> PoolStats`, `ping(pool: &Pool) -> Result<(), Error>`.

- [ ] **Step 1: Write the failing test**

Append to `crates/runtime/tests/pool_config.rs`:

```rust
#[tokio::test]
async fn stats_and_ping_report_a_live_pool() {
    let pool = ruprizzle::connect("sqlite::memory:").await.expect("connect");

    ruprizzle::pool::ping(&pool).await.expect("ping must succeed");

    let s = ruprizzle::pool::stats(&pool);
    assert_eq!(s.in_use + s.idle, s.size as usize);
}

#[tokio::test]
async fn ping_fails_against_an_unreachable_database() {
    let config = PoolConfig {
        acquire_timeout: Duration::from_millis(200),
        ..PoolConfig::default()
    };
    // Lazy connect so construction succeeds and the failure surfaces at ping.
    let Ok(pool) = connect_with("postgres://127.0.0.1:1/nope", &config).await else {
        return; // eager connect already refused, which is also a pass
    };
    assert!(ruprizzle::pool::ping(&pool).await.is_err());
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p ruprizzle --test pool_config`

Expected: FAIL to compile — `stats` and `ping` do not exist.

- [ ] **Step 3: Implement stats and ping**

Append to `crates/runtime/src/pool.rs`:

```rust
/// A point-in-time view of pool saturation, for metrics endpoints.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct PoolStats {
    /// Connections currently held by the pool, idle or in use.
    pub size: u32,
    /// Connections available for immediate checkout.
    pub idle: usize,
    /// Connections currently checked out.
    pub in_use: usize,
}

/// Samples pool saturation.
///
/// Cheap enough to call on every scrape of a metrics endpoint.
#[must_use]
pub fn stats(pool: &Pool) -> PoolStats {
    let size = pool.size();
    let idle = pool.num_idle();
    PoolStats {
        size,
        idle,
        in_use: (size as usize).saturating_sub(idle),
    }
}

/// Checks that the database is reachable.
///
/// Intended for readiness probes. Runs `SELECT 1`, which every supported
/// backend accepts.
///
/// # Errors
///
/// Returns an error if a connection cannot be acquired or the query fails.
pub async fn ping(pool: &Pool) -> Result<(), crate::Error> {
    sqlx::query("SELECT 1")
        .execute(pool)
        .await
        .map(|_| ())
        .map_err(Into::into)
}
```

- [ ] **Step 4: Re-export**

In `crates/runtime/src/lib.rs`:

```rust
pub use pool::{Pool, PoolConfig, PoolStats, connect, connect_with, ping, stats};
```

- [ ] **Step 5: Run the test to verify it passes**

Run: `cargo test -p ruprizzle --test pool_config`. Expected: PASS.

- [ ] **Step 6: Run the full gate and commit**

```bash
git add crates/runtime/src/pool.rs crates/runtime/src/lib.rs crates/runtime/tests/pool_config.rs
git commit -m "feat(pool): add saturation metrics and a health check

A readiness probe needs to know the database is reachable and a
dashboard needs to know whether the pool is saturated. Neither was
answerable from outside the crate."
```

---

## PR-07 · Keep user data out of error messages by default

**Est:** 1d · **Severity:** MEDIUM — compliance

`Error::UniqueViolation` interpolates the conflicting value into its `Display` output
(`crates/runtime/src/error.rs:7`). For the commonest case — a duplicate signup — that
value is an email address, and every web framework logs errors by default. Under GDPR
and similar regimes this must be deliberate and opt-in.

Verified safe: no test in the workspace asserts on this message.

**Files:**
- Modify: `crates/runtime/src/error.rs:6-12`
- Create: `crates/runtime/tests/error_redaction.rs`

**Interfaces:**
- Consumes: `Error` with `#[non_exhaustive]` from PR-03.
- Produces: `UniqueViolation` `Display` no longer contains `value`. New method `Error::conflicting_value(&self) -> Option<&str>` for callers that want it.

- [ ] **Step 1: Write the failing test**

Create `crates/runtime/tests/error_redaction.rs`:

```rust
//! User data must not reach logs through an error's Display.

use ruprizzle::Error;

#[test]
fn unique_violation_display_omits_the_value() {
    let e = Error::UniqueViolation {
        table: "users".to_owned(),
        columns: "email".to_owned(),
        value: Some("alice@example.com".to_owned()),
    };
    let rendered = e.to_string();
    assert!(
        !rendered.contains("alice@example.com"),
        "PII leaked into Display: {rendered}"
    );
    assert!(rendered.contains("users"));
    assert!(rendered.contains("email"));
}

#[test]
fn conflicting_value_is_available_on_request() {
    let e = Error::UniqueViolation {
        table: "users".to_owned(),
        columns: "email".to_owned(),
        value: Some("alice@example.com".to_owned()),
    };
    assert_eq!(e.conflicting_value(), Some("alice@example.com"));
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p ruprizzle --test error_redaction`

Expected: `unique_violation_display_omits_the_value` FAILS (value is present);
`conflicting_value_is_available_on_request` fails to compile.

- [ ] **Step 3: Redact the Display and add the accessor**

In `crates/runtime/src/error.rs`, replace the `UniqueViolation` variant's error
attribute:

```rust
    /// A unique constraint was violated.
    ///
    /// The conflicting value is captured but deliberately kept out of
    /// `Display`: it is user data, and errors are logged by default in every
    /// web framework. Reach it explicitly via
    /// [`conflicting_value`](Error::conflicting_value).
    #[error("unique constraint violated on `{table}.{columns}`")]
    UniqueViolation {
        table: String,
        columns: String,
        value: Option<String>,
    },
```

Append an `impl` block after the enum:

```rust
impl Error {
    /// The value that violated a unique constraint, if one was captured.
    ///
    /// This is user data. Logging it is a deliberate choice, which is why it
    /// is not part of [`Display`](std::fmt::Display).
    #[must_use]
    pub fn conflicting_value(&self) -> Option<&str> {
        match self {
            Error::UniqueViolation { value, .. } => value.as_deref(),
            _ => None,
        }
    }
}
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `cargo test -p ruprizzle --test error_redaction`. Expected: 2 passed.

- [ ] **Step 5: Document and commit**

Add a note to `docs/query-guide.md` under error handling explaining the policy.

```bash
git add crates/runtime/src/error.rs crates/runtime/tests/error_redaction.rs docs/query-guide.md
git commit -m "fix: keep the conflicting value out of UniqueViolation's Display

The commonest unique violation is a duplicate signup, so the value is
usually an email address -- and every web framework logs errors by
default. Capture it still, but make reaching it a deliberate call."
```

---

# Phase C — CI and supply chain

**Exit gate:** Every quality gate that exists runs automatically on every push, on three
operating systems, with dependency advisories checked.

---

## PR-08 · Fix the stale CI job and wire up the real generated-code gate

**Est:** 2h · **Severity:** HIGH — CI is currently red

The `generated-code-lint` job in `.github/workflows/ci.yml` is a pre-P3 placeholder. It
asserts the generator is *still unimplemented* by grepping for `"not implemented yet"`
and deliberately fails otherwise. The generator has worked since commit `6fbfb8d`;
running `cargo run -p ruprizzle-cli -- generate` today produces a missing-schema error,
not that string. **This job therefore fails on every push.**

Worse, the real guarantee lives in two `#[ignore]`d tests in
`crates/codegen/tests/compile.rs` whose ignore reason reads `"(CI: --ignored)"` — but no
job passes `--ignored`. `ci.yml` runs plain `cargo test --workspace`. Only
`cargo xtask harden` runs them, and no workflow invokes it. The project's flagship
guarantee — that generated code compiles clean under `clippy::pedantic` — is enforced
solely by a human remembering a local command.

Verified: those two tests pass in 18.9s when actually run.

**Files:**
- Modify: `.github/workflows/ci.yml` (replace the `generated-code-lint` job)

**Interfaces:**
- Produces: a CI job named `generated code compiles clean` that runs the real gate.

- [ ] **Step 1: Replace the job**

In `.github/workflows/ci.yml`, delete the entire `generated-code-lint` job and replace
it with:

```yaml
  generated-code:
    name: generated code compiles clean
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          components: clippy
      - uses: Swatinem/rust-cache@v2
      # Our output is other people's source code. A warning in it is our bug and
      # must fail our build, not theirs.
      #
      # These tests generate all four example schemas across both dialects into
      # eight real crates and run cargo check plus clippy::pedantic over them.
      # They are #[ignore]d by default because they take ~20s; this is the job
      # that un-ignores them.
      - run: cargo test -p ruprizzle-codegen --test compile -- --include-ignored
```

- [ ] **Step 2: Verify locally**

Run: `cargo test -p ruprizzle-codegen --test compile -- --include-ignored`

Expected: `2 passed; 0 failed`, roughly 19 seconds.

- [ ] **Step 3: Add the harden job**

Append to `.github/workflows/ci.yml`:

```yaml
  harden:
    name: pre-release hardening
    runs-on: ubuntu-latest
    # Expensive (publish dry-runs, deny check, audits). Run on main and on
    # demand rather than on every PR.
    if: github.event_name == 'push' || github.event_name == 'workflow_dispatch'
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          components: clippy
      - uses: Swatinem/rust-cache@v2
      - uses: EmbarkStudios/cargo-deny-action@v2
      - run: cargo xtask harden
```

Add `workflow_dispatch:` to the `on:` block at the top of the file so it can be
triggered manually:

```yaml
on:
  push:
    branches: [main]
  pull_request:
  workflow_dispatch:
```

- [ ] **Step 4: Make the panic audit an actual gate**

`panic_audit` in `xtask/src/main.rs:166` prints every `unwrap()`, `expect()`, and
`panic!` it finds and then always returns `Ok(())`. It is presented as a hardening step
but can never fail the build, so wiring `xtask harden` into CI (Step 3) would add a job
that reports problems nobody is forced to read.

Give it a baseline it must not exceed. In `xtask/src/main.rs`, change `panic_audit` to
count findings and return them, and have the caller compare against a per-crate ceiling:

```rust
/// Per-crate ceiling for `unwrap()` / `expect()` / `panic!` in library source.
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
```

Have `run_harden` fail when a crate exceeds its budget, and print the offending lines.
Confirm the current counts first — they were measured for
[ProductionReadiness.md](ProductionReadiness.md) § 8.6 and must be re-derived rather than
trusted, since Phase A and B added code:

```bash
for d in crates/*/; do
  echo -n "$d "
  grep -rn "\.unwrap()\|\.expect(\|panic!\|todo!\|unimplemented!" "$d/src" --include=*.rs 2>/dev/null | wc -l
done
```

- [ ] **Step 5: Commit**

```bash
git add .github/workflows/ci.yml
git commit -m "ci: run the real generated-code gate, drop the pre-P3 stub

The generated-code-lint job asserted the generator was still
unimplemented and exited 1 otherwise. It has been implemented since
6fbfb8d, so that job failed on every push.

The actual guarantee lived in two #[ignore]d tests whose ignore reason
said '(CI: --ignored)' -- but no job passed --ignored. Wire them up, and
add a harden job so cargo xtask harden stops depending on someone
remembering it."
```

---

## PR-09 · Test on Windows and macOS, and test at MSRV

**Est:** 4h · **Severity:** MEDIUM

All seven CI jobs run on `ubuntu-latest`. The CLI is distributed by `cargo install` to
developers on all three platforms, this project's own primary development machine is
Windows, and the codebase does path manipulation, file watching via `notify`, and
subprocess invocation — the three classic sources of cross-platform breakage. The MSRV
job additionally runs only `cargo build`, so 1.85 is verified to compile but not to work.

**Files:**
- Modify: `.github/workflows/ci.yml` (`test` and `msrv` jobs)

- [ ] **Step 1: Make the test job a matrix**

Replace the `test` job's `runs-on` and add a matrix:

```yaml
  test:
    name: test (${{ matrix.os }})
    runs-on: ${{ matrix.os }}
    strategy:
      fail-fast: false
      matrix:
        os: [ubuntu-latest, windows-latest, macos-latest]
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - uses: Swatinem/rust-cache@v2
      # Postgres is unavailable here on purpose: this job proves the suite is
      # green without it, which is what a contributor without Docker
      # experiences, and it is the only shape that works on the Windows and
      # macOS runners. The `integration` job below is where the databases are
      # mandatory.
      - run: cargo test --workspace
```

- [ ] **Step 2: Make the MSRV job run tests**

In the `msrv` job, replace `- run: cargo build --workspace` with:

```yaml
      - run: cargo test --workspace
```

- [ ] **Step 3: Verify locally on this machine**

This is a Windows host, so the Windows leg can be checked directly:

Run the Verification Command.

Expected: green. Any failure here is a genuine cross-platform bug the matrix would have
caught — fix it as part of this task rather than deferring.

- [ ] **Step 4: Commit**

```bash
git add .github/workflows/ci.yml
git commit -m "ci: test on Windows and macOS, and run tests at MSRV

Every job ran on ubuntu-latest, yet the CLI installs on all three
platforms and the codebase does path handling, file watching, and
subprocess work -- the classic sources of cross-platform breakage.

The MSRV job also only built, so 1.85 was verified to compile but never
to work."
```

---

## PR-10 · Automate advisory and licence scanning

**Est:** 3h · **Severity:** MEDIUM

`deny.toml` is a well-constructed configuration — three target triples, an explicit
licence allowlist, a scoped `ring`/OpenSSL exception, `wildcards = "deny"`,
`unknown-git = "deny"`, `required-git-spec = "tag"`. It runs only from
`cargo xtask harden`, which no workflow calls, and even there it is skipped silently if
`cargo-deny` is absent. Across **335 resolved dependencies**, no advisory or licence
check runs automatically, and nothing keeps them current.

PR-08 already added `cargo-deny-action` to the harden job. This task adds a standalone
per-PR job and dependency automation.

**Files:**
- Modify: `.github/workflows/ci.yml`
- Create: `.github/dependabot.yml`

- [ ] **Step 1: Add a standalone deny job**

Append to `.github/workflows/ci.yml`:

```yaml
  deny:
    name: advisories and licences
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: EmbarkStudios/cargo-deny-action@v2
        with:
          command: check advisories licenses bans sources
```

- [ ] **Step 2: Add Dependabot**

Create `.github/dependabot.yml`:

```yaml
version: 2
updates:
  - package-ecosystem: cargo
    directory: "/"
    schedule:
      interval: weekly
    open-pull-requests-limit: 5
    groups:
      # One PR for the patch noise, individual PRs for anything that could
      # actually break us.
      patch-updates:
        update-types: ["patch"]

  - package-ecosystem: github-actions
    directory: "/"
    schedule:
      interval: monthly
```

- [ ] **Step 3: Verify the deny configuration passes today**

Run: `cargo deny check`

If `cargo-deny` is not installed: `cargo install cargo-deny --locked`

Expected: passes, or reports advisories that must be triaged before this task closes. The
config sets `unsound` and `yanked` to `warn` for the alpha; leave that as-is and record
any warnings in the commit message.

- [ ] **Step 4: Commit**

```bash
git add .github/workflows/ci.yml .github/dependabot.yml
git commit -m "ci: run cargo-deny on every PR and enable Dependabot

deny.toml was thoughtfully written and never executed: it ran only from
cargo xtask harden, which no workflow calls, and was skipped silently
when cargo-deny was absent. 335 dependencies with no automated advisory
or licence check, and nothing keeping them current."
```

---

## PR-11 · Add the missing governance documents

**Est:** 4h · **Severity:** MEDIUM

Absent: `SECURITY.md`, `CONTRIBUTING.md`, `CHANGELOG.md`, issue and PR templates.
`SECURITY.md` is the consequential one — a database library with no documented
disclosure path receives its first vulnerability report as a public GitHub issue.
`RELEASES.md` exists and is well written, but it is release notes, not a changelog.

**Files:**
- Create: `SECURITY.md`, `CONTRIBUTING.md`, `CHANGELOG.md`
- Modify: `README.md` (link them)

- [ ] **Step 1: Write `SECURITY.md`**

```markdown
# Security policy

## Supported versions

ruprizzle is pre-1.0. Only the most recent `0.x` release receives security
fixes.

| Version | Supported |
|---|---|
| 0.1.x   | ✅ |
| < 0.1   | ❌ |

## Reporting a vulnerability

**Do not open a public issue.**

Report privately through GitHub's
[private vulnerability reporting](https://github.com/ruprizzle/ruprizzle-orm/security/advisories/new),
or email the maintainer listed in `Cargo.toml`.

Expect an acknowledgement within 72 hours and an assessment within seven days.
If a fix is warranted we will agree a disclosure date with you, defaulting to
90 days or the release of the fix, whichever is sooner.

## Scope

In scope:

- SQL injection through any public API, including identifier handling and the
  `Value` binding path.
- Migration application that corrupts, loses, or silently alters data.
- Credential leakage through errors, logs, or generated code.
- Generated code that introduces a vulnerability into a consuming project.

Out of scope:

- Vulnerabilities in `sqlx` or other dependencies — report those upstream,
  though we appreciate a heads-up.
- Denial of service through deliberately pathological schemas fed to the CLI.
- Anything requiring an attacker who already controls the schema file, since
  that file is trusted input equivalent to source code.
```

- [ ] **Step 2: Write `CONTRIBUTING.md`**

Cover: the `cargo xtask ci` gate; that Postgres tests need `RUPRIZZLE_REQUIRE_DB=1` to
be meaningful; `docker compose up -d` or a local Postgres; that generated code must stay
`clippy::pedantic`-clean; that schema DSL changes need a `trybuild` case when they affect
compile-time guarantees; and that `ProjectPlan/ImplementationPlan/` is the design record.

- [ ] **Step 3: Write `CHANGELOG.md`**

Keep-a-Changelog format, seeded with an `## [Unreleased]` section listing every change
from this plan, and a `## [0.1.0-alpha.1]` section summarising `RELEASES.md`.

- [ ] **Step 4: Link from the README**

Add to the README's Development section:

```markdown
- [Contributing](CONTRIBUTING.md) — how to build, test, and what CI enforces
- [Security policy](SECURITY.md) — how to report a vulnerability
- [Changelog](CHANGELOG.md)
```

- [ ] **Step 5: Fix the stale status line**

`README.md:23` reads *"Phases P1–P7 are implemented and P8 … is the current focus"*, but
P8 shipped in `418475f`. Replace with an accurate status sentence.

- [ ] **Step 6: Commit**

```bash
git add SECURITY.md CONTRIBUTING.md CHANGELOG.md README.md
git commit -m "docs: add security policy, contributing guide, and changelog

A database library with no documented disclosure path receives its first
vulnerability report as a public issue. Also corrects the README status
line, which still described P8 as upcoming after it shipped."
```

---

# Phase D — Confidence and measurement

**Exit gate:** Performance is measured rather than assumed, the diff engine is verified
by generated cases rather than hand-written ones, and every remaining claim in the README
is either true or removed.

---

## PR-12 · Benchmark against a real database

**Est:** 3d · **Severity:** MEDIUM

The only benchmark is `crates/runtime/benches/query_construction.rs`, which measures
in-memory builder construction — the cheapest part of any ORM operation. Nothing measures
execution latency, throughput, `include` batching at scale, or memory per row.

This matters specifically because of the `sqlx::Any` design: every `Uuid`, `Decimal`,
`DateTime`, `Date`, `Time`, and `Json` is serialised to text outbound and parsed from
text inbound, on every row. That cost is real and unquantified.

`ImplPlan09TestingRelease.md` § P8-02 already specified this benchmark set with
acceptance thresholds — *"our overhead versus hand-written sqlx"* — and it was never
implemented. Use those thresholds.

**Files:**
- Create: `crates/runtime/benches/end_to_end.rs`
- Modify: `crates/runtime/Cargo.toml` (register the bench)

**Interfaces:**
- Consumes: `connect_with` and `PoolConfig` from PR-05.
- Produces: a criterion bench group `end_to_end` with the seven cases from P8-02.

- [ ] **Step 1: Register the benchmark**

In `crates/runtime/Cargo.toml`:

```toml
[[bench]]
name    = "end_to_end"
harness = false
```

- [ ] **Step 2: Write the benchmark**

Create `crates/runtime/benches/end_to_end.rs`:

```rust
//! End-to-end benchmarks against a real database.
//!
//! The interesting number is our overhead versus hand-written sqlx, not our
//! speed versus another ORM on different hardware. Every case therefore has a
//! hand-written comparison arm.
//!
//! Skipped when no database is reachable, so `cargo bench` still works offline.

use std::time::Duration;

use criterion::{Criterion, criterion_group, criterion_main};
use ruprizzle::{PoolConfig, connect_with};
use tokio::runtime::Runtime;

fn pg_url() -> Option<String> {
    std::env::var("RUPRIZZLE_TEST_PG_URL")
        .or_else(|_| std::env::var("DATABASE_URL"))
        .ok()
}

fn bench_end_to_end(c: &mut Criterion) {
    let Some(url) = pg_url() else {
        eprintln!("skipping end_to_end benches: no RUPRIZZLE_TEST_PG_URL");
        return;
    };

    let rt = Runtime::new().expect("tokio runtime");
    let config = PoolConfig {
        min_connections: 4,
        max_connections: 4,
        ..PoolConfig::default()
    };
    let pool = rt
        .block_on(connect_with(&url, &config))
        .expect("connect for benches");

    rt.block_on(async {
        sqlx::query("DROP TABLE IF EXISTS bench_rows")
            .execute(&pool)
            .await
            .expect("drop");
        sqlx::query(
            "CREATE TABLE bench_rows (id BIGINT PRIMARY KEY, name TEXT NOT NULL, n BIGINT NOT NULL)",
        )
        .execute(&pool)
        .await
        .expect("create");
        for i in 0..1_000i64 {
            sqlx::query("INSERT INTO bench_rows (id, name, n) VALUES ($1, $2, $3)")
                .bind(i)
                .bind(format!("row-{i}"))
                .bind(i * 2)
                .execute(&pool)
                .await
                .expect("seed");
        }
    });

    let mut group = c.benchmark_group("end_to_end");
    group.measurement_time(Duration::from_secs(10));

    group.bench_function("sqlx_single_row_by_pk", |b| {
        b.iter(|| {
            rt.block_on(async {
                let _: (i64, String, i64) =
                    sqlx::query_as("SELECT id, name, n FROM bench_rows WHERE id = $1")
                        .bind(500i64)
                        .fetch_one(&pool)
                        .await
                        .expect("fetch");
            });
        });
    });

    group.bench_function("sqlx_thousand_rows", |b| {
        b.iter(|| {
            rt.block_on(async {
                let rows: Vec<(i64, String, i64)> =
                    sqlx::query_as("SELECT id, name, n FROM bench_rows")
                        .fetch_all(&pool)
                        .await
                        .expect("fetch");
                assert_eq!(rows.len(), 1_000);
            });
        });
    });

    group.finish();
}

criterion_group!(benches, bench_end_to_end);
criterion_main!(benches);
```

- [ ] **Step 3: Add the ruprizzle-side arms**

For each `sqlx_*` case above, add the equivalent through the generated client so the two
appear side by side in criterion's output. This requires a generated module for the bench
schema; generate it into `crates/runtime/benches/generated/` via the codegen test
harness pattern in `crates/codegen/tests/compile.rs`, and `include!` it.

The comparison arms are the entire point — a benchmark of only the sqlx path measures
sqlx, not ruprizzle.

- [ ] **Step 4: Run and record**

Run:
```bash
RUPRIZZLE_TEST_PG_URL=postgres://ruprizzle:ruprizzle@localhost:5432/ruprizzle_test \
  cargo bench -p ruprizzle --bench end_to_end
```

Record the results in `docs/` against the P8-02 acceptance thresholds: single-row and
1 000-row within 5% of hand-written, two-level include within 15%, bulk insert within
10%.

- [ ] **Step 5: Publish the numbers**

Create `docs/performance.md` with the measured table, the hardware it was measured on,
and an explicit note on the text-marshalling cost of rich types through `sqlx::Any`.
Link it from the README **and add it to `docs/SUMMARY.md`** — the mdBook site is built
from that file, so a page missing from it never appears.

If any case exceeds its threshold, open an issue rather than silently widening the
threshold.

- [ ] **Step 6: Commit**

```bash
git add crates/runtime/benches/end_to_end.rs crates/runtime/Cargo.toml docs/performance.md README.md
git commit -m "bench: measure end-to-end cost against hand-written sqlx

The only existing benchmark measured in-memory builder construction --
the cheapest part of any ORM operation. Nothing measured execution,
which is where the sqlx::Any text-marshalling of Uuid, Decimal, and
DateTime actually costs something.

Implements the benchmark set specified in ImplPlan09 P8-02, which was
planned with thresholds and never built."
```

---

## PR-13 · Property-test the diff engine

**Est:** 3d · **Severity:** HIGH — highest-value test investment available

The diff engine has five hand-written tests. It is the component where an untested edge
case becomes lost data, and hand-written cases only cover the transitions someone thought
of.

`ImplPlan09TestingRelease.md` § P8-01 already specified *"Diff round-trip | `proptest` |
1 property, 256 cases"* in the test matrix. `proptest` appears nowhere in the workspace —
this line was never implemented.

The property: **for any pair of schemas `a` and `b`, applying `up_sql(a, b)` to a
database at `a` leaves it at `b`** — verified by `drift::detect` reporting nothing.

**Files:**
- Create: `crates/migrate/tests/roundtrip_prop.rs`
- Modify: `crates/migrate/Cargo.toml` (add `proptest` dev-dependency)

**Interfaces:**
- Consumes: `ruprizzle_migrate::{diff, up_sql, detect}` — `diff(prev: &Schema, next: &Schema) -> Vec<Change>`, `up_sql(prev: &Schema, next: &Schema, dialect: &dyn DbDialect) -> String`, `detect(pool: &AnyPool, schema: &Schema) -> Result<Vec<String>, Error>`. `ruprizzle_parser::parse(name: &str, src: &str) -> Result<Schema, _>`.
- Produces: a proptest strategy over schema mutations, plus two properties.

- [ ] **Step 1: Add the dependency**

In `crates/migrate/Cargo.toml` under `[dev-dependencies]`:

```toml
proptest = "1"
```

- [ ] **Step 2: Write the strategy and the pure property**

Create `crates/migrate/tests/roundtrip_prop.rs`:

```rust
//! Property tests for the diff engine.
//!
//! Hand-written diff tests only cover transitions someone thought of. These
//! generate the transitions instead. Schemas are built by rendering DSL text
//! and parsing it, which exercises the parser on the same path users take.

use proptest::prelude::*;
use ruprizzle_core::ir::Schema;
use ruprizzle_dialect::dialect_for;
use ruprizzle_migrate::{diff, up_sql};
use ruprizzle_parser::parse;

/// A field that may appear on the generated model.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Field {
    name: String,
    ty: &'static str,
    optional: bool,
    unique: bool,
}

fn field_strategy() -> impl Strategy<Value = Field> {
    (
        "[a-z][a-z0-9]{2,7}",
        prop::sample::select(vec!["String", "Int", "BigInt", "Boolean", "DateTime"]),
        any::<bool>(),
        any::<bool>(),
    )
        .prop_map(|(name, ty, optional, unique)| Field {
            name,
            ty,
            optional,
            unique,
        })
}

/// Renders a schema with the given fields on a single model.
fn render(fields: &[Field]) -> String {
    let mut s = String::from(
        "datasource db {\n  provider = \"postgres\"\n  url = \"postgres://x/y\"\n}\n\n\
         generator client {\n  provider = \"rust\"\n}\n\n\
         model Thing {\n  id Int @id\n",
    );
    for f in fields {
        s.push_str(&format!(
            "  {} {}{}{}\n",
            f.name,
            f.ty,
            if f.optional { "?" } else { "" },
            if f.unique { " @unique" } else { "" }
        ));
    }
    s.push_str("}\n");
    s
}

/// Parses, or returns `None` if the generated text was not valid (duplicate
/// field names are the expected cause, and are not interesting to the property).
fn schema_of(fields: &[Field]) -> Option<Schema> {
    parse("prop", &render(fields)).ok()
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// Diffing a schema against itself must produce no changes. A false
    /// positive here means `migrate dev` writes empty migrations forever.
    #[test]
    fn diff_with_self_is_empty(fields in prop::collection::vec(field_strategy(), 0..6)) {
        let Some(s) = schema_of(&fields) else { return Ok(()); };
        prop_assert!(diff(&s, &s).is_empty(), "self-diff produced changes");
    }

    /// Any change between two schemas must produce SQL. A silent empty diff
    /// between different schemas is how a column quietly never gets created.
    #[test]
    fn different_schemas_produce_sql(
        a in prop::collection::vec(field_strategy(), 0..5),
        b in prop::collection::vec(field_strategy(), 0..5),
    ) {
        let (Some(sa), Some(sb)) = (schema_of(&a), schema_of(&b)) else { return Ok(()); };
        let changes = diff(&sa, &sb);
        if !changes.is_empty() {
            let dialect = dialect_for(sa.datasource.provider);
            let sql = up_sql(&sa, &sb, dialect.as_ref());
            prop_assert!(
                !sql.trim().is_empty(),
                "diff reported {} changes but produced no SQL",
                changes.len()
            );
        }
    }
}
```

- [ ] **Step 3: Run to verify the properties hold or find a real bug**

Run: `cargo test -p ruprizzle-migrate --test roundtrip_prop`

Expected: PASS, or a shrunk counterexample. **A counterexample is a success for this
task** — capture it as a hand-written regression test in `crates/migrate/tests/diff.rs`,
fix the engine, and note both in the commit.

- [ ] **Step 4: Add the database-backed round-trip property**

This is the property that makes `migrate dev` trustworthy: applying `up_sql(a, b)` to a
database sitting at `a` must leave it at `b`, as judged by the drift detector.

`proptest` drives a synchronous closure, so the runtime is built inside the property and
the case count is dropped to 32 — each case is two round trips to a real database, and
256 would dominate CI time.

Append to `crates/migrate/tests/roundtrip_prop.rs`:

```rust
use ruprizzle_migrate::detect;

/// Builds an empty-to-`schema` migration and applies it, then diffs to `target`
/// and applies that. The database must then report no drift against `target`.
async fn round_trip(url: &str, from: &Schema, to: &Schema) -> Result<Vec<String>, String> {
    let pool = ruprizzle::connect(url).await.map_err(|e| e.to_string())?;
    let dialect = dialect_for(from.datasource.provider);

    // Isolate each case: drop anything a previous case left behind.
    sqlx::query("DROP TABLE IF EXISTS \"Thing\"")
        .execute(&pool)
        .await
        .map_err(|e| e.to_string())?;

    let empty = parse("empty", &render(&[])).map_err(|_| "parse empty".to_owned())?;
    for sql in [
        up_sql(&empty, from, dialect.as_ref()),
        up_sql(from, to, dialect.as_ref()),
    ] {
        for stmt in ruprizzle_migrate::runner::split_statements(&sql) {
            sqlx::query(&stmt)
                .execute(&pool)
                .await
                .map_err(|e| format!("{stmt}: {e}"))?;
        }
    }

    detect(&pool, to).await.map_err(|e| e.to_string())
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(32))]

    /// Applying the generated migration must actually reach the target schema.
    #[test]
    fn applied_diff_reaches_the_target_schema(
        a in prop::collection::vec(field_strategy(), 0..4),
        b in prop::collection::vec(field_strategy(), 0..4),
    ) {
        let Ok(url) = std::env::var("RUPRIZZLE_TEST_PG_URL") else { return Ok(()); };
        let (Some(sa), Some(sb)) = (schema_of(&a), schema_of(&b)) else { return Ok(()); };

        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("runtime");

        match rt.block_on(round_trip(&url, &sa, &sb)) {
            Ok(drift) => prop_assert!(
                drift.is_empty(),
                "after applying the diff, drift remains: {drift:?}"
            ),
            Err(e) => prop_assert!(false, "round trip failed: {e}"),
        }
    }
}
```

Add `tokio` and `ruprizzle` to `crates/migrate/Cargo.toml` under `[dev-dependencies]`:

```toml
tokio     = { workspace = true }
ruprizzle = { workspace = true }
```

Run:
```bash
RUPRIZZLE_TEST_PG_URL=postgres://ruprizzle:ruprizzle@localhost:5432/ruprizzle_test \
  cargo test -p ruprizzle-migrate --test roundtrip_prop
```

Expected: PASS, or a shrunk counterexample — which, as in Step 3, is a success. Capture
it as a regression test and fix the engine.

- [ ] **Step 5: Run the full gate and commit**

```bash
git add crates/migrate/tests/roundtrip_prop.rs crates/migrate/Cargo.toml
git commit -m "test(migrate): property-test the diff engine

The diff engine had five hand-written tests and is the component where
an untested edge case becomes lost data. Hand-written cases only cover
transitions someone thought of.

Implements the proptest round-trip specified in ImplPlan09 P8-01, which
was in the test matrix from the start and never built."
```

---

## PR-14 · Test migration application under real concurrency

**Est:** 2d · **Severity:** MEDIUM

PR-02 made `apply_all` idempotent and proved it with a sequential simulation. The
guarantee that matters is under genuine interleaving.

**Files:**
- Modify: `crates/migrate/tests/concurrency.rs` (append)

**Interfaces:**
- Consumes: the idempotent `apply_all` from PR-02.

- [ ] **Step 1: Write the failing test**

Append to `crates/migrate/tests/concurrency.rs`:

```rust
/// Ten deployers racing on the same directory: exactly one applies each
/// migration, none error, and the schema ends up correct.
#[tokio::test]
async fn ten_concurrent_deployers_all_succeed() {
    let Some(url) = std::env::var("RUPRIZZLE_TEST_PG_URL").ok() else {
        if std::env::var("RUPRIZZLE_REQUIRE_DB").is_ok() {
            panic!("RUPRIZZLE_REQUIRE_DB is set but RUPRIZZLE_TEST_PG_URL is not");
        }
        eprintln!("skipping: no RUPRIZZLE_TEST_PG_URL");
        return;
    };

    let dir = fixture();
    let pool = ruprizzle::connect(&url).await.expect("connect");

    sqlx::query("DROP TABLE IF EXISTS conc_a, conc_b, _ruprizzle_migrations")
        .execute(&pool)
        .await
        .expect("clean slate");

    let mut handles = Vec::new();
    for _ in 0..10 {
        let path = dir.path().to_path_buf();
        let pool = pool.clone();
        handles.push(tokio::spawn(async move {
            Migrator::new(path).apply_all(&pool, false).await
        }));
    }

    let mut total_applied = 0;
    for h in handles {
        let report = h.await.expect("task panicked").expect("apply must not error");
        total_applied += report.applied.len();
    }

    assert_eq!(
        total_applied, 2,
        "each migration must be applied exactly once across all deployers"
    );
}
```

- [ ] **Step 2: Run it**

Run:
```bash
RUPRIZZLE_REQUIRE_DB=1 \
  RUPRIZZLE_TEST_PG_URL=postgres://ruprizzle:ruprizzle@localhost:5432/ruprizzle_test \
  cargo test -p ruprizzle-migrate --test concurrency -- --nocapture
```

Expected: PASS. If it fails, PR-02's re-check has a gap — the likely cause is
`ensure_table` racing, since `CREATE TABLE IF NOT EXISTS` is not atomic against a
concurrent creator on Postgres. Fix by retrying `ensure_table` once on a duplicate-object
error, and record that in the commit.

- [ ] **Step 3: Commit**

```bash
git add crates/migrate/tests/concurrency.rs
git commit -m "test(migrate): prove apply_all is safe under real interleaving

PR-02 proved idempotency with a sequential simulation. Ten racing
deployers is the case that actually happens during a rolling deploy."
```

---

## PR-15 · Resolve the `raw!` promise

**Est:** 1d · **Severity:** MEDIUM — a published claim that is currently untrue

`crates/macros/src/lib.rs` is sixteen lines containing one private
`fn placeholder_until_p4()`. Its own doc comment advertises *"the injection-safe `raw!`
fragment builder"*, `MasterPlan.md` lists the crate as shipping to users, and
`README.md:135` promises *"the ability to drop down to raw SQL without leaving the query
builder"*. None of that exists. Publishing an empty crate that claims this functionality
is a promise that must be honoured or retracted.

Decide, then execute. Recommendation: **implement `raw!`** — the escape hatch is one of
the six non-negotiable design principles in `MasterPlan.md`, and `Tx::execute` with a
separate `Vec<Value>` is not the ergonomic equivalent.

- [ ] **Step 1: Decide**

Choose one:
- **(a) Implement `raw!`** — a macro taking a SQL fragment with `{}` interpolation points that expand to binds, never to string interpolation.
- **(b) Retract** — delete `ruprizzle-macros` from the workspace and the publish list, remove the claim from `README.md:135` and `MasterPlan.md`.

- [ ] **Step 2a: If implementing, write the failing test**

Create `crates/runtime/tests/raw_macro.rs`:

```rust
use ruprizzle::raw;

#[test]
fn raw_binds_rather_than_interpolates() {
    let email = "'; DROP TABLE users; --";
    let fragment = raw!("email = {}", email);
    assert_eq!(fragment.sql(), "email = $1");
    assert_eq!(fragment.binds().len(), 1);
    assert!(
        !fragment.sql().contains("DROP"),
        "raw! interpolated user data into SQL"
    );
}
```

- [ ] **Step 3a: Implement the macro**

In `crates/macros/src/lib.rs`, replace the placeholder with a `proc_macro` that expands
each `{}` to a placeholder and pushes the corresponding expression onto a bind vector.
Add a matching `RawFragment` type in `crates/runtime/src/filter.rs` exposing `sql()` and
`binds()`, and accept it in the filter tree.

Add a `trybuild` compile-fail case proving a non-`Encodable` argument is rejected at
compile time.

- [ ] **Step 2b: If retracting, remove the crate**

Delete `crates/macros`, remove it from `Cargo.toml` workspace dependencies, remove the
dependency from `crates/runtime/Cargo.toml`, remove it from the `xtask release` package
list, and correct `README.md:135` and the MasterPlan crate table.

- [ ] **Step 4: Run the full gate and commit**

```bash
git commit -m "feat(macros): implement the raw! escape hatch"
# or
git commit -m "chore: remove the unimplemented ruprizzle-macros crate"
```

---

## PR-16 · Record the `sqlx::Any` decision as an ADR

**Est:** 1d · **Severity:** MEDIUM

Every query runs through `sqlx::Any`, the type-erased driver. That buys one identical
Rust API across Postgres and SQLite with the dialect chosen by URL at runtime — the
product's core promise. It costs: per-row text serialisation of `Uuid`, `Decimal`,
`DateTime`, `Date`, `Time`, and `Json` in both directions; reliance on server-side type
inference for index-eligible comparisons; timezone and format fragility for `DateTime`;
and Postgres arrays being rejected at runtime (`value.rs:204`).

The abstraction is leaking repeatedly in the same place: three of the last four commits
before this plan were fixes in exactly this layer. That pattern is worth recording before
users depend on runtime dialect selection and reversing becomes painful.

**Files:**
- Modify: `ProjectPlan/ImplementationPlan/ImplPlan10AppendixDecisions.md`
- Modify: `docs/known-limitations.md`

- [ ] **Step 1: Write the ADR**

Append to the ADR list, following the existing numbering and format:

```markdown
## ADR-0NN · Runtime dialect selection via `sqlx::Any`

**Status:** Accepted, with costs recorded.

**Context.** The product promises one identical Rust API across Postgres and
SQLite, with the backend chosen by URL scheme at runtime. `sqlx::Any` is the
only sqlx facility that provides this without generating a separate client per
dialect.

**Decision.** Route all runtime queries through `sqlx::Any`.

**Consequences.**

- `Any` implements neither `Encode` nor `Decode` for rich types, so `Uuid`,
  `Decimal`, `DateTime`, `Date`, `Time`, and `Json` are serialised to text
  outbound (`value.rs:158`) and parsed from text inbound (`decode.rs:33`), on
  every row.
- Comparisons on rich-typed columns rely on server-side parameter inference. If
  inference resolves to `text` rather than the column type, the comparison stops
  using the index — a silent performance cliff, not an error.
- `DateTime` correctness depends on server `DateStyle` and session timezone.
- Postgres arrays, `LISTEN`/`NOTIFY`, `COPY`, and composite types are
  unreachable. Array binds are rejected at runtime.
- The abstraction has leaked repeatedly: commits 1bfb512 and e737708 are both
  fixes in this layer.

**Revisit when** any of: a third dialect is added; benchmarks (PR-12) show the
text round-trip exceeding the P8-02 thresholds; or users need Postgres arrays.
The exit is generating dialect-specific native code paths behind a feature flag,
which is additive but costs the runtime-selection property.
```

- [ ] **Step 2: Surface the costs to users**

Add to `docs/known-limitations.md` under *Current alpha*:

```markdown
- **Postgres arrays** cannot be used as bind values. `Value::Array` is rejected
  at runtime.
- **Rich types round-trip as text.** `Uuid`, `Decimal`, `DateTime`, `Date`,
  `Time`, and `Json` are sent and received as text because the underlying
  `sqlx::Any` driver does not encode them natively. See ADR-0NN.
```

- [ ] **Step 3: Commit**

```bash
git add ProjectPlan/ImplementationPlan/ImplPlan10AppendixDecisions.md docs/known-limitations.md
git commit -m "docs: record the sqlx::Any trade-off as an ADR

Three of the last four commits before this plan were fixes in the Any
marshalling layer. That is a pattern worth recording before users depend
on runtime dialect selection and reversing gets expensive."
```

---

# Schedule and exit criteria

| Phase | Tasks | Effort | Cumulative | Unblocks |
|---|---|---|---|---|
| **A — Correctness** | PR-01 … PR-03 | ~1.5 days | Week 1 | Publishing to crates.io |
| **B — Operability** | PR-04 … PR-07 | ~5 days | Week 2–3 | Production, non-critical data |
| **C — CI** | PR-08 … PR-11 | ~2 days | Week 4 | Trustworthy green builds |
| **D — Confidence** | PR-12 … PR-16 | ~10 days | Week 5–6 | Production, critical data |

**Total: 5–6 weeks for one experienced Rust developer.**

## Exit criteria

- [ ] All 16 tasks complete, each committed separately.
- [ ] `cargo xtask harden` passes end to end, in CI, not just locally.
- [ ] CI green on Linux, Windows, and macOS.
- [ ] `cargo deny check` clean, or every warning triaged and recorded.
- [ ] Benchmarks published against the P8-02 thresholds.
- [ ] `docs/known-limitations.md` no longer lists query logging or pool metrics as deferred.
- [ ] Test count ≥ 220 (from a baseline of 167).
- [ ] `ProductionReadiness.md` re-run, scoring ≥ 85/100.

## Deliberately out of scope

These are real gaps that this plan does not close, listed so the omission is a decision
rather than an oversight:

- **Compile-time query checking** (`sqlx-data.json` / offline mode) — a 0.2 feature, and the README already labels it "planned" rather than shipped.
- **LSP** — deferred to 0.2; the TextMate grammar covers highlighting.
- **MySQL and MSSQL dialects** — additive behind `DbDialect`, and blocked on the ADR-0NN decision.
- **Migration squashing** and **heuristic rename detection** — documented limitations with a documented workaround (`@renamedFrom`).
- **Mutual foreign-key cycles in migrations** — documented; broken by hand across migrations.
- **Savepoints / nested transactions** — a genuine gap, but no user has asked and the flat `Tx` is correct as far as it goes.
- **The 29 `unwrap()`/`expect()` calls in `crates/parser/src`** — they sit where Pest guarantees the invariant, and rewriting them would add error paths that cannot be hit. PR-08 Step 4 freezes the count instead, so it can only fall.
- **The throwaway `pool.acquire()` in `apply_all` used only to read `backend_name()`** (`runner.rs:207`) — one pooled acquire per `apply_all` call, not per migration. Removing it means detecting the backend from connect options, which is more API surface than the saving justifies.
- **`Value::Array` being dead defensive code** (`value.rs:204`) — unreachable today because the `IN` compiler expands to individual binds. PR-16 documents the array limitation rather than removing the variant, since native array support is the eventual fix.
