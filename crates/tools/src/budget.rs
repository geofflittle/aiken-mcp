use std::sync::Arc;

use serde::{Deserialize, Serialize};

use aiken_mcp_core::{AikenRunner, CoreResult, Project, ProjectRoot, TestResult};

/// Cardano Plutus per-tx exec budget. These are the conservative defaults
/// used by Cardano mainnet today. Surfaced so callers can compute pct
/// without baking them into client code.
pub const TX_MEM_LIMIT: u64 = 14_000_000;
pub const TX_CPU_LIMIT: u64 = 10_000_000_000;

#[derive(Debug, Clone, Deserialize)]
pub struct BudgetRequest {
    pub path: String,
    pub filter: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct BudgetEntry {
    pub name: String,
    pub passed: bool,
    pub mem: Option<u64>,
    pub cpu: Option<u64>,
    pub mem_pct_of_tx_limit: Option<f64>,
    pub cpu_pct_of_tx_limit: Option<f64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct BudgetResponse {
    pub project_root: String,
    pub tx_mem_limit: u64,
    pub tx_cpu_limit: u64,
    pub entries: Vec<BudgetEntry>,
}

pub async fn handle_budget(
    runner: Arc<dyn AikenRunner>,
    req: BudgetRequest,
) -> CoreResult<BudgetResponse> {
    let root = ProjectRoot::discover(&req.path)?;
    let project = Project::new(root.clone());
    let outcome = runner.test(&project, req.filter.as_deref()).await?;

    let entries = outcome.tests.iter().map(|t| build_entry(t)).collect();

    Ok(BudgetResponse {
        project_root: root.as_path().display().to_string(),
        tx_mem_limit: TX_MEM_LIMIT,
        tx_cpu_limit: TX_CPU_LIMIT,
        entries,
    })
}

fn build_entry(t: &TestResult) -> BudgetEntry {
    BudgetEntry {
        name: t.name.clone(),
        passed: t.passed,
        mem: t.mem,
        cpu: t.cpu,
        mem_pct_of_tx_limit: t.mem.map(|m| m as f64 / TX_MEM_LIMIT as f64 * 100.0),
        cpu_pct_of_tx_limit: t.cpu.map(|c| c as f64 / TX_CPU_LIMIT as f64 * 100.0),
    }
}
