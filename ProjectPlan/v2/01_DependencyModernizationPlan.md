# Plan 01: Public Dependency Modernization (sqlx 0.9, rusqlite 0.40, MSRV 1.86)

**Date:** 2026-08-22  
**Author:** Vaibhav Gupta <vaibhavgupta9877@gmail.com>  
**Status:** Ready for Execution  
**Milestone:** v2.0.0 (Major Breaking Milestone)  
**Primary Crates:** `crates/runtime`, `crates/migrate`, `crates/cli`, `crates/dialect`, `crates/testkit`  
**Dependencies Baseline:** `sqlx 0.9.0`, `rusqlite 0.40.0`, Rust 1.86.0

---

## 1. Context, Objectives & Scope

In `ruprizzle-orm` 1.0, `crates/runtime/src/lib.rs` exports `pub use sqlx;` and `sqlite-rusqlite` re-exports `rusqlite::Row` and `rusqlite::types`. Because these are part of ruprizzle's public API surface, major upgrades to upstream drivers (`sqlx 0.8 -> 0.9`, `rusqlite 0.32 -> 0.40`) are breaking changes reserved strictly for the **v2.0 major milestone**.

By executing this modernization in **v2.0**:
1. We upgrade to `sqlx 0.9.0` and `rusqlite 0.40.0` across all executor and migration paths.
2. We raise the workspace Minimum Supported Rust Version (MSRV) to **1.86** (required by sqlx 0.9).
3. We eliminate the `RUSTSEC-2023-0071` security advisory exception in `deny.toml` by utilizing sqlx 0.9's decoupled `mysql-rsa` feature flags.

---

## 2. Technical Architecture & Breaking Changes Analysis

### 2.1 `sqlx 0.9` Changes & Solutions

#### A. `SqlSafeStr` and `AssertSqlSafe` on Dynamic Query Sites
- **Upstream Change:** `sqlx 0.9` enforces query string safety to mitigate SQL injection. `sqlx::query(sql)` now requires `sql: impl SqlSafeStr`. Dynamic strings must be wrapped in `sqlx::AssertSqlSafe(sql)`.
- **Ruprizzle Adaptation:** Ruprizzle generates SQL via its dialect AST and compiler. All query dispatch locations (~133 sites across ~25 files) will wrap compiled SQL strings in `sqlx::AssertSqlSafe` or use helper wrappers in `crates/runtime/src/executor.rs`.
- **Code Pattern:**
  ```rust
  // Old (sqlx 0.8):
  sqlx::query(&compiled_sql)
  
  // New (sqlx 0.9):
  sqlx::query(sqlx::AssertSqlSafe(&compiled_sql))
  ```

#### B. `SqliteValue` (`!Sync`) and `SqliteValueRef` (`!Send`) Bounds
- **Upstream Change:** In sqlx 0.9, SQLite value types tightened thread-safety markers to match SQLite C-API internals.
- **Ruprizzle Adaptation:**
  - Audit `crates/runtime/src/decode.rs` and `crates/runtime/src/rusqlite.rs`.
  - Ensure intermediate decoded buffers extract owned values (`Value`) before crossing `.await` suspension points.
  - Retain `Send + 'static` bounds on all public `Executor` boxed future return types (`BoxFuture<'static, Result<...>>`).

#### C. `AnyArguments` Lifetime Removals
- **Upstream Change:** Lifetimes were removed from `AnyArguments` and the `Arguments` trait in sqlx 0.9.
- **Ruprizzle Adaptation:** Update generic bounds on `crates/runtime/src/executor.rs`, `query.rs`, and dialect execution adapters.

#### D. MySQL Text/Blob to `AnyTypeInfo` Conversions
- **Upstream Change:** MySQL type metadata normalization changed for text and blob types.
- **Ruprizzle Adaptation:** Update MySQL type decoding assertions in `crates/runtime/src/decode.rs` and verify against `tests/integration/tests/dialect_conformance.rs`.

---

### 2.2 `rusqlite 0.40` Upgrade

- Update `rusqlite` from `0.32` to `0.40` in `crates/runtime/Cargo.toml` and workspace root `Cargo.toml`.
- Verify `crates/runtime/src/rusqlite.rs` compatibility with updated `Statement`, `Row`, and `ToSql` trait signatures.

---

### 2.3 `deny.toml` Security Advisory Cleanup

- `RUSTSEC-2023-0071` (vulnerability in `rsa` crate) was previously ignored because `sqlx-mysql 0.8` pulled in `rsa` unconditionally for `caching_sha2_password`.
- In `sqlx 0.9`, this is moved behind an optional `mysql-rsa` feature.
- By not enabling `mysql-rsa` (or using modern native crypto), `rsa` is pruned from the dependency graph.
- Remove `RUSTSEC-2023-0071` from `deny.toml` ignore list and verify `cargo deny check advisories`.

---

## 3. Step-by-Step Implementation Tasks

### Task 1: Update Workspace Dependency Manifests & MSRV
- [ ] Update `Cargo.toml` (root):
  - Set `rust-version = "1.86"`.
  - Update `sqlx` workspace dependency to `"0.9.0"` with features `["runtime-tokio", "postgres", "sqlite", "mysql", "chrono", "uuid", "json", "bigdecimal"]`.
  - Update `rusqlite` workspace dependency to `"0.40.0"`.
- [ ] Update all child crate `Cargo.toml` files (`rust-version = "1.86"`).

### Task 2: Refactor `crates/runtime` for `sqlx 0.9` & `rusqlite 0.40`
- [ ] In `crates/runtime/src/executor.rs`:
  - Introduce `safe_sql(sql: &str) -> sqlx::AssertSqlSafe<&str>` internal helper.
  - Update `fetch_all`, `fetch_optional`, `fetch_one`, `execute` to wrap dynamic SQL.
- [ ] In `crates/runtime/src/query.rs` and `crates/runtime/src/compile.rs`:
  - Update query execution and `.to_sql()` bind dispatch.
- [ ] In `crates/runtime/src/decode.rs`:
  - Update decoding for SQLite and MySQL type infos.
- [ ] In `crates/runtime/src/rusqlite.rs`:
  - Update `rusqlite 0.40` API calls, row indexing, and transaction wrappers.

### Task 3: Refactor `crates/migrate` & `crates/cli`
- [ ] Update migration runner and introspectors in `crates/migrate/src/runner.rs` and `introspect.rs`.
- [ ] Update `crates/cli/src/main.rs` database command executors (`db push`, `migrate dev`, `seed`).

### Task 4: Clean Up Security Advisories in `deny.toml`
- [ ] Edit `deny.toml` to remove `RUSTSEC-2023-0071` from `ignore`.
- [ ] Run `cargo deny check advisories` and verify zero vulnerabilities.

---

## 4. Verification & Testing Strategy

```powershell
# 1. Workspace compilation
cargo check --workspace --all-targets

# 2. Format & Clippy
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings

# 3. Unit and integration tests
cargo test --workspace
$env:RUPRIZZLE_TEST_RUSQLITE=1; cargo test -p ruprizzle --features "sqlite-rusqlite,ruprizzle-testkit/sqlite-rusqlite"

# 4. Security & hardening
cargo deny check advisories
cargo xtask harden
```

---

## 5. Definition of Done

1. Root workspace and all 10 member crates declare `rust-version = "1.86"`.
2. Workspace builds against `sqlx 0.9.0` and `rusqlite 0.40.0` with zero compiler errors or warnings.
3. All unit, integration, and conformance tests pass.
4. `cargo deny check advisories` passes with no ignored advisory entries for `rsa`.
