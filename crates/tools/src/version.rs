use std::sync::Arc;

use serde::Serialize;

use aiken_mcp_core::{AikenRunner, CoreResult};

#[derive(Debug, Clone, Serialize)]
pub struct VersionResponse {
    pub version: String,
}

pub async fn handle_version(runner: Arc<dyn AikenRunner>) -> CoreResult<VersionResponse> {
    let version = runner.version().await?;
    Ok(VersionResponse { version })
}
