# Production Readiness Assessment — ruprizzle-orm

**Version assessed:** `0.1.0-alpha.1` (commit `e737708`)
**Date:** 2026-08-10
**Assessor:** Claude Code (static analysis + live build, lint, and dual-database test execution)
**Scope:** The ORM workspace only. No auth, RPC, UI, or reference application is in this repo.

---

## 1. Verdict

| Axis | Score | Grade |
|---|---|---|
| **Production readiness** | **52 / 100** | **D+ — Not production ready** |
| Engineering craft | 78 / 100 | B+ — Well above alpha norms |

These two numbers are deliberately separate, because they say different things and
conflating them would mislead.

**The craftsmanship is genuinely good.** Zero `unsafe` across the entire workspace,
parameterised SQL by construction rather than by convention, a clean eight-crate
separation that keeps the parser and codegen out of the consumer dependency graph,
typed error taxonomy, 167 tests green against a live Postgres 17 *and* SQLite, and
generated code that is verified to compile clean under `clippy::pedantic`. This is
not a weekend project with a README.

**It is nonetheless not deployable to production**, for three reasons that are
structural rather than cosmetic:

1. **It is operationally blind.** There is not one line of `tracing`, logging, or
   metrics in the runtime or migration engine. You cannot see a slow query, correlate
   a query to a request, or observe pool saturation. You would be running a database
   layer you cannot debug in situ.
2. **The connection pool cannot be tuned.** `connect(url)` takes a URL and nothing
   else — no max connections, no acquire timeout, no idle or max lifetime. Under real
   load you have no lever to pull.
3. **The migration path — the one component where a bug is unrecoverable — has
   confirmed defects.** Two were found and empirically reproduced during this
   assessment (§5.1, §5.2), including silent UTF-8 corruption of migration SQL.

The project's own `RELEASES.md` states plainly: *"What we do not claim: production
readiness."* That self-assessment is accurate, and the honesty is itself a positive
signal. This document quantifies the distance to that goal rather than disputing it.

---

## 2. Scorecard by dimension

| # | Dimension | Weight | Score | Rationale |
|---|---|---|---|---|
| 1 | Correctness & testing | 20% | 7.0 | 167 tests green on real Postgres + SQLite; snapshot, conformance, and `trybuild` compile-fail coverage. But two real defects surfaced within minutes of targeted probing; no property tests, no fuzzing, no concurrency tests. |
| 2 | Security | 15% | 7.5 | Parameterised binding is architecturally enforced; identifier quoting escapes correctly; automated injection audit; `forbid(unsafe_code)` in all 8 crates. But `cargo-deny` never runs in CI, no `SECURITY.md`, and error messages can echo user data into logs. |
| 3 | Operability & observability | 15% | 2.5 | No tracing, no query logging, no slow-query detection, no pool metrics, no health check. Untunable pool. This is the single largest gap. |
| 4 | Data safety & migrations | 15% | 6.5 | SHA-256 checksum verification, per-migration transaction, Postgres advisory lock, destructive-change gating, drift detection — a strong design undermined by the splitter defects and a lock ordering race. |
| 5 | Architecture & design | 10% | 8.0 | Excellent crate boundaries and separation of concerns. Marked down for the `sqlx::Any` compromise (§5.3), which is load-bearing and hard to reverse. |
| 6 | CI/CD & release engineering | 10% | 5.0 | Good seven-job matrix in principle, but one job is a stale stub that now fails, Linux-only, and the strongest gate (`xtask harden`) is not wired into CI at all. |
| 7 | Documentation | 5% | 8.0 | 915 lines of task-oriented guides, an honest limitations page, `missing_docs` and `RUSTDOCFLAGS=-D warnings` enforced. Missing `CONTRIBUTING`, `SECURITY`, `CHANGELOG`. |
| 8 | API stability & semver | 5% | 5.0 | Alpha by declaration. Public error enums lack `#[non_exhaustive]`, so any new variant is a breaking change. |
| 9 | Performance | 5% | 4.0 | One microbenchmark of query *construction*. No end-to-end throughput, latency, or memory data at all. Rich types round-trip through text on every row. |

