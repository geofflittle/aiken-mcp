//! Corpus search across user-supplied reference Aiken codebases.
//!
//! Backed by `rg` (ripgrep) over a list of root directories. The set of
//! corpus roots is supplied at construction time via the `AIKEN_MCP_CORPUS`
//! env (colon-separated paths) or by the parent process explicitly.
//!
//! v0 is intentionally simple: ripgrep over `.ak` files, return the first N
//! matches. v1 may swap in a precomputed index (tantivy or symbol graph).

use std::path::PathBuf;
use std::process::Stdio;

use async_trait::async_trait;
use tokio::process::Command;
use tracing::debug;

use aiken_mcp_core::{CoreError, CoreResult, CorpusHit, CorpusSearch};

#[derive(Debug, Clone)]
pub struct RipgrepCorpus {
    roots: Vec<PathBuf>,
}

impl RipgrepCorpus {
    pub fn new(roots: Vec<PathBuf>) -> Self {
        Self { roots }
    }

    pub fn from_env(var: &str) -> Self {
        let roots = std::env::var(var)
            .ok()
            .map(|raw| {
                raw.split(':')
                    .filter(|s| !s.is_empty())
                    .map(PathBuf::from)
                    .collect()
            })
            .unwrap_or_default();
        Self { roots }
    }

    pub fn roots(&self) -> &[PathBuf] {
        &self.roots
    }
}

#[async_trait]
impl CorpusSearch for RipgrepCorpus {
    async fn search(&self, query: &str, max_hits: usize) -> CoreResult<Vec<CorpusHit>> {
        if self.roots.is_empty() {
            return Ok(Vec::new());
        }

        let mut hits = Vec::new();
        for root in &self.roots {
            let mut cmd = Command::new("rg");
            cmd.arg("--no-heading")
                .arg("--with-filename")
                .arg("--line-number")
                .arg("--glob")
                .arg("*.ak")
                .arg("--max-count")
                .arg(max_hits.to_string())
                .arg("--")
                .arg(query)
                .arg(root);
            cmd.stdout(Stdio::piped()).stderr(Stdio::piped());

            debug!(?cmd, "running rg");

            let output = match cmd.output().await {
                Ok(o) => o,
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                    return Err(CoreError::other(
                        "ripgrep (rg) not on PATH; install via `brew install ripgrep`",
                    ));
                }
                Err(e) => return Err(CoreError::Io(e)),
            };

            let stdout = String::from_utf8_lossy(&output.stdout);
            for line in stdout.lines() {
                if hits.len() >= max_hits {
                    break;
                }
                if let Some(hit) = parse_rg_line(line, root) {
                    hits.push(hit);
                }
            }
        }

        hits.truncate(max_hits);
        Ok(hits)
    }
}

fn parse_rg_line(line: &str, root: &PathBuf) -> Option<CorpusHit> {
    // Format: <path>:<line>:<text>
    let mut parts = line.splitn(3, ':');
    let file = parts.next()?;
    let lineno: u32 = parts.next()?.parse().ok()?;
    let snippet = parts.next()?;
    Some(CorpusHit {
        corpus: root.display().to_string(),
        file: file.to_string(),
        line: lineno,
        snippet: snippet.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_env_parses_colon_paths() {
        std::env::set_var("AIKEN_MCP_CORPUS_TEST", "/a:/b:/c");
        let c = RipgrepCorpus::from_env("AIKEN_MCP_CORPUS_TEST");
        assert_eq!(c.roots().len(), 3);
        std::env::remove_var("AIKEN_MCP_CORPUS_TEST");
    }

    #[test]
    fn from_env_empty_when_unset() {
        let c = RipgrepCorpus::from_env("DEFINITELY_NOT_SET_KJSDHFLKJ");
        assert!(c.roots().is_empty());
    }

    #[test]
    fn parses_rg_line() {
        let line = "/x/foo.ak:42:fn bar() -> Bool { True }";
        let hit = parse_rg_line(line, &PathBuf::from("/x")).unwrap();
        assert_eq!(hit.line, 42);
        assert_eq!(hit.file, "/x/foo.ak");
        assert!(hit.snippet.contains("fn bar"));
    }
}
