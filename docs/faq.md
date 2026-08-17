# Frequently asked questions

## What is ruprizzle-orm?

A schema-first ORM for Rust. You write a Prisma-style `.ruprizzle` schema, and
the CLI generates typed entities, a Drizzle-style query builder, and migration
SQL. It targets Postgres and SQLite from day one.

## Is it production-ready?

Not yet. The current release is `0.4.0-beta.2`. The API will change, and the
[known limitations](KnownLimitations.md) are documented explicitly.

## How is it different from Diesel or SeaORM?

- It is schema-first: the schema file is the source of truth.
- It generates a type-safe, token-based query builder where cross-model or
  wrong-typed filters are compile errors.
- It supports nested `include` with per-relation filters in a bounded number of
  queries.
- It diffs the schema to generate migrations automatically.

## Which databases are supported?

Postgres and SQLite. The dialect trait makes adding more backends an additive
change.

## Does it require a query engine sidecar?

No. The runtime is a library built on `sqlx`. There is no separate process or
hidden query engine binary.

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
        "text": "A schema-first ORM for Rust. You write a Prisma-style .ruprizzle schema, and the CLI generates typed entities, a Drizzle-style query builder, and migration SQL. It targets Postgres and SQLite from day one."
      }
    },
    {
      "@type": "Question",
      "name": "Is it production-ready?",
      "acceptedAnswer": {
        "@type": "Answer",
        "text": "Not yet. The current release is 0.4.0-beta.2. The API will change, and the known limitations are documented explicitly."
      }
    },
    {
      "@type": "Question",
      "name": "How is it different from Diesel or SeaORM?",
      "acceptedAnswer": {
        "@type": "Answer",
        "text": "It is schema-first: the schema file is the source of truth. It generates a type-safe, token-based query builder where cross-model or wrong-typed filters are compile errors. It supports nested include with per-relation filters in a bounded number of queries. It diffs the schema to generate migrations automatically."
      }
    },
    {
      "@type": "Question",
      "name": "Which databases are supported?",
      "acceptedAnswer": {
        "@type": "Answer",
        "text": "Postgres and SQLite. The dialect trait makes adding more backends an additive change."
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
      "name": "How do I report bugs or request features?",
      "acceptedAnswer": {
        "@type": "Answer",
        "text": "Open an issue on the GitHub repository https://github.com/vaibhavgupta9877/ruprizzle-orm."
      }
    }
  ]
}
</script>