**Weighted total: 6.25 / 10 on craft dimensions.** Adjusted down to **5.2 / 10 (52/100)**
for production readiness, because observability and the migration defects are
*blocking* rather than merely weighting factors — no amount of strength elsewhere
compensates for a database layer you cannot observe applying migrations you cannot
fully trust.

---

## 3. Verification performed

Everything below was executed against this working tree, not inferred from the source.

| Check | Command | Result |
|---|---|---|
| Formatting | `cargo fmt --all --check` | ✅ Clean |
| Lint | `cargo clippy --workspace --all-targets -- -D warnings` | ✅ Zero warnings |
| Full suite, Postgres **mandatory** | `RUPRIZZLE_REQUIRE_DB=1 RUPRIZZLE_TEST_PG_URL=… cargo test --workspace` | ✅ **167 passed, 0 failed** across 27 binaries |
| Generated-code compile gate | `cargo test -p ruprizzle-codegen --test compile -- --ignored` | ✅ 2 passed (8 generated crates, both dialects, `clippy::pedantic` clean) — **18.9 s** |
| CLI generator liveness | `cargo run -p ruprizzle-cli -- generate` | ⚠️ Runs (fails only for a missing schema file) — which means CI job `generated-code-lint` now **fails**, see §6.1 |
| Migration splitter — UTF-8 | Ad-hoc probe test | ❌ **FAIL** — see §5.1 |
| Migration splitter — dollar quoting | Ad-hoc probe test | ❌ **FAIL** — see §5.2 |

**Codebase size:** 14,440 lines of source across 8 crates + xtask; 3,823 lines of test
code (a 3.8 : 1 source-to-test ratio, which is healthy); 83-line Pest grammar;
335 resolved dependencies.

**Test distribution:** parser 20, dialect 23, core 18, migrate 31, runtime 20,
integration 73, codegen 3 (+2 gated), CLI 3, doctests 2.

---

## 4. What is genuinely strong

These are load-bearing strengths, not participation trophies.

**SQL injection is prevented by construction, not by discipline.** `compile.rs` has no
code path that interpolates a runtime value into a SQL string — every value goes
through `push_bind`, which appends a dialect-correct placeholder and pushes onto a
bind vector. Identifier quoting escapes embedded quotes properly
(`crates/dialect/src/postgres.rs:23`). And `cargo xtask harden` runs an automated
injection audit that greps for `format!`-built SQL, so a regression is caught
mechanically rather than at review time. This is the correct layering: make the unsafe
thing unexpressible, then audit for the exception.

**Zero unsafe code, enforced.** All eight crates carry `#![forbid(unsafe_code)]`. The
only two places where `unsafe` would normally appear — pinned future projections in
`query.rs:310` and `tx.rs:295` — were solved by requiring `Unpin` instead, with a
comment explaining why. That is the right trade and it was made deliberately.

**The test harness is honest about skipping.** The dual-database harness skips Postgres
when unreachable so contributors without Docker get a green build, but
`RUPRIZZLE_REQUIRE_DB=1` promotes that skip to a hard failure, and CI sets it. This is
the difference between a suite that *is* green and one that only *reports* green — a
distinction most projects get wrong.

**The generated code is held to a higher standard than the generator.** The codegen
compile test materialises all four example schemas across both dialects into eight real
crates and runs `cargo check` plus `clippy::pedantic` over them. The reasoning in
`ci.yml` is exactly right: *"Our output is other people's source code. A warning in it
is our bug and must fail our build, not theirs."* I verified this passes.

**Migration engine fundamentals are sound.** SHA-256 checksums are verified against the
tracking table before anything is applied, each migration runs in its own transaction,
Postgres deployments take a transaction-scoped advisory lock, and destructive changes
are blocked behind an explicit `accept_data_loss` flag. Drift detection exists.
`migrate dev` and `migrate deploy` are separate commands specifically so a prototyping
habit cannot carry into CI — good product thinking.

**Error taxonomy is production-grade in shape.** `UniqueViolation`, `ForeignKeyViolation`,
`NotNullViolation`, `CheckViolation`, `Deadlock`, and `SerializationFailure` are
distinct typed variants rather than a stringly-typed blob, and `is_retryable()` plus
`IsolationLevel::Serializable` give callers what they need to build correct retry loops.

**Documentation is unusually honest.** `docs/known-limitations.md` opens with *"It is a
feature, not an apology"* and then lists real limitations, including a *"When to choose
something else"* section that recommends competitors. Projects that document their
boundaries this candidly are usually the ones whose claims you can trust.

