use std::sync::Arc;

use serde::{Deserialize, Serialize};

use aiken_mcp_core::diagnostic::{Diagnostic, Severity};
use aiken_mcp_core::{AikenRunner, CheckOutcome, CoreResult, Project, ProjectRoot};

#[derive(Debug, Clone, Deserialize)]
pub struct CheckRequest {
    /// Path inside (or to) an Aiken project. Project root is discovered upward.
    pub path: String,
    /// Optional module filter passed to `aiken check -m`.
    pub module: Option<String>,
    /// Delete `<project>/build` before invoking check. Use when a previous
    /// check reported empty diagnostics or stale state.
    #[serde(default)]
    pub clean: bool,
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
    if req.clean {
        let build_dir = root.as_path().join("build");
        if build_dir.exists() {
            let _ = tokio::fs::remove_dir_all(&build_dir).await;
        }
    }
    let project = Project::new(root.clone());
    let mut outcome = runner.check(&project, req.module.as_deref()).await?;
    if !outcome.success && outcome.diagnostics.is_empty() {
        outcome.diagnostics.push(Diagnostic {
            severity: Severity::Warning,
            message: "aiken check failed but no diagnostics were parsed. \
                      Possible causes: stale build cache (retry with clean: true), \
                      stray .ak files outside lib/validators/ (e.g. benches.ak), \
                      or an unexpected aiken output format. See raw_stderr."
                .to_string(),
            span: None,
            code: None,
        });
    }
    Ok(CheckResponse {
        outcome,
        project_root: root.as_path().display().to_string(),
    })
}
