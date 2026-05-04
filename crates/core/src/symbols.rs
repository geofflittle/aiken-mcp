use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::error::CoreResult;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SymbolKind {
    Fn,
    Type,
    Const,
    Validator,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Symbol {
    pub name: String,
    pub kind: SymbolKind,
    pub module: String,
    pub signature: String,
    pub file: String,
    pub line: u32,
    /// Doc comment (`///` lines) immediately preceding the declaration, if any.
    /// Joined with newlines; leading triple-slash markers stripped.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub doc: Option<String>,
}

#[async_trait]
pub trait SymbolIndex: Send + Sync {
    /// Lookup symbols whose `name` OR `doc` text contains `query`
    /// (case-insensitive). Up to `max_hits` results.
    async fn lookup(&self, query: &str, max_hits: usize) -> CoreResult<Vec<Symbol>>;
}
