# ADR-001: Build on sqlx rather than a custom driver

**Date:** 2026-08-13  
**Status:** Accepted

**Decision:** sqlx provides connection pooling, the wire protocols, TLS, and type
encoding. We generate code that calls it.

**Why:** writing a Postgres wire-protocol implementation is a multi-month project
with a large security surface and no differentiation. Every hour spent there is an
hour not spent on the schema DSL, diffing, and relation loading — which is where
the actual product is.

**Cost:** we inherit sqlx's release cadence and any breaking changes. Mitigated by
an exact version pin in the runtime crate.
