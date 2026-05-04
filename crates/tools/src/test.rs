use std::sync::Arc;

use serde::{Deserialize, Serialize};

use aiken_mcp_core::{AikenRunner, CoreResult, Project, ProjectRoot, TestOutcome};

#[derive(Debug, Clone, Deserialize)]
pub struct TestRequest {
    pub path: String,
    pub filter: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TestResponse {
    pub outcome: TestOutcome,
    pub project_root: String,
}

pub async fn handle_test(
    runner: Arc<dyn AikenRunner>,
    req: TestRequest,
) -> CoreResult<TestResponse> {
    let root = ProjectRoot::discover(&req.path)?;
    let project = Project::new(root.clone());
    let outcome = runner.test(&project, req.filter.as_deref()).await?;
    Ok(TestResponse {
        outcome,
        project_root: root.as_path().display().to_string(),
    })
}
