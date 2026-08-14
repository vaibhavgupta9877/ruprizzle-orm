# ADR-011 — Explicit joins alongside batched relations

**Date:** 2026-08-17  
**Status:** Accepted

## Context

ADR-004 chose batched relation loading over JOINs for `include`. That decision
remains correct for relation loading: it avoids row explosion, keeps `take` and
`filter` per relation simple, and only adds a bounded number of round trips.
However, batched loading cannot express every query. Reporting queries, joins
on non-relation keys, and self-joins all need an explicit JOIN in the SQL.

## Decision

1. **Batched loading stays the default for `include`.** ADR-004 is not weakened.
   `include` still issues one query per relation level.

2. **Explicit joins are a separate, opt-in query-builder feature.** They are
   exposed through `SelectQuery::inner_join`, `left_join`, `right_join`, and
   `full_join` for cases the batcher cannot express.

3. **Join conditions are inferred from declared relations when possible.** If the
   join target is a model related to the source through a schema relation, the
   builder uses the foreign-key columns from the relation definition.

4. **Arbitrary join conditions are supported via typed column tokens from both
   sides.** A `JoinOn` or `On` builder lets users write `A::id.eq(B::a_id)`
   across two models, even when no schema relation connects them.

5. **Joined queries return tuples of model types.** An inner join returns
   `(A, B)`. A left join returns `(A, Option<B>)`. A right join returns
   `(Option<A>, B)`. A full join returns `(Option<A>, Option<B>)`. This is the
   natural SQL semantics and is enforced at compile time.

6. **Self-joins use table aliasing.** The builder generates a unique alias for
   the joined side and prefixes its columns so the two sides of a self-join are
   distinguishable in the result set and in `FromRow` decoding.

7. **Dialects emit the appropriate JOIN keywords.** `RIGHT JOIN` and `FULL OUTER
   JOIN` are not supported by SQLite before 3.39. The dialect capability check
   reports this and the builder emits a clear construction-time error rather
   than SQL that fails at the server.

## Consequences

- Users can write SQL-like typed joins without losing the batched `include`
  path.
- The query surface covers Drizzle's `leftJoin`/`innerJoin` and Diesel's join
  DSL parity.
- Compile-time type safety for outer-join nullability (left/right/full).
- SQLite users get a helpful error for right/full joins unless they target a
  backend that supports them.
