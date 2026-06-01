use std::path::Path;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::diagnostic::SourceSpan;
use crate::error::CoreResult;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Hover {
    pub markdown: String,
    pub range: Option<SourceSpan>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Completion {
    pub label: String,
    pub kind: Option<String>,
    pub detail: Option<String>,
    pub doc: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Location {
    pub file: String,
    pub line: u32,
    pub column: u32,
}

/// Abstraction over an Aiken language server. The concrete impl in
/// `aiken-mcp-lsp` shells out to `aiken lsp --stdio` and speaks
/// JSON-RPC over its stdio.
#[async_trait]
pub trait LspClient: Send + Sync {
    async fn hover(&self, file: &Path, line: u32, column: u32) -> CoreResult<Option<Hover>>;
    async fn completions(&self, file: &Path, line: u32, column: u32)
        -> CoreResult<Vec<Completion>>;
    async fn definition(&self, file: &Path, line: u32, column: u32) -> CoreResult<Vec<Location>>;
}
