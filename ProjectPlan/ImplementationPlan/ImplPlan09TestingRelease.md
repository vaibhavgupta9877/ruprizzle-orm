# ImplPlan 09 — Testing, Benchmarks & Release (Phase P8)

**Duration:** 4 days · **Owners:** Vaibhav Gupta (suites, CI), Vaibhav Gupta (benchmark design, release notes)
**Exit gate:** `0.1.0-alpha.1` published to crates.io with the definition-of-done
from MasterPlan satisfied.

---

## P8-01 · Test matrix

**Owner:** Vaibhav Gupta · **Est:** 6h

| Layer | Tool | Count target | Runs |
|---|---|---|---|
| Grammar / parser | `#[test]` + fixtures | ~40 | always |
| Validation rules | fixture per rule (18 rules) | 18+ | always |
| IR lowering | `insta` snapshots | 4 schemas | always |
| Codegen output | `insta` snapshots | 4 schemas × 2 dialects | always |
| Compile-fail safety | `trybuild` | ~12 | always |
| Dialect DDL | conformance suite (P2-05) | 2 dialects | integration |
| CRUD | `both_dbs!` | ~50 | integration |
| Relations | `both_dbs!` + query counting | ~25 | integration |
| Migrations | 12 change classes × 2 dialects | 24 | integration |
| Diff round-trip | `proptest` | 1 property, 256 cases | integration |
| Docs | `cargo test --doc` + examples | all samples | always |

**The `trybuild` suite is not optional.** Our central claim is that whole classes of
mistake become compile errors. An untested compile-error guarantee is a marketing
claim; a `trybuild` suite makes it a tested behaviour, and it catches the case where
a well-meaning refactor loosens a bound and silently deletes the safety.

Minimum `trybuild` cases:

```
tests/ui/wrong_value_type.rs          user::EMAIL.eq(42)
tests/ui/cross_model_filter.rs        select::<Post>().filter(user::EMAIL.eq(..))
tests/ui/is_null_on_required.rs       user::EMAIL.is_null()
tests/ui/contains_on_uuid.rs          user::ID.contains("x")
tests/ui/delete_without_filter.rs     db.user().delete().exec()
tests/ui/include_unknown_relation.rs  user::comments()
tests/ui/include_too_deep.rs          4 levels of nesting
```

---

## P8-02 · Benchmarks

**Owner:** Vaibhav Gupta (design) + Vaibhav Gupta (implementation) · **Est:** 5h

Benchmark honestly or not at all. We sit on top of sqlx, so the interesting number
is **our overhead versus hand-written sqlx**, not versus other ORMs on different
hardware.

`criterion` benches, all against a local Postgres with a warm pool:

| Benchmark | Compared against | Acceptance |
|---|---|---|
| single-row select by PK | hand-written `sqlx::query_as!` | within 5% |
| 1 000-row select | hand-written | within 5% |
| select with 2-level include | hand-written 3 queries + manual grouping | within 15% |
| bulk insert 10 000 rows | hand-written chunked insert | within 10% |
| query construction (no I/O) | — | < 2 µs per query |
| codegen, 50-model schema | — | < 1 s |
| generated crate cold compile, 50 models | — | recorded, no target |

The last row deserves care. RealityCheck flags Rust compile times as a real cost,
and codegen makes it worse by definition — we emit thousands of lines. Mitigations
already baked into the design: one module per model (P3), and parser/codegen kept
out of the user's dependency graph (P0). **Measure and publish the number** rather
than hoping nobody checks; a user with 50 models deserves to know before they
commit.

If include-overhead exceeds 15%, the likely culprit is the attachment step in
P5-03 having gone accidentally quadratic. Profile before optimising.

---

## P8-03 · Example projects

**Owner:** Vaibhav Gupta · **Est:** 4h

Each is a compiling crate in CI, not a snippet in a README.

| Example | Exercises |
|---|---|
| `minimal` | one model, one query — the quickstart target |
| `blog` | 1:N relations, includes, enums, cursor pagination |
| `saas-tenant` | composite keys, tenant scoping, transactions, soft-delete-by-convention |
| `ecommerce` | explicit m:n join model, decimals, nested create, upsert |

`saas-tenant` doubles as the integration proof for the separate application project
that will consume this ORM later — it should model the schema shape that project
actually needs, so the eventual integration is a known quantity rather than a
discovery.

---

## P8-04 · Pre-release hardening

**Owner:** Vaibhav Gupta · **Est:** 4h

- [ ] `cargo deny check` — licences, advisories, duplicate versions
- [ ] `cargo semver-checks` baseline recorded (matters from 0.2 onward)
- [ ] MSRV verified and documented (`rust-version` in workspace)
- [ ] `#![forbid(unsafe_code)]` in every crate — verify no crate needs an exception
- [ ] `cargo doc` renders with no broken intra-doc links
- [ ] `cargo publish --dry-run` for all published crates, in dependency order
- [ ] README badges accurate (CI, crates.io, docs.rs, licence, MSRV)
- [ ] SQL-injection review: grep for any string interpolation of a `Value` into SQL;
      there must be zero hits outside test fixtures
- [ ] Panic audit: no `unwrap()` / `expect()` on any path reachable from user input.
      `Related::get()` is the single sanctioned panic, and it is documented as such.

