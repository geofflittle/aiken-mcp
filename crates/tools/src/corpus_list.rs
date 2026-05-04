//! Curated Aiken corpus manifest tool.
//!
//! Reads the embedded `corpora.toml` and returns entries, optionally filtered
//! by tag. Static data, parsed once at startup.

use std::sync::OnceLock;

use serde::{Deserialize, Serialize};

use aiken_mcp_core::CoreResult;

const CORPORA_TOML: &str = include_str!("../data/corpora.toml");

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CorpusEntry {
    pub name: String,
    pub url: String,
    pub author: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct CorporaFile {
    #[serde(default)]
    entries: Vec<CorpusEntry>,
}

fn entries() -> &'static Vec<CorpusEntry> {
    static CACHE: OnceLock<Vec<CorpusEntry>> = OnceLock::new();
    CACHE.get_or_init(|| match toml::from_str::<CorporaFile>(CORPORA_TOML) {
        Ok(f) => f.entries,
        Err(e) => {
            tracing::warn!(error = %e, "failed to parse corpora.toml; corpus list empty");
            Vec::new()
        }
    })
}

#[derive(Debug, Clone, Deserialize)]
pub struct CorpusListRequest {
    /// Optional tag filter; entries must contain this tag (case-insensitive).
    #[serde(default)]
    pub tag: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CorpusListResponse {
    pub entries: Vec<CorpusEntry>,
}

pub async fn handle_corpus_list(req: CorpusListRequest) -> CoreResult<CorpusListResponse> {
    let all = entries();
    let filtered: Vec<CorpusEntry> = match req.tag.as_deref() {
        Some(needle) => {
            let needle = needle.to_ascii_lowercase();
            all.iter()
                .filter(|e| e.tags.iter().any(|t| t.to_ascii_lowercase() == needle))
                .cloned()
                .collect()
        }
        None => all.clone(),
    };
    Ok(CorpusListResponse { entries: filtered })
}
