use std::sync::Arc;

use serde::{Deserialize, Serialize};

use aiken_mcp_core::{CoreResult, Symbol, SymbolIndex};

#[derive(Debug, Clone, Deserialize)]
pub struct SymbolLookupRequest {
    pub query: String,
    #[serde(default = "default_max_hits")]
    pub max_hits: usize,
}

fn default_max_hits() -> usize {
    20
}

#[derive(Debug, Clone, Serialize)]
pub struct SymbolLookupResponse {
    pub symbols: Vec<Symbol>,
}

pub async fn handle_symbol_lookup(
    index: Arc<dyn SymbolIndex>,
    req: SymbolLookupRequest,
) -> CoreResult<SymbolLookupResponse> {
    let symbols = index.lookup(&req.query, req.max_hits).await?;
    Ok(SymbolLookupResponse { symbols })
}
