//! Aiken symbol index.
//!
//! Walks user-supplied root directories, parses every `.ak` file with a small
//! line-level regex pass, returns matching `pub fn` / `pub type` /
//! `pub const` / `validator` declarations.
//!
//! v0 is regex-based. Aiken's surface syntax for these declarations is
//! stable enough that line-level extraction works well in practice. When
//! Aiken adds a new top-level form, add a new regex.
//!
//! v1 may swap to a precomputed cache or to a real parser if expressivity
//! is needed.

use std::path::PathBuf;

use async_trait::async_trait;
use regex::Regex;
use tokio::fs;
use tracing::warn;
use walkdir::WalkDir;

use aiken_mcp_core::{CoreResult, Symbol, SymbolIndex, SymbolKind};

#[derive(Debug, Clone)]
pub struct FileWalkSymbolIndex {
    roots: Vec<PathBuf>,
}

impl FileWalkSymbolIndex {
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
impl SymbolIndex for FileWalkSymbolIndex {
    async fn lookup(&self, query: &str, max_hits: usize) -> CoreResult<Vec<Symbol>> {
        if self.roots.is_empty() {
            return Ok(Vec::new());
        }

        let regexes = ParseRegexes::new();
        let q = query.to_ascii_lowercase();

        let mut hits = Vec::new();
        for root in &self.roots {
            for entry in WalkDir::new(root).into_iter().filter_map(|e| e.ok()) {
                if hits.len() >= max_hits {
                    break;
                }
                let path = entry.path();
                if path.extension().and_then(|s| s.to_str()) != Some("ak") {
                    continue;
                }
                let text = match fs::read_to_string(path).await {
                    Ok(t) => t,
                    Err(e) => {
                        warn!(file = %path.display(), error = %e, "skipping unreadable .ak");
                        continue;
                    }
                };

                let module = derive_module(root, path);
                for (lineno, line) in text.lines().enumerate() {
                    if let Some(sym) = regexes.match_line(line, lineno as u32 + 1, &module, path) {
                        if sym.name.to_ascii_lowercase().contains(&q) {
                            hits.push(sym);
                            if hits.len() >= max_hits {
                                break;
                            }
                        }
                    }
                }
            }
        }
        Ok(hits)
    }
}

fn derive_module(root: &std::path::Path, file: &std::path::Path) -> String {
    let rel = file.strip_prefix(root).unwrap_or(file);
    let rel = rel.with_extension("");
    rel.components()
        .map(|c| c.as_os_str().to_string_lossy().to_string())
        .collect::<Vec<_>>()
        .join("/")
}

struct ParseRegexes {
    pub_fn: Regex,
    pub_type: Regex,
    pub_const: Regex,
    validator: Regex,
}

impl ParseRegexes {
    fn new() -> Self {
        Self {
            pub_fn: Regex::new(r"^\s*pub\s+fn\s+(\w+)\s*(.*)$").unwrap(),
            pub_type: Regex::new(r"^\s*pub\s+type\s+(\w+)\b(.*)$").unwrap(),
            pub_const: Regex::new(r"^\s*pub\s+const\s+(\w+)\b(.*)$").unwrap(),
            validator: Regex::new(r"^\s*validator\s+(\w+)\b(.*)$").unwrap(),
        }
    }

    fn match_line(
        &self,
        line: &str,
        lineno: u32,
        module: &str,
        path: &std::path::Path,
    ) -> Option<Symbol> {
        if let Some(c) = self.pub_fn.captures(line) {
            return Some(self.build(line, &c, SymbolKind::Fn, lineno, module, path));
        }
        if let Some(c) = self.pub_type.captures(line) {
            return Some(self.build(line, &c, SymbolKind::Type, lineno, module, path));
        }
        if let Some(c) = self.pub_const.captures(line) {
            return Some(self.build(line, &c, SymbolKind::Const, lineno, module, path));
        }
        if let Some(c) = self.validator.captures(line) {
            return Some(self.build(line, &c, SymbolKind::Validator, lineno, module, path));
        }
        None
    }

    fn build(
        &self,
        line: &str,
        captures: &regex::Captures,
        kind: SymbolKind,
        lineno: u32,
        module: &str,
        path: &std::path::Path,
    ) -> Symbol {
        let name = captures.get(1).unwrap().as_str().to_string();
        Symbol {
            name,
            kind,
            module: module.to_string(),
            signature: line.trim().to_string(),
            file: path.display().to_string(),
            line: lineno,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[tokio::test]
    async fn lookup_finds_pub_fn_and_pub_type() {
        let tmp = tempdir().unwrap();
        let f = tmp.path().join("foo.ak");
        fs::write(
            &f,
            "pub fn add(a: Int, b: Int) -> Int { a + b }\npub type Foo { x: Int }\nvalidator bar { ... }\n",
        )
        .unwrap();
        let idx = FileWalkSymbolIndex::new(vec![tmp.path().to_path_buf()]);
        let hits = idx.lookup("add", 10).await.unwrap();
        assert!(hits.iter().any(|h| h.name == "add" && matches!(h.kind, SymbolKind::Fn)));
        let hits = idx.lookup("Foo", 10).await.unwrap();
        assert!(hits.iter().any(|h| h.name == "Foo" && matches!(h.kind, SymbolKind::Type)));
        let hits = idx.lookup("bar", 10).await.unwrap();
        assert!(hits.iter().any(|h| h.name == "bar" && matches!(h.kind, SymbolKind::Validator)));
    }

    #[tokio::test]
    async fn empty_when_no_roots() {
        let idx = FileWalkSymbolIndex::new(Vec::new());
        let hits = idx.lookup("x", 10).await.unwrap();
        assert!(hits.is_empty());
    }
}
