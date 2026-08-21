//! Documentation target for the `ruprizzle` command-line interface.
//!
//! This crate ships a binary, not a library API — everything it does lives in
//! `src/main.rs` behind the `ruprizzle` executable. The binary's own rustdoc
//! cannot be published, because its target is named `ruprizzle` and would
//! collide with the [`ruprizzle`](https://docs.rs/ruprizzle) runtime crate's
//! documentation in a shared `target/doc`. Without this file rustdoc is asked
//! to document nothing at all, which docs.rs records as a failed build — see
//! `ProjectPlan/v1/V1StableRelease.md` S1-02.
//!
//! So this module exists to carry the CLI's reference documentation, and
//! nothing else. It exports no items and is not covered by semver; see
//! `docs/Stability.md`.
//!
//! ---
#![doc = include_str!("../README.md")]
#![forbid(unsafe_code)]
