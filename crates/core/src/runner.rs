use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::diagnostic::Diagnostic;
use crate::error::CoreResult;
use crate::project::Project;

/// Cardano Plutus per-tx exec budget. Conservative mainnet defaults, used to
/// compute per-test mem/cpu percentages.
pub const TX_MEM_LIMIT: u64 = 14_000_000;
pub const TX_CPU_LIMIT: u64 = 10_000_000_000;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckOutcome {
    pub success: bool,
    pub diagnostics: Vec<Diagnostic>,
    pub tests: Vec<TestResult>,
    pub tx_mem_limit: u64,
    pub tx_cpu_limit: u64,
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
    pub mem_pct_of_tx_limit: Option<f64>,
    pub cpu_pct_of_tx_limit: Option<f64>,
    pub message: Option<String>,
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
    async fn fmt(&self, source: &str) -> CoreResult<FmtOutcome>;
    async fn uplc_decode(&self, project: &Project, target: &str) -> CoreResult<UplcOutcome>;
    async fn new_project(&self, parent_dir: &str, name: &str) -> CoreResult<NewProjectOutcome>;
    async fn version(&self) -> CoreResult<String>;
}
