# ADR-002 · Codegen, not proc-macros-over-structs

**Decision:** a CLI generates source files that the user commits (or gitignores).
**Why:** three reasons. Generated code is *readable* — users can open it, understand
it, and debug it, which proc-macro output never is. IDE completion works perfectly
with no macro expansion. And compile times are better, because the user's build
does not run the parser and codegen on every `cargo check`.
**Cost:** an explicit `generate` step. Mitigated by `--watch` (P7-03).
**Rejected alternative:** `#[derive(Model)]` on hand-written structs, SeaORM-style.
That inverts the source of truth — the schema stops being declarative and
migrations can no longer be diffed from it, which forfeits the headline feature.
