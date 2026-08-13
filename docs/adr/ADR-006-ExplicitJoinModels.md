# ADR-006: Explicit join models for many-to-many

**Date:** 2026-08-13  
**Status:** Accepted

**Decision:** no implicit join tables in v1.

**Why:** hidden tables violate the predictable-SQL principle, and Prisma's implicit
`_AToB` tables become a migration dead end the moment you need a column on the
join. Sugar can be added later without breaking anyone.
