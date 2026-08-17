# Architecture Decision Records

These records capture significant design decisions in ruprizzle-orm. Each ADR
explains the decision, the rationale, and the costs.

| ADR | Title |
|-----|-------|
| ADR-001 | [Build on sqlx rather than a custom driver](ADR-001-BuildOnSqlx.md) |
| ADR-002 | [Codegen, not proc-macros-over-structs](ADR-002-CodegenNotProcMacros.md) |
| ADR-003 | [`Related<T>` instead of `Option<T>` for relations](ADR-003-RelatedInsteadOfOption.md) |
| ADR-004 | [Batched relation loading, not JOINs](ADR-004-BatchedRelationsNoJoins.md) |
| ADR-005 | [Column tokens, not a type-level query DSL](ADR-005-ColumnTokens.md) |
| ADR-006 | [Explicit join models for many-to-many](ADR-006-ExplicitJoinModels.md) |
| ADR-007 | [Snapshot = serialized IR](ADR-007-SnapshotSerializedIr.md) |
| ADR-008 | [Postgres and SQLite together from day one](ADR-008-PostgresAndSqlite.md) |
| ADR-009 | [Runtime dialect selection via `sqlx::Any`](ADR-009-RuntimeDialectSelection.md) |
| ADR-010 | [Postgres arrays and SQLite JSON fallback](ADR-010-PostgresArraysAndSqliteFallback.md) |
| ADR-011 | [Explicit joins alongside batched relations](ADR-011-ExplicitJoinsAlongsideBatchedRelations.md) |
