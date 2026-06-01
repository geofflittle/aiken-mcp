//! LSP client wrapping `aiken lsp --stdio`.
//!
//! Single instance per process. We launch the language server lazily on the
//! first request, send `initialize`, then a `didOpen` for any file we touch
//! (the LSP protocol requires open before hover/completions/definition).
//!
//! The client is intentionally minimal: only the three request kinds the
//! MCP exposes (`hover`, `completion`, `definition`). Add more as needed.
//!
//! Threading: a single-threaded reader task owns stdout. Requests get IDs and
//! resolve via a oneshot channel held in a `pending` map. Notifications are
//! discarded (we don't surface diagnostics through the LSP yet — `aiken_check`
//! is a better path for that).

mod transport;

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use lsp_types::request::{Completion, GotoDefinition, HoverRequest, Initialize};
use lsp_types::{
    ClientCapabilities, CompletionContext, CompletionItem, CompletionParams, CompletionResponse,
    CompletionTriggerKind, DidOpenTextDocumentParams, GotoDefinitionParams, GotoDefinitionResponse,
    HoverContents, HoverParams, InitializeParams, InitializeResult, InitializedParams,
    MarkupContent, PartialResultParams, Position, TextDocumentIdentifier, TextDocumentItem,
    TextDocumentPositionParams, Uri, WorkDoneProgressParams,
};
use std::str::FromStr;
use tokio::sync::Mutex;
use tracing::warn;

use aiken_mcp_core::{
    Completion as CoreCompletion, CoreError, CoreResult, Hover as CoreHover, Location as CoreLoc,
    LspClient,
};

use crate::transport::LspTransport;

/// Wraps a single child `aiken lsp --stdio` process.
pub struct AikenLspClient {
    binary: PathBuf,
    state: Arc<Mutex<Option<LspTransport>>>,
    initialized: Arc<AtomicBool>,
    next_id: Arc<AtomicI64>,
}

impl AikenLspClient {
    pub fn new() -> Self {
        Self::with_binary(PathBuf::from("aiken"))
    }

    pub fn with_binary(binary: PathBuf) -> Self {
        Self {
            binary,
            state: Arc::new(Mutex::new(None)),
            initialized: Arc::new(AtomicBool::new(false)),
            next_id: Arc::new(AtomicI64::new(1)),
        }
    }

    fn next_id(&self) -> i64 {
        self.next_id.fetch_add(1, Ordering::Relaxed)
    }

    /// Lazy-spawn + initialize. Returns a clone of the transport.
    async fn ensure_started(&self) -> CoreResult<LspTransport> {
        let mut guard = self.state.lock().await;
        if let Some(t) = guard.as_ref() {
            return Ok(t.clone());
        }

        let transport = LspTransport::spawn(&self.binary, &["lsp", "--stdio"]).await?;
        *guard = Some(transport.clone());
        drop(guard);

        if !self.initialized.load(Ordering::Acquire) {
            let id = self.next_id();
            let params = InitializeParams {
                capabilities: ClientCapabilities::default(),
                ..Default::default()
            };
            let _: InitializeResult = transport
                .request::<Initialize>(id, params, Duration::from_secs(15))
                .await?;
            transport
                .notify("initialized", InitializedParams {})
                .await?;
            self.initialized.store(true, Ordering::Release);
        }

        Ok(transport)
    }

    async fn ensure_doc_open(&self, transport: &LspTransport, file: &Path) -> CoreResult<()> {
        let uri = file_url(file)?;
        let text = tokio::fs::read_to_string(file)
            .await
            .map_err(CoreError::Io)?;
        let params = DidOpenTextDocumentParams {
            text_document: TextDocumentItem {
                uri,
                language_id: "aiken".to_string(),
                version: 1,
                text,
            },
        };
        transport.notify("textDocument/didOpen", params).await
    }
}

impl Default for AikenLspClient {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl LspClient for AikenLspClient {
    async fn hover(&self, file: &Path, line: u32, column: u32) -> CoreResult<Option<CoreHover>> {
        let transport = self.ensure_started().await?;
        self.ensure_doc_open(&transport, file).await?;

        let uri = file_url(file)?;
        let params = HoverParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position: Position {
                    line,
                    character: column,
                },
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
        };
        let id = self.next_id();
        let resp: Option<lsp_types::Hover> = transport
            .request::<HoverRequest>(id, params, Duration::from_secs(10))
            .await?;
        Ok(resp.map(|h| CoreHover {
            markdown: hover_to_markdown(&h.contents),
            range: None,
        }))
    }