---

## 5. Blockers — must be resolved before production

### 5.1 Migration SQL splitter silently corrupts non-ASCII text — **CRITICAL**

`split_statements` in `crates/migrate/src/runner.rs:500` walks the SQL as raw bytes and
casts each byte to a `char`:

```rust
c => {
    current.push(c as char);   // byte → char: Latin-1, not UTF-8
    i += 1;
}
```

`u8 as char` performs a Latin-1 widening, not UTF-8 decoding. Every multi-byte UTF-8
sequence is split into its constituent bytes and re-encoded as separate characters.

**Reproduced:**

```
input:  INSERT INTO t (name) VALUES ('café');
output: INSERT INTO t (name) VALUES ('cafÃ©')
```

**Impact:** Any migration containing a non-ASCII character — seed data, a localised
default value, an accented string in a `CHECK` constraint, a comment — is silently
mojibake'd before being sent to the database. It does not error. It does not warn. The
corrupted value is committed, the migration is recorded as successfully applied with a
valid checksum, and the damage is discovered later in production data. Because
`down.sql` reverses the *intended* statement rather than the corrupted one, rollback
does not reliably clean it up.

This is the most serious finding in this assessment: it is a silent data-corruption bug
on the one code path where silent failure is least acceptable.

**Fix:** Iterate over `sql.chars()` (or index by `char_indices`) instead of `as_bytes()`.
The ASCII-only delimiters being scanned for (`'`, `-`, `/`, `*`, `;`) cannot appear as
UTF-8 continuation bytes, so the change is mechanical and safe. Add a regression test
with a non-ASCII literal.

### 5.2 Migration SQL splitter breaks Postgres dollar-quoted bodies — **HIGH**

The splitter handles `'…'` literals, `--` comments, and `/* … */` blocks, but not
`$$ … $$` or `$tag$ … $tag$` dollar quoting.

**Reproduced:**

```
input:  CREATE FUNCTION f() RETURNS trigger AS $$ BEGIN RETURN NEW; END; $$ LANGUAGE plpgsql;
output: 3 statements —
        ["CREATE FUNCTION f() RETURNS trigger AS $$ BEGIN RETURN NEW",
         "END",
         "$$ LANGUAGE plpgsql"]
```

**Impact:** Triggers, stored procedures, and `plpgsql` functions cannot be expressed in
a migration. This matters more than it first appears, because `docs/migrations-guide.md`
explicitly documents hand-editing migrations for backfills — the exact workflow where
users reach for procedural SQL. The failure is at least loud (the fragments are invalid
SQL and the transaction aborts), so this is a blocker for capability, not for safety.

**Fix:** Add dollar-quote state to the scanner: on `$`, attempt to match `$[A-Za-z_0-9]*$`,
and if it matches, consume verbatim until the identical closing tag.

### 5.3 No observability whatsoever — **CRITICAL for operations**

A workspace-wide search for `tracing::`, `log::`, `println!`, and `eprintln!` across
`crates/runtime/src` and `crates/migrate/src` returns **zero results**. The runtime has
no dependency on `tracing` or `log`.

**Consequence:** In production you cannot answer *"which query is slow?"*, *"which
request issued this query?"*, *"is the pool saturated?"*, or *"how long did that
migration actually take?"*. Every mature Rust database layer (sqlx, SeaORM, Diesel's
instrumentation hooks) emits structured spans; consumers depend on that for APM
integration. `.to_sql()` gives you the SQL at development time, which is a genuinely
good debugging affordance, but it is not a substitute for runtime telemetry.

`docs/known-limitations.md` defers *"connection pool metrics and query logging"* to
0.2, so this is a known and accepted gap — but it is precisely the gap that separates
alpha from production.

**Fix:** Add `tracing` as an optional (default-on) dependency. Emit a span per query
carrying SQL, bind count, row count, and elapsed time; a span per transaction; and an
event per migration statement. This is perhaps two days of work and is the single
highest-leverage change available.

### 5.4 Connection pool is not configurable — **HIGH**

`crates/runtime/src/pool.rs` is 17 lines in total:

