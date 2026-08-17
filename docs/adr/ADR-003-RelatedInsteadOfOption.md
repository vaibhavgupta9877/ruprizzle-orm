# ADR-003: `Related<T>` instead of `Option<T>` for relations

**Date:** 2026-08-13  
**Status:** Accepted

**Decision:** a three-state-by-construction type distinguishing "not loaded" from
"loaded and empty."

**Why:** `Option<Vec<Post>>` makes `None` ambiguous, and the ambiguity produces a
silent wrong answer rather than an error. Loud failure with an actionable message
beats a quiet bug.

**Cost:** one unfamiliar type in the public API, and one sanctioned panic.
