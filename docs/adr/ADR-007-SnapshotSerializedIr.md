# ADR-007: Snapshot = serialized IR

**Date:** 2026-08-13  
**Status:** Accepted

**Decision:** the migration snapshot is `serde_json` of `ir::Schema`.

**Why:** one type means the differ compares exactly what the parser produces, with
no second schema representation to drift out of sync. It is also human-readable in
diffs and easy to inspect during debugging.

**Cost:** IR changes require a snapshot format version and a migration path. A
`version` field is present from day one for this reason.
