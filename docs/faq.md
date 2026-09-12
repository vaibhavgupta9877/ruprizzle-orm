# Frequently asked questions

## What is ruprizzle-orm?

A schema-first ORM for Rust. You write a Prisma-style `.ruprizzle` schema, and
the CLI generates typed entities, a Drizzle-style query builder, and migration
SQL. It targets PostgreSQL, MySQL/MariaDB, and SQLite 3+.

## Is it production-ready?

Not yet. The only version published on crates.io is `1.0.0-rc.1`, a release
candidate. No stable release has been published: `v1.0.0`, `v1.0.1` and `v1.5.0`
are git tags whose publish runs failed before uploading anything. Use ruprizzle
for evaluation and bounded projects, not as a default mission-critical dependency.

The design is complete and the test suite is broad, but the release controls are
not green. The current, honest assessment scores the project 62/100 and lists what
is blocking publication; see
[the remediation plan](https://github.com/vaibhavgupta9877/ruprizzle-orm/blob/main/ProjectPlan/ProductionReadinessSolPlan.md).

Two gates were waived along the way, both in writing rather than by omission: the
48-hour `rusqlite` soak, accepted on 15.56 h / 1.46 B ops / 0 errors, and the
two-week release-candidate feedback window, because the project had no external
consumers to collect feedback from. See [Stability](Stability.md) for the semver
policy, the waivers, and the public-dependency list (the 1.0 line is pinned to
`sqlx 0.8`), and [Known limitations](KnownLimitations.md) for the boundaries.

The v1.1–v1.5 line ([what's new](WhatsNewV1_1ToV1_5.md)) is complete in the
repository as `1.5.0`, adding Ruprizzle Studio and the Turso and D1 adapters, and
is buildable from source. There is no Neon adapter: Neon is Postgres over TLS, so
its connection string goes straight to `ruprizzle::connect`.

## Which version should I install?

`1.0.0-rc.1` — it is the only one on crates.io.

```toml
[dependencies]
ruprizzle = "1.0.0-rc.1"
```

For the v1.1–v1.5 features, depend on the git repository instead:

```toml
[dependencies]
ruprizzle = { git = "https://github.com/vaibhavgupta9877/ruprizzle-orm" }
```

## What databases does ruprizzle support?

PostgreSQL, MySQL/MariaDB and SQLite 3+, behind a `DbDialect` trait. Connections
go through [`sqlx`](https://github.com/launchbadge/sqlx) by default. Two native
drivers are available as optional features: `sqlite-rusqlite` for synchronous
SQLite, and an experimental `postgres-tokio-postgres` for PostgreSQL. Turso/libSQL
and Cloudflare D1 adapters exist in the repository, over their providers' HTTP
APIs, but have not been published to crates.io and have not been validated against
a live hosted database.

## Is ruprizzle free? What licence is it under?

Yes — free and open source, dual-licensed MIT OR Apache-2.0, the standard
arrangement in the Rust ecosystem. There is no paid tier, no telemetry and no
commercial licence to buy.

## How is it different from Diesel or SeaORM?

- It is schema-first: the schema file is the single source of truth.
- It generates a type-safe, token-based query builder where cross-model or
  wrong-typed filters are compile errors.
- It supports nested `include` with per-relation filters in a bounded number of
  queries.
- It diffs the schema to generate migrations automatically.
- It exposes `.to_sql()` on every builder.

## Which databases are supported?

PostgreSQL 17+, MySQL/MariaDB, and SQLite 3+ through SQLx. Native `rusqlite` and
`tokio-postgres` drivers are available behind feature flags for better SQLite and
PostgreSQL performance.

## Does it require a query engine sidecar?

No. The runtime is a library built on `sqlx`. There is no separate process or
hidden query engine binary.

## Does it support compile-time query checking?

Yes. Use `ruprizzle check --manifest <path>` with a query manifest captured from tests or examples.
See [ADR-012](adr/ADR-012-OfflineQueryChecking.md) for the design.

## Is there an LSP?

Yes. `ruprizzle lsp` (and the `ruprizzle-lsp` crate) provide completion, hover,
formatting, diagnostics, and go-to-definition for `schema.ruprizzle`. A VS Code
extension is in `editor/`.

## How do I report bugs or request features?

Open an issue on the [GitHub repository](https://github.com/vaibhavgupta9877/ruprizzle-orm).

<script type="application/ld+json">
{
  "@context": "https://schema.org",
  "@type": "FAQPage",
  "mainEntity": [
    {
      "@type": "Question",
      "name": "What is ruprizzle-orm?",
      "acceptedAnswer": {
        "@type": "Answer",
        "text": "A schema-first ORM for Rust. You write a Prisma-style .ruprizzle schema, and the CLI generates typed entities, a Drizzle-style query builder, and migration SQL. It targets PostgreSQL, MySQL/MariaDB, and SQLite 3+."
      }
    },
    {
      "@type": "Question",
      "name": "Is it production-ready?",
      "acceptedAnswer": {
        "@type": "Answer",
        "text": "1.0.0 is a stable release, with the public API covered by semantic versioning. The two-week release-candidate feedback window was waived, with reasons documented in the stability policy, because the project had no external consumers to collect feedback from. The 1.0 line is pinned to sqlx 0.8, since ruprizzle re-exports sqlx as part of its own public API."
      }
    },
    {
      "@type": "Question",
      "name": "How is it different from Diesel or SeaORM?",
      "acceptedAnswer": {
        "@type": "Answer",
        "text": "It is schema-first, generates a type-safe token-based query builder where cross-model or wrong-typed filters are compile errors, supports nested include with per-relation filters, diffs the schema to generate migrations, and exposes .to_sql() on every builder."
      }
    },
    {
      "@type": "Question",
      "name": "Which databases are supported?",
      "acceptedAnswer": {
        "@type": "Answer",
        "text": "PostgreSQL 17+, MySQL/MariaDB, and SQLite 3+ through SQLx. Native rusqlite and tokio-postgres drivers are available behind feature flags."
      }
    },
    {
      "@type": "Question",
      "name": "Does it require a query engine sidecar?",
      "acceptedAnswer": {
        "@type": "Answer",
        "text": "No. The runtime is a library built on sqlx. There is no separate process or hidden query engine binary."
      }
    },
    {
      "@type": "Question",
      "name": "Does it support compile-time query checking?",
      "acceptedAnswer": {
        "@type": "Answer",
        "text": "Yes. Use ruprizzle check with a query manifest captured from tests or examples."
      }
    },
    {
      "@type": "Question",
      "name": "Is there an LSP?",
      "acceptedAnswer": {
        "@type": "Answer",
        "text": "Yes. ruprizzle-lsp provides completion, diagnostics, and go-to-definition for schema.ruprizzle. A VS Code extension is in editor/."
      }
    },
    {
      "@type": "Question",
      "name": "How do I report bugs or request features?",
      "acceptedAnswer": {
        "@type": "Answer",
        "text": "Open an issue on https://github.com/vaibhavgupta9877/ruprizzle-orm."
      }
    }
  ]
}
</script>
