use std::sync::Arc;

use serde::{Deserialize, Serialize};

use aiken_mcp_core::{Blueprint, BlueprintReader, CoreResult, ProjectRoot};

#[derive(Debug, Clone, Deserialize)]
pub struct BlueprintRequest {
    pub path: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct BlueprintResponse {
    pub blueprint: Blueprint,
    pub project_root: String,
}

pub async fn handle_blueprint(
    reader: Arc<dyn BlueprintReader>,
    req: BlueprintRequest,
) -> CoreResult<BlueprintResponse> {
    let root = ProjectRoot::discover(&req.path)?;
    let blueprint = reader.read(root.as_path()).await?;
    Ok(BlueprintResponse {
        blueprint,
        project_root: root.as_path().display().to_string(),
    })
}
