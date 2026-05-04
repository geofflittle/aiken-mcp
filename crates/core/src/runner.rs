use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::diagnostic::Diagnostic;
use crate::error::CoreResult;
use crate::project::Project;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckOutcome {
    pub success: bool,
    pub diagnostics: Vec<Diagnostic>,
    pub raw_stdout: String,
    pub raw_stderr: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildOutcome {
    pub success: bool,
    pub diagnostics: Vec<Diagnostic>,
    pub artifacts: Vec<String>,
    pub raw_stdout: String,
    pub raw_stderr: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestResult {
    pub name: String,
    pub passed: bool,
    pub mem: Option<u64>,
    pub cpu: Option<u64>,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestOutcome {
    pub success: bool,
    pub tests: Vec<TestResult>,
    pub raw_stdout: String,
    pub raw_stderr: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FmtOutcome {
    pub success: bool,
    pub formatted_source: Option<String>,
    pub raw_stderr: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UplcOutcome {
    pub success: bool,
    pub uplc: String,
    pub raw_stderr: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewProjectOutcome {
    pub success: bool,
    pub created_path: Option<String>,
    pub raw_stdout: String,
    pub raw_stderr: String,
}

/// Abstraction over how Aiken commands are executed. Lets tests inject a fake
/// runner without running real subprocess work.
#[async_trait]
pub trait AikenRunner: Send + Sync {
    async fn check(&self, project: &Project, filter: Option<&str>) -> CoreResult<CheckOutcome>;
    async fn build(&self, project: &Project) -> CoreResult<BuildOutcome>;
    async fn test(&self, project: &Project, filter: Option<&str>) -> CoreResult<TestOutcome>;
    async fn fmt(&self, source: &str) -> CoreResult<FmtOutcome>;
    async fn uplc_decode(&self, project: &Project, target: &str) -> CoreResult<UplcOutcome>;
    async fn new_project(&self, parent_dir: &str, name: &str) -> CoreResult<NewProjectOutcome>;
    async fn version(&self) -> CoreResult<String>;
}
