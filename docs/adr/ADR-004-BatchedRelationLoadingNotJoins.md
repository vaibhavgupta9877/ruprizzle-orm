# ADR-004 · Batched relation loading, not JOINs

**Decision:** one query per relation level.
**Why:** see the comparison table in ImplPlan06 P5-03. JOINs cause row explosion,
make per-relation `take` and `filter` hard, and require de-duplication.
**Cost:** more round trips than a single JOIN for shallow queries. Acceptable — the
count is bounded by schema depth, not data size.
