//! `ruprizzle-lsp` binary entry point.
//!
//! Reads LSP messages from stdin and writes responses to stdout.

#![forbid(unsafe_code)]

#[tokio::main]
async fn main() {
    ruprizzle_lsp::run_stdio().await;
}
