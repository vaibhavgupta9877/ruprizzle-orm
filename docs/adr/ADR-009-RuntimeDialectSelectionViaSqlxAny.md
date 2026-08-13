# ADR-009 · Runtime dialect selection via `sqlx::Any`

**Status:** Accepted, with costs recorded.

**Context.** The product promises one identical Rust API across Postgres and
SQLite, with the backend chosen by URL scheme at runtime. `sqlx::Any` is the
only sqlx facility that provides this without generating a separate client per
dialect.

**Decision.** Route all runtime queries through `sqlx::Any`.

**Consequences.**

- `Any` implements neither `Encode` nor `Decode` for rich types, so `Uuid`,
  `Decimal`, `DateTime`, `Date`, `Time`, and `Json` are serialised to text
  outbound (`crates/runtime/src/value.rs`) and parsed from text inbound
  (`crates/runtime/src/decode.rs`), on every row.
- Comparisons on rich-typed columns rely on server-side parameter inference. If
  inference resolves to `text` rather than the column type, the comparison stops
  using the index — a silent performance cliff, not an error.
- `DateTime` correctness depends on server `DateStyle` and session timezone.
- Postgres arrays, `LISTEN`/`NOTIFY`, `COPY`, and composite types are
  unreachable. Array binds are rejected at runtime.
- The abstraction has leaked repeatedly: three of the last four commits before
  this plan were fixes in this layer.

**Revisit when** any of: a third dialect is added; benchmarks (PR-12) show the
text round-trip exceeding the P8-02 thresholds; or users need Postgres arrays.
The exit is generating dialect-specific native code paths behind a feature flag,
which is additive but costs the runtime-selection property.
