use std::sync::Arc;

use serde::{Deserialize, Serialize};

use aiken_mcp_core::{AikenRunner, CheckOutcome, CoreResult, Project, ProjectRoot};

#[derive(Debug, Clone, Deserialize)]
pub struct CheckRequest {
    /// Path inside (or to) an Aiken project. Project root is discovered upward.
    pub path: String,
    /// Optional module filter passed to `aiken check -m`.
    pub module: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CheckResponse {
    pub outcome: CheckOutcome,
    pub project_root: String,
}

pub async fn handle_check(
    runner: Arc<dyn AikenRunner>,
    req: CheckRequest,
) -> CoreResult<CheckResponse> {
    let root = ProjectRoot::discover(&req.path)?;
    let project = Project::new(root.clone());
    let outcome = runner.check(&project, req.module.as_deref()).await?;
    Ok(CheckResponse {
        outcome,
        project_root: root.as_path().display().to_string(),
    })
}
