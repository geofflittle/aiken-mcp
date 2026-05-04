use std::path::PathBuf;
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use aiken_mcp_core::{CoreResult, Hover, LspClient};

#[derive(Debug, Clone, Deserialize)]
pub struct HoverRequest {
    pub file: String,
    pub line: u32,
    pub column: u32,
}

#[derive(Debug, Clone, Serialize)]
pub struct HoverResponse {
    pub hover: Option<Hover>,
}

pub async fn handle_hover(
    lsp: Arc<dyn LspClient>,
    req: HoverRequest,
) -> CoreResult<HoverResponse> {
    let path = PathBuf::from(req.file);
    let hover = lsp.hover(&path, req.line, req.column).await?;
    Ok(HoverResponse { hover })
}
