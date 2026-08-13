# ADR-005 · Column tokens, not a type-level query DSL

**Decision:** `Column<M, T>` consts with inherent methods, rather than Diesel's
type-level relational algebra.
**Why:** Diesel's approach is more powerful and catches strictly more at compile
time, but its error messages are the single most cited reason people bounce off it.
Column tokens catch the errors that actually happen in practice — wrong value type,
wrong model, wrong operator for the type — with errors a human can read.
**Cost:** some invalid queries (a malformed `GROUP BY`, say) fail at the database
rather than at compile time.
