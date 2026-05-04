use std::sync::Arc;

use serde::{Deserialize, Serialize};

use aiken_mcp_core::{CoreResult, DocsFetcher};

#[derive(Debug, Clone, Deserialize)]
pub struct DocsLookupRequest {
    /// Relative path under the docs base URL, e.g. `language-tour/types`.
    pub path: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct DocsLookupResponse {
    pub body: String,
}

pub async fn handle_docs_lookup(
    docs: Arc<dyn DocsFetcher>,
    req: DocsLookupRequest,
) -> CoreResult<DocsLookupResponse> {
    let body = docs.fetch(&req.path).await?;
    Ok(DocsLookupResponse { body })
}
