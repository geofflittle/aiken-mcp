use std::sync::Arc;

use serde::{Deserialize, Serialize};

use aiken_mcp_core::{CoreResult, CorpusHit, CorpusSearch};

#[derive(Debug, Clone, Deserialize)]
pub struct SearchRequest {
    pub query: String,
    #[serde(default = "default_max_hits")]
    pub max_hits: usize,
}

fn default_max_hits() -> usize {
    20
}

#[derive(Debug, Clone, Serialize)]
pub struct SearchResponse {
    pub hits: Vec<CorpusHit>,
}

pub async fn handle_search(
    corpus: Arc<dyn CorpusSearch>,
    req: SearchRequest,
) -> CoreResult<SearchResponse> {
    let hits = corpus.search(&req.query, req.max_hits).await?;
    Ok(SearchResponse { hits })
}
