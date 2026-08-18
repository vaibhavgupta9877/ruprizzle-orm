# Frequently asked questions

## What is ruprizzle-orm?

A schema-first ORM for Rust. You write a Prisma-style `.ruprizzle` schema, and
the CLI generates typed entities, a Drizzle-style query builder, and migration
SQL. It targets PostgreSQL, MySQL/MariaDB, and SQLite 3+.

## Is it production-ready?

`1.0.0-rc.1` is a release candidate. The public API is frozen for the 1.0 line,
but the project is collecting at least two weeks of real-world feedback before
declaring `1.0.0`. See [Stability](Stability.md) and
[Known limitations](KnownLimitations.md) for the honest boundaries.

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

Yes. Use `ruprizzle check` with a query manifest captured from tests or examples.
See [ADR-012](adr/ADR-012-OfflineQueryChecking.md) for the design.

## Is there an LSP?

Yes. `ruprizzle-lsp` provides completion, diagnostics, and go-to-definition for
`schema.ruprizzle`. A VS Code extension is in `editor/`.

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
        "text": "1.0.0-rc.1 is a release candidate. The public API is frozen for the 1.0 line, but the project is collecting at least two weeks of real-world feedback before declaring 1.0.0."
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
