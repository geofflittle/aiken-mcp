use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::error::CoreResult;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CorpusHit {
    pub corpus: String,
    pub file: String,
    pub line: u32,
    pub snippet: String,
}

#[async_trait]
pub trait CorpusSearch: Send + Sync {
    async fn search(&self, query: &str, max_hits: usize) -> CoreResult<Vec<CorpusHit>>;
}
