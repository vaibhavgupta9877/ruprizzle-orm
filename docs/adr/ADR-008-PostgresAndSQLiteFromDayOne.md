# ADR-008 · Postgres and SQLite together from day one

**Decision:** both dialects in v1, not Postgres first and SQLite later.
**Why:** an abstraction with one implementation is not proven to be an abstraction.
Adding SQLite in month two would mean discovering every Postgres assumption baked
into codegen and migrations at the worst possible moment. SQLite also makes the
test suite fast and dependency-free for contributors.
**Cost:** roughly three extra days in P2 and P6, mostly the SQLite table-rebuild
path. Paid deliberately.