```rust
pub async fn connect(url: &str) -> Result<Pool, crate::Error> {
    sqlx::any::install_default_drivers();
    Pool::connect(url).await.map_err(Into::into)
}
```

This accepts sqlx defaults — 10 max connections, 30 s acquire timeout, no max lifetime,
no idle timeout, no `min_connections`, no `test_before_acquire` control, no
`after_connect` hook for session settings such as `statement_timeout` or `search_path`.
There is no exposed `PoolOptions` path.

**Consequence:** You cannot size the pool to your database's `max_connections`, cannot
recycle connections through a load balancer or failover, and cannot set a statement
timeout — which means a single pathological query can hold a connection indefinitely.

**Fix:** Expose a `ConnectOptions`/builder that forwards to `sqlx::pool::PoolOptions`,
and re-export the sqlx types so advanced users can construct a pool directly. This is a
small, purely additive API change.

---

## 6. Significant gaps

### 6.1 CI contains a stale job that now fails, and the strongest gate is not automated

The `generated-code-lint` job in `.github/workflows/ci.yml` is a placeholder from the
pre-P3 era. It asserts the generator is *still unimplemented* by grepping for the string
`"not implemented yet"`, and fails deliberately if it is not found:

```yaml
if cargo run -q -p ruprizzle-cli -- generate 2>&1 | grep -q "not implemented yet"; then
  echo "generator not implemented yet (expected before P3); nothing to lint"
else
  echo '::error::ruprizzle generate now produces output.'
  exit 1
fi
```

The generator has been implemented since commit `6fbfb8d` (P3). I confirmed `generate`
now runs. **This job therefore fails on every push**, which means either CI is currently
red on `main` or the failure is being ignored — both bad, and the second is worse
because it trains the team to disregard red.

Compounding this: the *real* generated-code guarantee lives in the two `#[ignore]`d tests
in `crates/codegen/tests/compile.rs`, whose ignore reason reads `"(CI: --ignored)"` —
but **no CI job passes `--ignored`**. `ci.yml` runs plain `cargo test --workspace`. Only
`cargo xtask harden` runs them, and `xtask harden` is never invoked by any workflow. The
project's flagship quality guarantee is enforced solely by a human remembering to run a
local command.

**Fix:** Delete the stale job. Replace it with `cargo test -p ruprizzle-codegen --test
compile -- --include-ignored`. Add a scheduled or pre-release job that runs
`cargo xtask harden` in full.

### 6.2 CI is Linux-only

All seven jobs run on `ubuntu-latest`. There is no Windows or macOS job — despite the
CLI being distributed via `cargo install` to developers on all three platforms, the
primary development machine for this project being Windows, and the codebase doing
filesystem path manipulation, file watching (`notify`), and subprocess invocation, all
of which are the classic sources of cross-platform breakage.

**Fix:** Add `windows-latest` and `macos-latest` to the `test` job matrix. SQLite-only
is sufficient for those runners; keep the Postgres integration job on Linux.

### 6.3 Supply-chain scanning is configured but never runs

`deny.toml` is well constructed — three target triples, an explicit licence allowlist, a
scoped `ring`/OpenSSL exception, `wildcards = "deny"`, `unknown-git = "deny"`,
`required-git-spec = "tag"`. It is a thoughtful configuration.

It is invoked only from `cargo xtask harden`, which no workflow calls, and even there it
is skipped silently if `cargo-deny` is not installed. Across 335 resolved dependencies,
**no advisory or licence check runs automatically.** There is also no Dependabot or
Renovate configuration, so dependencies will drift and CVEs will go unnoticed.

**Fix:** Add a `cargo-deny` CI job (`EmbarkStudios/cargo-deny-action`) and a
`.github/dependabot.yml` for the cargo ecosystem.

### 6.4 Concurrent `migrate deploy` can spuriously fail

In `apply_all` (`crates/migrate/src/runner.rs:200`), the pending set is computed at line
209–214, but the advisory lock is not acquired until line 238 — *inside* each
per-migration transaction, after the set is fixed.

Two deployers starting simultaneously (rolling deploy, two replicas, CI re-run) both
compute the same pending list. The advisory lock correctly serialises the transactions,
but once the first commits, the second proceeds to execute the same DDL again. The
tracking-table insert is an idempotent upsert, but the DDL is not: `CREATE TABLE` fails
with *"relation already exists"* and the deploy errors out.

