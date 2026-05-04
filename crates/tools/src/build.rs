use std::sync::Arc;

use serde::{Deserialize, Serialize};

use aiken_mcp_core::{AikenRunner, BuildOutcome, CoreResult, Project, ProjectRoot};

#[derive(Debug, Clone, Deserialize)]
pub struct BuildRequest {
    pub path: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct BuildResponse {
    pub outcome: BuildOutcome,
    pub project_root: String,
}

pub async fn handle_build(
    runner: Arc<dyn AikenRunner>,
    req: BuildRequest,
) -> CoreResult<BuildResponse> {
    let root = ProjectRoot::discover(&req.path)?;
    let project = Project::new(root.clone());
    let outcome = runner.build(&project).await?;
    Ok(BuildResponse {
        outcome,
        project_root: root.as_path().display().to_string(),
    })
}
