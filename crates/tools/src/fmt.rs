use std::sync::Arc;

use serde::{Deserialize, Serialize};

use aiken_mcp_core::{AikenRunner, CoreResult, FmtOutcome};

#[derive(Debug, Clone, Deserialize)]
pub struct FmtRequest {
    pub source: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct FmtResponse {
    pub outcome: FmtOutcome,
}

pub async fn handle_fmt(runner: Arc<dyn AikenRunner>, req: FmtRequest) -> CoreResult<FmtResponse> {
    let outcome = runner.fmt(&req.source).await?;
    Ok(FmtResponse { outcome })
}