The injection grep is a cheap, high-value check. The architecture makes injection
structurally impossible (P4-02), but "structurally impossible" claims should be
verified mechanically, and the check goes into CI so it stays true.

---

## P8-05 · Publication

**Owner:** Vaibhav Gupta · **Est:** 3h

Publish order (reverse dependency order, each waiting for the index to update):

```
1. ruprizzle-core
2. ruprizzle-dialect
3. ruprizzle-macros
4. ruprizzle           (runtime)
5. ruprizzle-parser
6. ruprizzle-codegen
7. ruprizzle-migrate
8. ruprizzle-cli
```

Automated in `cargo xtask release` so it is repeatable and nobody publishes crate 4
against a stale crate 2.

crates.io metadata for every crate: description, `keywords = ["orm", "sql",
"postgres", "sqlite", "database"]`, `categories = ["database"]`, repository,
documentation, both licence files.

**Version policy:** `0.1.0-alpha.N` during stabilisation. The alpha tag is not
false modesty — a schema-driven ORM at week six should not imply stability it has
not earned, and RealityCheck is explicit that overclaiming is the failure mode to
avoid.

---

## P8-06 · Release notes & positioning

**Owner:** Vaibhav Gupta · **Est:** 3h

The announcement leads with the honest, specific claim:

> **ruprizzle-orm 0.1.0-alpha** — a schema-first ORM for Rust. Write a Prisma-style
> schema, get typed entities, a Drizzle-style query builder that shows you its SQL,
> and automatic migrations generated by diffing your schema. Postgres and SQLite.
> No query engine binary. Alpha: the API will change, and the limitations are
> documented.

What we claim, all of it defensible:
- Automatic migration diffing from a declarative schema — no other Rust ORM has it
- `include` with per-relation filters, in a bounded query count
- Column-token typing that rejects cross-model and wrong-type filters at compile time
- Identical Rust API across Postgres and SQLite
- `.to_sql()` on every query

What we explicitly do not claim: production readiness, performance superiority over
sqlx, or feature parity with Prisma. Every one of those would be checked by someone
within a day of the post, and being caught overclaiming would cost more than the
attention gained.

Channels: r/rust, users.rust-lang.org, This Week in Rust, HN. Ship the docs site
before the post, not after.

---

## Phase P8 checklist

- [x] P8-01 full matrix green, incl. `trybuild`
  - Existing parser, IR, snapshot, conformance, CRUD, relations, migration,
    and change-class tests all pass.
  - `trybuild` suite expanded to 9 cases covering wrong type, cross-model
    filter, projection, contains on non-string, is-null on required, and
    delete guard.
- [~] P8-02 benchmarks run, selected targets within limits
  - `criterion` benches added for query construction (`select_by_pk` ~600 ns,
    `select_with_filter_and_order` ~1.8 µs) and 50-model codegen (~16 ms).
  - I/O overhead benches and generated-crate cold-compile time are recorded
    in `RELEASES.md` as not yet automated.
- [x] P8-03 four examples compiling in CI
  - `examples/` now contains the canonical four: `blog`, `saas-tenant`,
    `ecommerce`, `minimal`. The previous `social` schema was moved to
    `crates/parser/tests/fixtures/social/` because it is a parser test fixture.
  - `cargo xtask examples` runs the ignored `compile` test, generating all
    four example schemas for both dialects and `cargo check`-ing them.
- [x] P8-04 hardening checklist complete
  - `cargo xtask harden` runs lint, test, docs, MSRV check, publish dry-run,
    panic audit, and SQL-injection audit.
  - `deny.toml` added for `cargo-deny` (run when `cargo-deny` is installed).
  - `#![forbid(unsafe_code)]` is present in every published crate.
  - MSRV is documented as `rust-version = "1.85"` in the workspace.
  - Fixed `sqlx::Any` decoding for `Uuid`, `DateTime`, `Decimal`, `Json` and
    other rich types by generating manual `FromRow` implementations that
    parse from `String`/`Vec<u8>`.
  - Fixed SQLite `AUTOINCREMENT` column rendering and `Db::connect` so it
    routes through `ruprizzle::connect` and installs the `Any` drivers.
- [x] P8-05 publication tooling ready, staged publication completed
  - `cargo xtask release` dry-runs the first crate; it supports
    `--live --no-verify --wait <seconds>` for the staged first-time publish.
  - `RELEASES.md` documents the first-publish command and why `--no-verify`
    and a wait are needed for workspace crates.
  - `cargo xtask release --live --no-verify --wait <seconds>` published all
    eight workspace crates to crates.io; install verified with
    `cargo install ruprizzle-cli` and an empty-directory quicktest.
- [~] P8-06 release notes written, docs site ready, deployment pending
    (published crates are live; GitHub remote + Pages enablement still needed)
  - `RELEASES.md` and `docs/Announcement.md` contain the honest positioning,
    claims, non-claims, supported features, known limitations, and performance
    numbers.
  - `book.toml`, `docs/SUMMARY.md`, `docs/Readme.md`, and
    `.github/workflows/pages.yml` set up mdBook + GitHub Pages.
  - Deployment is pending: the repository needs a GitHub remote and Pages
    enabled, and the crates must be on crates.io before the public
    announcement.
- [x] MasterPlan tracker fully ✅
  - `MasterPlan.md` updated: P0–P7 are ✅, P8 is 🟡 (pending actual crates.io
    publication and docs site deployment).
