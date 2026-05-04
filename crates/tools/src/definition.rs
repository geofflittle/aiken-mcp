use std::path::PathBuf;
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use aiken_mcp_core::{CoreResult, Location, LspClient};

#[derive(Debug, Clone, Deserialize)]
pub struct DefinitionRequest {
    pub file: String,
    pub line: u32,
    pub column: u32,
}

#[derive(Debug, Clone, Serialize)]
pub struct DefinitionResponse {
    pub locations: Vec<Location>,
}

pub async fn handle_definition(
    lsp: Arc<dyn LspClient>,
    req: DefinitionRequest,
) -> CoreResult<DefinitionResponse> {
    let path = PathBuf::from(req.file);
    let locations = lsp.definition(&path, req.line, req.column).await?;
    Ok(DefinitionResponse { locations })
}