Data integrity is preserved — the transaction rolls back atomically — so this is a
liveness and operational-noise bug, not a corruption bug. But a failed deploy at 3 a.m.
that is actually a no-op is an expensive false alarm.

**Fix:** Acquire the advisory lock (session-scoped, or a transaction wrapping the whole
run) *before* computing the pending set, and re-read applied IDs inside the lock.

### 6.5 No end-to-end performance data

The only benchmark is `crates/runtime/benches/query_construction.rs`, which measures
in-memory builder construction — the cheapest part of any ORM operation. There is no
measurement of query execution latency, throughput under concurrency, `include`
batching efficiency at scale, memory per row, or pool contention.

This matters specifically because of the `sqlx::Any` design (§7.1): every `Uuid`,
`Decimal`, `DateTime`, `Date`, `Time`, and `Json` value is serialised to text on the way
in and parsed from text on the way out, on every row. That cost is real and currently
unquantified. `RELEASES.md` correctly declines to claim performance superiority, but
"we don't claim it" and "we haven't measured it" are different positions, and only the
first is defensible indefinitely.

**Fix:** Add a criterion benchmark hitting a real database — single-row fetch, 1k-row
fetch, `include` across three levels, bulk insert — and publish the numbers.

### 6.6 Missing governance and release documentation

Absent: `CONTRIBUTING.md`, `SECURITY.md`, `CODE_OF_CONDUCT.md`, `CHANGELOG.md`,
`.github/dependabot.yml`, issue and PR templates, and a publish/release workflow
(`.github/workflows/` contains only `ci.yml` and `pages.yml`).

`SECURITY.md` is the most consequential omission: a database library with no documented
vulnerability disclosure path will receive its first security report as a public GitHub
issue. `RELEASES.md` exists and is well written, but it is release notes, not a
Keep-a-Changelog changelog.

---

## 7. Architectural risks worth naming

### 7.1 The `sqlx::Any` foundation is load-bearing and expensive to reverse

Every query in the runtime goes through `sqlx::Any`, the type-erased driver. This buys a
genuinely valuable property: one identical Rust API across Postgres and SQLite, with the
dialect chosen by URL scheme at runtime. The cost is significant and worth stating
plainly.

`Any` does not implement `Encode`/`Decode` for rich types, so the codebase works around
it in both directions:

- **Outbound** (`crates/runtime/src/value.rs:158`): `Uuid`, `Decimal`, `DateTime`,
  `Date`, `Time`, and `Json` are each `to_string()`'d and bound as text, with a comment
  reading *"let the database cast from text/bytes"*.
- **Inbound** (`crates/runtime/src/decode.rs:33`): decoding tries `String`, then falls
  back to `Vec<u8>`, then parses — a heuristic dual-path per column.

The consequences:

1. **Per-row serialisation cost** on every rich-typed column, in both directions.
2. **Reliance on server-side type inference.** Binding a `uuid` comparison as text works
   because Postgres infers the parameter type from context. If inference ever resolves to
   `text` rather than `uuid`, the comparison stops using the index — a silent performance
   cliff rather than an error.
3. **Timezone and format fragility.** `DateTime` is bound as RFC 3339 text and parsed
   back from text; correctness depends on server `DateStyle` and session timezone.
4. **Postgres-native features are unreachable.** Arrays are explicitly rejected at
   runtime (`value.rs:204`: `"array bind values are not supported yet"`). `LISTEN`/`NOTIFY`,
   `COPY`, and composite types are out of reach.
5. **Fragility is demonstrated by history.** Three of the last four commits are fixes in
   exactly this layer: *"fix sqlx::Any rich-type decoding"*, *"cross-dialect boolean
   decoding"*. The abstraction is leaking, repeatedly, in the same place.

This is not necessarily the wrong call — the API uniformity it delivers is the product's
core promise. But it should be a **documented ADR with its costs enumerated**, and the
0.2 roadmap should consider generating dialect-specific native code paths behind a
feature flag. Reversing it after users depend on runtime dialect selection will be
painful.

### 7.2 `ruprizzle-macros` ships as an empty crate

