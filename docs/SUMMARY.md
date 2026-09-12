# Summary

[Introduction](README.md)

# Getting started

- [Quickstart](quickstart.md)
- [Schema reference](SchemaReference.md)
- [Query guide](QueryGuide.md)
- [Relations guide](RelationsGuide.md)
- [Migrations guide](MigrationsGuide.md)
- [Examples](Examples.md)

# Choosing ruprizzle

- [ruprizzle vs Diesel, SeaORM, SQLx and Prisma](FeaturesMasterComparison.md)
- [Migrating from other ORMs](MigratingFrom.md)
- [Performance](performance.md)
- [Benchmark results](BenchmarkResults.md)
- [FAQ](faq.md)

# Running in production

- [Operations](Operations.md)
- [Dialect notes](DialectNotes.md)
- [Known limitations](KnownLimitations.md)
- [Stability and semantic versioning](Stability.md)
- [Versioning policy](Versioning.md)
- [Soak test report](SoakReport.md)

# Releases

- [What's new in v1.1–v1.5](WhatsNewV1_1ToV1_5.md)
- [Migration guide to v1](MigrationGuideToV1.md)
- [1.0.0 announcement](announcement.md)

# Reference

- [Architecture decision records](adr/index.md)
  - [Build on sqlx rather than a custom driver](adr/ADR-001-BuildOnSqlx.md)
  - [Codegen, not proc-macros-over-structs](adr/ADR-002-CodegenNotProcMacros.md)
  - [`Related<T>` instead of `Option<T>` for relations](adr/ADR-003-RelatedInsteadOfOption.md)
  - [Batched relation loading, not JOINs](adr/ADR-004-BatchedRelationsNoJoins.md)
  - [Column tokens, not a type-level query DSL](adr/ADR-005-ColumnTokens.md)
  - [Explicit join models for many-to-many](adr/ADR-006-ExplicitJoinModels.md)
  - [Snapshot = serialized IR](adr/ADR-007-SnapshotSerializedIr.md)
  - [Postgres and SQLite together from day one](adr/ADR-008-PostgresAndSqlite.md)
  - [Runtime dialect selection via `sqlx::Any`](adr/ADR-009-RuntimeDialectSelection.md)
  - [Postgres arrays and SQLite JSON fallback](adr/ADR-010-PostgresArraysAndSqliteFallback.md)
  - [Explicit joins alongside batched relations](adr/ADR-011-ExplicitJoinsAlongsideBatchedRelations.md)
  - [Offline query checking](adr/ADR-012-OfflineQueryChecking.md)
