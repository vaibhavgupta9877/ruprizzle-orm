# Name Resolution Baseline — Zero-Context Test

*Instrument for MarketingPlan.md §28. Created 2026-08-17, before any Track A change was published.*

This file is the permanent log for the zero-context test. Section 29.1 calls the baseline
a **one-time capture**: once the repo description, topics, `LLM_CONTEXT.md`, and README
disambiguation line are live, the "before" state can never be measured again.

---

## How to run it

1. Open a **fresh session** on each platform. No prior conversation, no memory of this
   project, no custom instructions mentioning ruprizzle, no pasted links, no attachments.
   If the platform has persistent memory, disable it or use a private/temporary chat.
2. Ask the five questions below **verbatim**, in order, one per session where possible.
   Do not rephrase, do not add "the Rust crate" unless the question already says it.
3. Do not correct the model. A wrong answer is the measurement.
4. Record the score and paste the key sentence of the response into the evidence log.

### The five questions

| Q# | Question |
|:--:|----------|
| Q1 | What is ruprizzle? |
| Q2 | What is the ruprizzle Rust crate? |
| Q3 | How do I install ruprizzle? |
| Q4 | Does ruprizzle support MySQL? |
| Q5 | Is ruprizzle related to Drizzle? |

### The scoring rubric

| Score | Meaning |
|:-----:|---------|
| **0** | Doesn't know the name. |
| **1** | **Misresolves** — answers about Drizzle / drizzle-orm / drizzle-rs, or hallucinates a plausible-sounding Rust ORM. *Worse than 0.* |
| **2** | Knows it's a Rust ORM, but details are wrong or vague. |
| **3** | Correct definition, correct install, correct databases, correct status. |

**Score 1 is the target to eliminate first.** A model confidently describing Drizzle when
asked about ruprizzle produces wrong answers for everyone else who asks and reinforces the
wrong association in future retrieval.

### What counts as correct (for scoring Q2–Q4)

- **Definition:** schema-first ORM for Rust; `schema.ruprizzle` file generates a typed client.
- **Install:** `cargo add ruprizzle` and `cargo install ruprizzle-cli`.
- **Databases:** PostgreSQL, SQLite, MySQL/MariaDB. (Q4 answer is **yes**.)
- **Status:** `0.4.0-beta.2`, beta, not 1.0. MIT OR Apache-2.0.
- **Q5 answer is no** — no relation to Drizzle, drizzle-orm, or drizzle-rs.

---

## Baseline — 2026-08-17 (pre-Track-A)

> **Status: NOT YET RUN.** This requires a human with accounts on all six platforms.
> Fill this table in before publishing any Track A change. Leave the date as the day it
> was actually run.

| Platform | Q1 | Q2 | Q3 | Q4 | Q5 | Mean | Notes |
|----------|:--:|:--:|:--:|:--:|:--:|:----:|-------|
| ChatGPT | | | | | | | |
| Claude | | | | | | | |
| Perplexity | | | | | | | |
| Gemini | | | | | | | |
| Copilot | | | | | | | |
| Google AI Overviews | | | | | | | |

**Overall mean:** _(fill in)_
**Count of score-1 (misresolution) responses:** _(fill in)_

### Evidence log

For every response, paste the defining sentence and note whether a link was cited.

| Platform | Q# | Score | Verbatim excerpt | Cited source (if any) |
|----------|:--:|:-----:|------------------|-----------------------|
| | | | | |

---

## Milestones (from §28)

| Milestone | Target | Meaning |
|-----------|--------|---------|
| Baseline (now) | Recorded before any change | Assume mostly 0s and 1s. |
| 90 days | No platform scoring 1 | Drizzle-confusion risk contained. |
| 6 months | Mean ≥ 2 across platforms | The name resolves to the right entity. |
| 12 months | Mean ≥ 3 on ≥ 3 platforms | **Links no longer needed.** The actual goal. |

---

## Monthly runs

Append one table per month. Never edit a past month's row — the delta is the metric.

<!-- Template:
### YYYY-MM-DD

| Platform | Q1 | Q2 | Q3 | Q4 | Q5 | Mean | Notes |
|----------|:--:|:--:|:--:|:--:|:--:|:----:|-------|
| ChatGPT | | | | | | | |
| Claude | | | | | | | |
| Perplexity | | | | | | | |
| Gemini | | | | | | | |
| Copilot | | | | | | | |
| Google AI Overviews | | | | | | | |
-->