`crates/macros/src/lib.rs` is 16 lines containing a single private
`fn placeholder_until_p4()`. Its own doc comment advertises *"the injection-safe `raw!`
fragment builder"*, and `MasterPlan.md` lists the crate as shipping to users. Neither
`raw!` nor any other macro exists.

Meanwhile `README.md:135` promises *"the ability to drop down to raw SQL without leaving
the query builder"* — a claim delivered instead by `Tx::execute`/`fetch_all` and
`Executor::execute_raw`, which take a bare `&str` with a separate `Vec<Value>`. That
works, but it is not the ergonomic escape hatch the docs describe, and publishing an
empty crate to crates.io claiming that functionality is a promise that will need
honouring or retracting.

### 7.3 Public error enums are not `#[non_exhaustive]`

`ruprizzle::Error` and `ruprizzle_migrate::Error` are public enums without
`#[non_exhaustive]`. Any downstream `match` is exhaustive, so adding a variant — an
inevitability for a database library that will encounter new constraint classes — is a
semver-breaking change. Adding the attribute now, before 0.1.0 final, costs nothing;
adding it after costs a major version.

### 7.4 Error messages may echo user data into logs

`Error::UniqueViolation` interpolates the conflicting value into its `Display` output
(`crates/runtime/src/error.rs:7`). For the common case — a duplicate signup — that value
is an email address. Anything that logs the error, which is the default behaviour of
every web framework, writes PII to disk. Under GDPR or similar regimes this needs to be
a deliberate, documented, and ideally opt-in behaviour.

### 7.5 Advisory lock uses a hardcoded, collision-prone key

`SELECT pg_advisory_xact_lock(42)` (`runner.rs:238`). Advisory lock keys share a single
global namespace per database. Any other tool or application using key `42` — and `42`
is the single most likely magic number a developer picks — will contend with, or be
blocked by, migrations. Derive the key from a hash of the migration table name instead.

---

## 8. Minor findings

| # | Finding | Location | Severity |
|---|---|---|---|
| 1 | `execution_ms` records cumulative elapsed time since the loop began, not per-migration duration — the third migration reports the total of all three. | `runner.rs:263` | Low |
| 2 | `is_postgres` acquires a pool connection solely to read `backend_name()`, then drops it. | `runner.rs:207` | Trivial |
| 3 | `xtask` panic audit prints findings but always returns `Ok(())` — it is advisory, never a gate, despite being presented as a hardening step. | `xtask/src/main.rs:166` | Low |
| 4 | `README.md:23` states *"Phases P1–P7 are implemented and P8 … is the current focus"*, but P8 shipped in commit `418475f`. | `README.md` | Low |
| 5 | The MSRV job runs `cargo build` only, never `cargo test` — MSRV is verified to compile but not to work. | `ci.yml` | Low |
| 6 | 29 `unwrap()`/`expect()` calls in `crates/parser/src` (of 41 workspace-wide). Concentrated in grammar handling where invariants are compiler-guaranteed, but they are the largest panic surface in the codebase and are not individually justified by comment. | `crates/parser/src` | Low |
| 7 | No savepoint or nested-transaction support; `Tx` is flat commit/rollback only. | `tx.rs` | Low |
| 8 | `Value::Array` exists in the runtime enum but errors at bind time. Currently unreachable (the `IN` compiler expands to individual binds), so it is dead defensive code that reads as an unimplemented feature. | `value.rs:204` | Trivial |
| 9 | No health-check or `ping` helper on the pool for readiness probes. | `pool.rs` | Low |

---

## 9. Path to production

Estimates assume one experienced Rust developer.

### Phase 1 — Correctness blockers (~1 week)

1. Fix the UTF-8 corruption in `split_statements` (§5.1). Add a non-ASCII regression test. **Half a day, highest priority in this document.**
2. Add dollar-quote support to the splitter (§5.2). **One day.**
3. Move advisory-lock acquisition before pending-set computation (§6.4). **Half a day.**
4. Fix the per-migration `execution_ms` calculation (§8.1). **One hour.**
5. Add `#[non_exhaustive]` to public error enums (§7.3). **One hour — do this before any publish.**
6. Derive the advisory lock key from the migration table name (§7.5). **One hour.**

### Phase 2 — Operability (~1.5 weeks)