    async fn completions(
        &self,
        file: &Path,
        line: u32,
        column: u32,
    ) -> CoreResult<Vec<CoreCompletion>> {
        let transport = self.ensure_started().await?;
        self.ensure_doc_open(&transport, file).await?;

        let uri = file_url(file)?;
        let params = CompletionParams {
            text_document_position: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position: Position {
                    line,
                    character: column,
                },
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
            context: Some(CompletionContext {
                trigger_kind: CompletionTriggerKind::INVOKED,
                trigger_character: None,
            }),
        };
        let id = self.next_id();
        let resp: Option<CompletionResponse> = transport
            .request::<Completion>(id, params, Duration::from_secs(10))
            .await?;
        let items: Vec<CompletionItem> = match resp {
            Some(CompletionResponse::Array(items)) => items,
            Some(CompletionResponse::List(list)) => list.items,
            None => Vec::new(),
        };
        Ok(items
            .into_iter()
            .map(|i| CoreCompletion {
                label: i.label,
                kind: i.kind.map(|k| format!("{:?}", k)),
                detail: i.detail,
                doc: i.documentation.map(doc_to_string),
            })
            .collect())
    }

    async fn definition(&self, file: &Path, line: u32, column: u32) -> CoreResult<Vec<CoreLoc>> {
        let transport = self.ensure_started().await?;
        self.ensure_doc_open(&transport, file).await?;

        let uri = file_url(file)?;
        let params = GotoDefinitionParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position: Position {
                    line,
                    character: column,
                },
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
        };
        let id = self.next_id();
        let resp: Option<GotoDefinitionResponse> = transport
            .request::<GotoDefinition>(id, params, Duration::from_secs(10))
            .await?;
        let mut out = Vec::new();
        match resp {
            Some(GotoDefinitionResponse::Scalar(loc)) => out.push(to_core_loc(&loc)),
            Some(GotoDefinitionResponse::Array(locs)) => {
                out.extend(locs.iter().map(to_core_loc));
            }
            Some(GotoDefinitionResponse::Link(links)) => {
                for link in links {
                    out.push(CoreLoc {
                        file: link.target_uri.path().as_str().to_string(),
                        line: link.target_range.start.line,
                        column: link.target_range.start.character,
                    });
                }
            }
            None => {}
        }
        Ok(out)
    }
}

fn file_url(path: &Path) -> CoreResult<Uri> {
    let abs = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir().map_err(CoreError::Io)?.join(path)
    };
    let url = url::Url::from_file_path(&abs).map_err(|_| {
        warn!(path = %abs.display(), "Url::from_file_path failed");
        CoreError::other(format!("could not convert path to URL: {}", abs.display()))
    })?;
    Uri::from_str(url.as_str())
        .map_err(|e| CoreError::other(format!("could not parse file URL as Uri: {e}")))
}

fn to_core_loc(loc: &lsp_types::Location) -> CoreLoc {
    CoreLoc {
        file: loc.uri.path().as_str().to_string(),
        line: loc.range.start.line,
        column: loc.range.start.character,
    }
}

fn hover_to_markdown(contents: &HoverContents) -> String {
    match contents {
        HoverContents::Markup(MarkupContent { value, .. }) => value.clone(),
        HoverContents::Scalar(s) => match s {
            lsp_types::MarkedString::String(v) => v.clone(),
            lsp_types::MarkedString::LanguageString(ls) => {
                format!("```{}\n{}\n```", ls.language, ls.value)
            }
        },
        HoverContents::Array(arr) => arr
            .iter()
            .map(|s| match s {
                lsp_types::MarkedString::String(v) => v.clone(),
                lsp_types::MarkedString::LanguageString(ls) => {
                    format!("```{}\n{}\n```", ls.language, ls.value)
                }
            })
            .collect::<Vec<_>>()
            .join("\n\n"),
    }
}

fn doc_to_string(d: lsp_types::Documentation) -> String {
    match d {
        lsp_types::Documentation::String(s) => s,
        lsp_types::Documentation::MarkupContent(m) => m.value,
    }
}
