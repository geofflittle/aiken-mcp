use std::sync::Arc;

use serde::{Deserialize, Serialize};

use aiken_mcp_core::{AikenRunner, CoreResult, Project, ProjectRoot, UplcOutcome};

#[derive(Debug, Clone, Deserialize)]
pub struct UplcRequest {
    pub path: String,
    /// Path to a `.cbor` UPLC file or other artifact to decode.
    pub target: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct UplcResponse {
    pub outcome: UplcOutcome,
    pub project_root: String,
}

pub async fn handle_uplc(
    runner: Arc<dyn AikenRunner>,
    req: UplcRequest,
) -> CoreResult<UplcResponse> {
    let root = ProjectRoot::discover(&req.path)?;
    let project = Project::new(root.clone());
    let outcome = runner.uplc_decode(&project, &req.target).await?;
    Ok(UplcResponse {
        outcome,
        project_root: root.as_path().display().to_string(),
    })
}
