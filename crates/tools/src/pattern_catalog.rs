//! Curated pattern catalog tool.
//!
//! Reads the embedded `patterns.toml`. Supports lookup by name and search by
//! keyword (matched against name/title/description/keywords). Each pattern
//! carries refs into the corpus so consumers can read the canonical impl.

use std::sync::OnceLock;

use serde::{Deserialize, Serialize};

use aiken_mcp_core::CoreResult;

const PATTERNS_TOML: &str = include_str!("../data/patterns.toml");

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PatternRef {
    pub repo: String,
    pub path: String,
    #[serde(default)]
    pub lines: Option<String>,
    #[serde(default)]
    pub author: Option<String>,
    #[serde(default)]
    pub note: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Pattern {
    pub name: String,
    pub title: String,
    pub description: String,
    #[serde(default)]
    pub keywords: Vec<String>,
    #[serde(default)]
    pub refs: Vec<PatternRef>,
    #[serde(default)]
    pub gotchas: Option<String>,
    #[serde(default)]
    pub alternatives: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct PatternsFile {
    #[serde(default)]
    patterns: Vec<Pattern>,
}

fn patterns() -> &'static Vec<Pattern> {
    static CACHE: OnceLock<Vec<Pattern>> = OnceLock::new();
    CACHE.get_or_init(|| match toml::from_str::<PatternsFile>(PATTERNS_TOML) {
        Ok(f) => f.patterns,
        Err(e) => {
            tracing::warn!(error = %e, "failed to parse patterns.toml; catalog empty");
            Vec::new()
        }
    })
}

#[derive(Debug, Clone, Deserialize)]
pub struct PatternCatalogRequest {
    /// Optional pattern slug for exact match (returns one entry).
    #[serde(default)]
    pub name: Option<String>,
    /// Optional fuzzy query matched against name/title/description/keywords.
    #[serde(default)]
    pub query: Option<String>,
    #[serde(default = "default_max")]
    pub max_hits: usize,
}

fn default_max() -> usize {
    20
}

#[derive(Debug, Clone, Serialize)]
pub struct PatternCatalogResponse {
    pub patterns: Vec<Pattern>,
}

pub async fn handle_pattern_catalog(
    req: PatternCatalogRequest,
) -> CoreResult<PatternCatalogResponse> {
    let all = patterns();

    if let Some(name) = req.name.as_deref() {
        let needle = name.to_ascii_lowercase();
        let matches: Vec<Pattern> = all
            .iter()
            .filter(|p| p.name.to_ascii_lowercase() == needle)
            .cloned()
            .collect();
        return Ok(PatternCatalogResponse { patterns: matches });
    }

    let q = req.query.as_deref().map(|s| s.to_ascii_lowercase());
    let mut hits: Vec<Pattern> = match q.as_deref() {
        Some(needle) => all
            .iter()
            .filter(|p| matches_query(p, needle))
            .cloned()
            .collect(),
        None => all.clone(),
    };
    hits.truncate(req.max_hits);
    Ok(PatternCatalogResponse { patterns: hits })
}

fn matches_query(p: &Pattern, needle: &str) -> bool {
    let lowered = |s: &str| s.to_ascii_lowercase();
    if lowered(&p.name).contains(needle)
        || lowered(&p.title).contains(needle)
        || lowered(&p.description).contains(needle)
    {
        return true;
    }
    p.keywords.iter().any(|k| lowered(k).contains(needle))
}
