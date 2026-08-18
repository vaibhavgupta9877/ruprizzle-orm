//! Language server for `schema.ruprizzle`.
//!
//! Implements the minimal LSP surface needed by the editor extension:
//! diagnostics, completion, go-to-definition and hover. The server stays small
//! by re-using the existing parser and core IR; it does not generate code or
//! touch the database.

#![forbid(unsafe_code)]
#![warn(missing_docs, clippy::pedantic)]

use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::Mutex;
use tower_lsp::jsonrpc::Result;
#[allow(clippy::wildcard_imports)]
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer, LspService, Server};

pub mod completion;
pub mod diagnostics;
pub mod goto;
pub mod hover;

/// LSP server state.
pub struct Backend {
    /// Connection to the editor client.
    pub client: Client,
    /// Map of open document URIs to their current text.
    documents: Arc<Mutex<HashMap<Url, String>>>,
}

impl Backend {
    /// Create a new backend for the given client.
    #[must_use]
    pub fn new(client: Client) -> Self {
        Self {
            client,
            documents: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Parse a document and publish diagnostics to the client.
    async fn validate_and_publish(&self, uri: &Url, text: &str, version: Option<i32>) {
        let file_name = uri.path();
        let mut diags = Vec::new();

        match ruprizzle_parser::parse_with_warnings(file_name, text) {
            Ok((_schema, warnings)) => {
                for warning in warnings {
                    diags.push(diagnostics::schema_error_to_diagnostic(text, &warning));
                }
            }
            Err(errors) => {
                for error in &errors.errors {
                    diags.push(diagnostics::schema_error_to_diagnostic(text, error));
                }
            }
        }

        self.client
            .publish_diagnostics(uri.clone(), diags, version)
            .await;
    }
}

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    async fn initialize(&self, _: InitializeParams) -> Result<InitializeResult> {
        Ok(InitializeResult {
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Kind(
                    TextDocumentSyncKind::INCREMENTAL,
                )),
                completion_provider: Some(CompletionOptions {
                    trigger_characters: Some(vec!["@".to_string(), ".".to_string()]),
                    ..CompletionOptions::default()
                }),
                definition_provider: Some(OneOf::Left(true)),
                hover_provider: Some(HoverProviderCapability::Simple(true)),
                ..ServerCapabilities::default()
            },
            server_info: Some(ServerInfo {
                name: "ruprizzle-lsp".to_owned(),
                version: Some(env!("CARGO_PKG_VERSION").to_owned()),
            }),
        })
    }

    async fn initialized(&self, _: InitializedParams) {
        self.client
            .log_message(MessageType::INFO, "ruprizzle-lsp initialised")
            .await;
    }

    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        let uri = params.text_document.uri;
        let text = params.text_document.text;
        let version = Some(params.text_document.version);

        let mut docs = self.documents.lock().await;
        docs.insert(uri.clone(), text.clone());
        drop(docs);

        self.validate_and_publish(&uri, &text, version).await;
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        let uri = params.text_document.uri;
        let version = Some(params.text_document.version);

        let mut docs = self.documents.lock().await;
        let text = docs.get(&uri).cloned().unwrap_or_default();

        let mut text = text;
        for change in params.content_changes {
            if let Some(range) = change.range {
                text = apply_change(&text, range, &change.text);
            } else {
                text = change.text;
            }
        }
        docs.insert(uri.clone(), text.clone());
        drop(docs);

        self.validate_and_publish(&uri, &text, version).await;
    }

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        let mut docs = self.documents.lock().await;
        docs.remove(&params.text_document.uri);
        self.client
            .publish_diagnostics(params.text_document.uri, Vec::new(), None)
            .await;
    }

    async fn completion(&self, params: CompletionParams) -> Result<Option<CompletionResponse>> {
        let docs = self.documents.lock().await;
        let Some(text) = docs.get(&params.text_document_position.text_document.uri) else {
            return Ok(None);
        };
        let text = text.clone();
        drop(docs);

        let uri = &params.text_document_position.text_document.uri;
        let file_name = uri.path();
        let schema = ruprizzle_parser::parse(file_name, &text).ok();
        let position = params.text_document_position.position;

        Ok(completion::complete(&text, schema.as_ref(), position))
    }

    async fn goto_definition(
        &self,
        params: GotoDefinitionParams,
    ) -> Result<Option<GotoDefinitionResponse>> {
        let docs = self.documents.lock().await;
        let Some(text) = docs.get(&params.text_document_position_params.text_document.uri) else {
            return Ok(None);
        };
        let text = text.clone();
        drop(docs);

        let uri = &params.text_document_position_params.text_document.uri;
        let file_name = uri.path();
        let schema = ruprizzle_parser::parse(file_name, &text).ok();
        let position = params.text_document_position_params.position;

        Ok(goto::goto_definition(uri, &text, schema.as_ref(), position))
    }

    async fn hover(&self, params: HoverParams) -> Result<Option<Hover>> {
        let docs = self.documents.lock().await;
        let Some(text) = docs.get(&params.text_document_position_params.text_document.uri) else {
            return Ok(None);
        };
        let text = text.clone();
        drop(docs);

        let file_name = params
            .text_document_position_params
            .text_document
            .uri
            .path();
        let schema = ruprizzle_parser::parse(file_name, &text).ok();
        let position = params.text_document_position_params.position;

        Ok(hover::hover(&text, schema.as_ref(), position))
    }
}

fn apply_change(text: &str, range: Range, new: &str) -> String {
    let start = position_to_byte_offset(text, range.start);
    let end = position_to_byte_offset(text, range.end);
    let mut result = text.to_owned();
    result.replace_range(start..end, new);
    result
}

fn position_to_byte_offset(text: &str, pos: Position) -> usize {
    let mut line = 0;
    let mut col = 0;
    for (i, c) in text.char_indices() {
        if line == pos.line && col == pos.character {
            return i;
        }
        if c == '\n' {
            line += 1;
            col = 0;
        } else {
            col += 1;
        }
    }
    text.len()
}

/// Run the language server over stdio.
pub async fn run_stdio() {
    let (stdin, stdout) = (tokio::io::stdin(), tokio::io::stdout());
    let (service, socket) = LspService::new(Backend::new);
    Server::new(stdin, stdout, socket).serve(service).await;
}
