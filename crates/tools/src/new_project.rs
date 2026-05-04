use std::sync::Arc;

use serde::{Deserialize, Serialize};

use aiken_mcp_core::{AikenRunner, CoreResult, NewProjectOutcome};

#[derive(Debug, Clone, Deserialize)]
pub struct NewProjectRequest {
    pub parent_dir: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct NewProjectResponse {
    pub outcome: NewProjectOutcome,
}

pub async fn handle_new_project(
    runner: Arc<dyn AikenRunner>,
    req: NewProjectRequest,
) -> CoreResult<NewProjectResponse> {
    let outcome = runner.new_project(&req.parent_dir, &req.name).await?;
    Ok(NewProjectResponse { outcome })
}
