//! Static error-explainer table.
//!
//! Each entry maps a regex pattern to a canonical fix-shaped explanation.
//! Patterns are matched against the user-supplied error string in order;
//! the first match wins. New entries should be added to `data/errors.toml`.

use std::sync::OnceLock;

use regex::Regex;
use serde::{Deserialize, Serialize};

use aiken_mcp_core::CoreResult;

const ERRORS_TOML: &str = include_str!("../data/errors.toml");

#[derive(Debug, Clone, Deserialize)]
struct ExplainerFile {
    #[serde(default)]
    entries: Vec<ExplainerEntry>,
}

#[derive(Debug, Clone, Deserialize)]
struct ExplainerEntry {
    pattern: String,
    title: String,
    explanation: String,
    #[serde(default)]
    fix: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ExplainRequest {
    pub error: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ExplainResponse {
    pub matched: Option<ExplainHit>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ExplainHit {
    pub title: String,
    pub explanation: String,
    pub fix: Option<String>,
}

struct Compiled {
    regex: Regex,
    entry: ExplainerEntry,
}

fn entries() -> &'static Vec<Compiled> {
    static CACHE: OnceLock<Vec<Compiled>> = OnceLock::new();
    CACHE.get_or_init(|| {
        let parsed: ExplainerFile = match toml::from_str(ERRORS_TOML) {
            Ok(p) => p,
            Err(e) => {
                tracing::warn!(error = %e, "failed to parse embedded errors.toml; explainer disabled");
                ExplainerFile { entries: Vec::new() }
            }
        };
        parsed
            .entries
            .into_iter()
            .filter_map(|entry| match Regex::new(&entry.pattern) {
                Ok(regex) => Some(Compiled { regex, entry }),
                Err(e) => {
                    tracing::warn!(pattern = %entry.pattern, error = %e, "invalid regex in errors.toml");
                    None
                }
            })
            .collect()
    })
}

pub async fn handle_explain(req: ExplainRequest) -> CoreResult<ExplainResponse> {
    let matched = entries().iter().find_map(|c| {
        if c.regex.is_match(&req.error) {
            Some(ExplainHit {
                title: c.entry.title.clone(),
                explanation: c.entry.explanation.clone(),
                fix: c.entry.fix.clone(),
            })
        } else {
            None
        }
    });
    Ok(ExplainResponse { matched })
}