7. Add `tracing` instrumentation: span per query with SQL, bind count, row count, and duration; span per transaction; event per migration statement (§5.3). **Two to three days.**
8. Expose `PoolOptions` — max/min connections, acquire timeout, idle and max lifetime, `after_connect` hook (§5.4). **One to two days.**
9. Add pool metrics accessors and a `ping`/health-check helper (§8.9). **One day.**
10. Make PII in error `Display` opt-in or redacted by default (§7.4). **One day.**

### Phase 3 — CI and supply chain (~1 week)

11. Delete the stale `generated-code-lint` job; replace it with `--include-ignored` (§6.1). **Two hours.**
12. Add `windows-latest` and `macos-latest` to the test matrix (§6.2). **Half a day — expect real failures to fix.**
13. Add a `cargo-deny` job and `dependabot.yml` (§6.3). **Half a day.**
14. Wire `cargo xtask harden` into a scheduled or pre-release workflow (§6.1). **Half a day.**
15. Make MSRV run tests, not just build (§8.5). **One hour.**
16. Add `SECURITY.md`, `CONTRIBUTING.md`, `CHANGELOG.md` (§6.6). **One day.**

### Phase 4 — Confidence and measurement (~2 weeks)

17. End-to-end benchmarks against real databases; publish the numbers (§6.5). **Three days.**
18. Property-based tests for the diff engine — generate a random schema pair, diff, apply, and assert the result matches the target schema. **Three days. This is the highest-value test investment available**, because the diff engine is where an untested edge case becomes lost data.
19. Concurrency tests for `migrate deploy` and pool behaviour under contention. **Two days.**
20. Either implement `raw!` or remove `ruprizzle-macros` from the published set (§7.2). **One to three days depending on the choice.**
21. Write the `sqlx::Any` ADR with its costs enumerated, and decide the 0.2 position (§7.1). **One day.**

**Total to a defensible 0.1.0 production release: 5–6 weeks.**

---

## 10. Recommendation by use case

| Use case | Verdict |
|---|---|
| Side projects, prototypes, internal tools | ✅ **Use it.** The DX is good, the errors are excellent, and the blast radius is acceptable. |
| Production service, non-critical data | ⚠️ **After Phases 1–2.** You need observability and a tunable pool before you can operate it. |
| Production service, critical or regulated data | ❌ **Not yet.** Wait for Phase 4. The migration engine needs property-based testing before it should be trusted with data you cannot lose. |
| Evaluation against Diesel / SeaORM / sqlx | ✅ **Worth evaluating.** The schema-first migration diffing is genuinely differentiated — no other Rust ORM has it — and that is the right reason to be interested. |
| Publishing `0.1.0-alpha.1` to crates.io today | ⚠️ **After Phase 1 items 1, 2, and 5.** Shipping the UTF-8 corruption bug to users, and shipping error enums without `#[non_exhaustive]`, are both avoidable in under a day. |

---

## 11. Closing assessment

Judged against what it claims to be — an alpha — this project is **well ahead of the
curve**. The eight-crate architecture is disciplined, the security posture is
structurally sound rather than incidentally so, the test harness is honest about what
it does and does not verify, and the documentation refuses to overclaim. A 3.8 : 1
source-to-test ratio with 167 tests green against two live databases, plus verified
`clippy::pedantic`-clean generated output, is a standard many 1.0 releases do not meet.

The gap to production is not a quality gap. It is a **completeness gap in the operational
layer** — telemetry, pool control, supply-chain automation, cross-platform verification —
plus a small number of concrete defects in the migration engine that targeted probing
found quickly and that targeted testing would have caught.

The single most important action is fixing the UTF-8 corruption in `split_statements`
(§5.1). It is a half-day fix on a silent data-corruption path, and it should not survive
another commit.

The second most important is adding `tracing`. Everything else on the list is a matter
of scheduling; that one is a matter of whether the library can be operated at all.

---

*Assessment methodology: full source review of 14,440 lines across 8 crates; live
execution of `cargo fmt`, `cargo clippy --all-targets -D warnings`, and the complete
test suite against PostgreSQL 17.10 and SQLite with `RUPRIZZLE_REQUIRE_DB=1`; execution
of the gated generated-code compile and pedantic-lint suites; targeted probe tests
written and run to confirm §5.1 and §5.2 (removed afterward); review of CI workflows,
`deny.toml`, release tooling, and all 915 lines of user documentation.*
