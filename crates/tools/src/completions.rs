use std::path::PathBuf;
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use aiken_mcp_core::{Completion, CoreResult, LspClient};

#[derive(Debug, Clone, Deserialize)]
pub struct CompletionsRequest {
    pub file: String,
    pub line: u32,
    pub column: u32,
}

#[derive(Debug, Clone, Serialize)]
pub struct CompletionsResponse {
    pub items: Vec<Completion>,
}

pub async fn handle_completions(
    lsp: Arc<dyn LspClient>,
    req: CompletionsRequest,
) -> CoreResult<CompletionsResponse> {
    let path = PathBuf::from(req.file);
    let items = lsp.completions(&path, req.line, req.column).await?;
    Ok(CompletionsResponse { items })
}
