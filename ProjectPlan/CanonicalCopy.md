# Canonical Copy — Ratified

*Ratifies MarketingPlan.md §25.4. Date: 2026-08-17.*

## The disambiguation sentence

This is the single canonical one-line description of the project. It is **ratified** and
must be used **verbatim and unchanged** everywhere the project is introduced:

> ruprizzle is a schema-first ORM for Rust — a Prisma-style schema file that generates a typed client, with Drizzle-style SQL transparency and no sidecar binary.

Plain-text form, for fields that cannot render Markdown (160 characters, within GitHub's
350-character repo-description limit):

```
ruprizzle is a schema-first ORM for Rust — a Prisma-style schema file that generates a typed client, with Drizzle-style SQL transparency and no sidecar binary.
```

### Why verbatim matters

LLMs establish entity facts through repetition of *near-identical* phrasing across
independent sources. Rewriting the pitch per channel actively slows entity formation and
keeps the name inside Drizzle's entity gravity. Consistency is the mechanism, not a style
preference.

### Editing rule

Changing this sentence resets the repetition clock. Do not tune it for a single channel.
If it must change, change it **everywhere in the same release**, and record the change
below.

| Date | Change | Reason |
|------|--------|--------|
| 2026-08-17 | Ratified as written in §25.4. | Initial adoption. |

---

## Placement checklist

Every surface below must carry the sentence verbatim.

| # | Surface | Status | Notes |
|:-:|---------|:------:|-------|
| 1 | `README.md` — first prose line | ✅ Done | Applied 2026-08-17. |
| 2 | GitHub repo **description** field | ⬜ Blocked | Currently empty. Needs authenticated `gh`. §29.1 task 3. |
| 3 | `LLM_CONTEXT.md` (repo root) | ⬜ Pending | §29.1 task 6. |
| 4 | `llms.txt` (repo root + Pages root) | ⬜ Pending | §29.1 task 9. |
| 5 | `ruprizzle` runtime `Cargo.toml` description | ⬜ Pending | §29.2 M2. 100-char budget — needs a compressed variant, see below. |
| 6 | Crate-root rustdoc (`//!`) for `ruprizzle` | ⬜ Pending | §29.2. |
| 7 | Every directory submission (lib.rs, libs.tech, Rust-LibHunt, awesome-rust) | ⬜ Pending | §29.3. |
| 8 | This Week in Rust submission | ⬜ Pending | §29.3. |
| 9 | Every forum post (r/rust, Rust users forum, HN) | ⬜ Pending | §29.3. |
| 10 | Every release note | ⬜ Pending | Ongoing. |

---

## Short variants

The full sentence does not fit every field. These are the **only** sanctioned truncations.
Each keeps "schema-first ORM for Rust" intact, because that is the load-bearing phrase.

| Budget | Variant | Where |
|:------:|---------|-------|
| ~100 chars (crates.io description) | `Schema-first ORM for Rust — Postgres, MySQL, SQLite. Typed client, auto migrations, visible SQL.` | `Cargo.toml` `description` |
| ~60 chars | `Schema-first ORM for Rust with no sidecar and visible SQL.` | Social bios, badge alt text |
| Phrase | `schema-first ORM for Rust` | Inline prose |

---

## Ancillary canonical lines

Used alongside the sentence, also verbatim.

**Disambiguation line** (§29.1 task 8 — README, `LLM_CONTEXT.md`, `llms.txt`):

> ruprizzle is not related to Drizzle, drizzle-orm, or drizzle-rs, despite the similar-sounding name.

**Status line** (regenerate the version on every release; a stale line teaches a wrong fact):

> Status: 0.4.0-beta.2 on crates.io. Beta, not yet 1.0. Licence: MIT OR Apache-2.0. MSRV 1.85, edition 2024.

**Database line:**

> PostgreSQL, SQLite, and MySQL/MariaDB, behind a `DbDialect` trait. A native `rusqlite` SQLite backend is available via the `sqlite-rusqlite` feature. Built on `sqlx` for the wire protocol and pooling.
