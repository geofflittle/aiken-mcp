use std::path::Path;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::error::CoreResult;

/// One validator entry from a Plutus blueprint (`plutus.json` per CIP-57).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlueprintValidator {
    pub title: String,
    pub hash: Option<String>,
    pub compiled_size_bytes: Option<usize>,
    pub parameters: Vec<BlueprintParam>,
    pub datum: Option<serde_json::Value>,
    pub redeemer: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlueprintParam {
    pub title: String,
    pub schema: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Blueprint {
    pub validators: Vec<BlueprintValidator>,
    pub raw: serde_json::Value,
}

/// Reads a Plutus blueprint produced by `aiken build` from a project root.
#[async_trait]
pub trait BlueprintReader: Send + Sync {
    async fn read(&self, project_root: &Path) -> CoreResult<Blueprint>;
}
