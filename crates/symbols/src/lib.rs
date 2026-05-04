//! Aiken symbol index.
//!
//! Walks user-supplied root directories, parses every `.ak` file with a small
//! line-level pass, returns matching `pub fn` / `pub type` / `pub const` /
//! `validator` declarations along with any preceding `///` doc comments.
//!
//! Lookup matches against the symbol's name OR its doc text
//! (case-insensitive), so consumers can search by topic or by symbol name
//! interchangeably without separate tools.
//!
//! v0 is regex-based. Aiken's surface syntax for these declarations is
//! stable enough that line-level extraction works well in practice. When
//! Aiken adds a new top-level form, add a new regex.
//!
//! Doc capture rules:
//! - Lines starting with `///` (item docs) are accumulated into a pending
//!   buffer. The triple-slash marker + at most one space is stripped.
//! - A blank line resets the buffer.
//! - A non-doc, non-blank line that doesn't match a `pub` / `validator`
//!   declaration also resets the buffer (avoids attaching unrelated
//!   comments to the next item).
//! - When a declaration is matched, the current buffer is attached as its
//!   doc and the buffer is cleared.
//! - Module-level docs (`////`) are not attached; they're file-scope. v1 may
//!   surface them as a separate field.

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
                let symbols = extract_symbols(&text, &regexes, &module, path);
                for sym in symbols {
                    if matches_query(&sym, &q) {
                        hits.push(sym);
                        if hits.len() >= max_hits {
                            break;
                        }
                    }
                }
            }
        }
        Ok(hits)
    }
}

fn matches_query(sym: &Symbol, q_lower: &str) -> bool {
    if sym.name.to_ascii_lowercase().contains(q_lower) {
        return true;
    }
    if let Some(doc) = sym.doc.as_deref() {
        if doc.to_ascii_lowercase().contains(q_lower) {
            return true;
        }
    }
    false
}

/// Walk a single file's lines, returning every public symbol with its doc
/// comment block (if any). Pure function — easy to unit-test.
fn extract_symbols(
    text: &str,
    regexes: &ParseRegexes,
    module: &str,
    path: &std::path::Path,
) -> Vec<Symbol> {
    let mut out = Vec::new();
    let mut pending_doc: Vec<String> = Vec::new();

    for (idx, raw) in text.lines().enumerate() {
        let line = raw;
        let trimmed = line.trim_start();
        let lineno = idx as u32 + 1;

        // Doc comment line: append to buffer.
        // Note: `////` (module doc) is intentionally NOT captured here.
        if let Some(rest) = trimmed.strip_prefix("///") {
            if rest.starts_with('/') {
                // `////` module doc — reset, do not attach to next item.
                pending_doc.clear();
                continue;
            }
            let body = rest.strip_prefix(' ').unwrap_or(rest);
            pending_doc.push(body.to_string());
            continue;
        }

        // Blank line: reset doc buffer.
        if trimmed.is_empty() {
            pending_doc.clear();
            continue;
        }

        // Try to match a declaration.
        if let Some(sym) = regexes.match_line(line, lineno, module, path, &pending_doc) {
            out.push(sym);
            pending_doc.clear();
            continue;
        }

        // Non-doc, non-decl line: pending docs become unattachable.
        pending_doc.clear();
    }

    out
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
            pub_fn: Regex::new(r"^\s*pub\s+fn\s+(\w+)\b").unwrap(),
            pub_type: Regex::new(r"^\s*pub\s+type\s+(\w+)\b").unwrap(),
            pub_const: Regex::new(r"^\s*pub\s+const\s+(\w+)\b").unwrap(),
            validator: Regex::new(r"^\s*validator\s+(\w+)\b").unwrap(),
        }
    }

    fn match_line(
        &self,
        line: &str,
        lineno: u32,
        module: &str,
        path: &std::path::Path,
        pending_doc: &[String],
    ) -> Option<Symbol> {
        let (name, kind) = if let Some(c) = self.pub_fn.captures(line) {
            (c.get(1).unwrap().as_str().to_string(), SymbolKind::Fn)
        } else if let Some(c) = self.pub_type.captures(line) {
            (c.get(1).unwrap().as_str().to_string(), SymbolKind::Type)
        } else if let Some(c) = self.pub_const.captures(line) {
            (c.get(1).unwrap().as_str().to_string(), SymbolKind::Const)
        } else if let Some(c) = self.validator.captures(line) {
            (c.get(1).unwrap().as_str().to_string(), SymbolKind::Validator)
        } else {
            return None;
        };

        let doc = if pending_doc.is_empty() {
            None
        } else {
            Some(pending_doc.join("\n"))
        };

        Some(Symbol {
            name,
            kind,
            module: module.to_string(),
            signature: line.trim().to_string(),
            file: path.display().to_string(),
            line: lineno,
            doc,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::Path;
    use tempfile::tempdir;

    fn extract_from(source: &str) -> Vec<Symbol> {
        let regexes = ParseRegexes::new();
        extract_symbols(source, &regexes, "test", Path::new("test.ak"))
    }

    #[tokio::test]
    async fn lookup_matches_name() {
        let tmp = tempdir().unwrap();
        let f = tmp.path().join("foo.ak");
        fs::write(&f, "pub fn add(a: Int, b: Int) -> Int { a + b }\n").unwrap();
        let idx = FileWalkSymbolIndex::new(vec![tmp.path().to_path_buf()]);
        let hits = idx.lookup("add", 10).await.unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].name, "add");
    }

    #[tokio::test]
    async fn lookup_matches_doc_text() {
        let tmp = tempdir().unwrap();
        let f = tmp.path().join("foo.ak");
        fs::write(
            &f,
            "/// Verify a Merkle proof against a known root.\npub fn verify(proof: List<ByteArray>) -> Bool { True }\n",
        )
        .unwrap();
        let idx = FileWalkSymbolIndex::new(vec![tmp.path().to_path_buf()]);
        let hits = idx.lookup("merkle", 10).await.unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].name, "verify");
        assert!(hits[0].doc.as_deref().unwrap().contains("Merkle"));
    }

    #[test]
    fn captures_single_line_doc() {
        let symbols = extract_from(
            "/// Add two ints.\npub fn add(a: Int, b: Int) -> Int { a + b }\n",
        );
        assert_eq!(symbols.len(), 1);
        assert_eq!(symbols[0].doc.as_deref(), Some("Add two ints."));
    }

    #[test]
    fn captures_multi_line_doc() {
        let symbols = extract_from(
            "/// First line.\n/// Second line.\npub fn add() {}\n",
        );
        assert_eq!(symbols[0].doc.as_deref(), Some("First line.\nSecond line."));
    }

    #[test]
    fn blank_line_resets_doc() {
        let symbols = extract_from(
            "/// Stale doc.\n\npub fn add() {}\n",
        );
        assert!(symbols[0].doc.is_none());
    }

    #[test]
    fn non_decl_line_resets_doc() {
        let symbols = extract_from(
            "/// Stale doc.\nlet x = 1\npub fn add() {}\n",
        );
        assert!(symbols[0].doc.is_none());
    }

    #[test]
    fn ignores_module_docs() {
        let symbols = extract_from(
            "//// Module-level doc.\npub fn add() {}\n",
        );
        // Module doc shouldn't attach to add.
        assert!(symbols[0].doc.is_none());
    }

    #[test]
    fn captures_validator_with_doc() {
        let symbols = extract_from(
            "/// Bridge validator.\nvalidator bridge { ... }\n",
        );
        assert_eq!(symbols[0].kind, SymbolKind::Validator);
        assert_eq!(symbols[0].doc.as_deref(), Some("Bridge validator."));
    }

    #[test]
    fn handles_consecutive_decls() {
        let symbols = extract_from(
            "/// First.\npub fn one() {}\n/// Second.\npub fn two() {}\n",
        );
        assert_eq!(symbols.len(), 2);
        assert_eq!(symbols[0].doc.as_deref(), Some("First."));
        assert_eq!(symbols[1].doc.as_deref(), Some("Second."));
    }

    #[test]
    fn pub_const_and_pub_type_captured() {
        let symbols = extract_from(
            "/// Tau.\npub const tau: Int = 6\n/// Foo type.\npub type Foo { x: Int }\n",
        );
        assert_eq!(symbols.len(), 2);
        assert!(matches!(symbols[0].kind, SymbolKind::Const));
        assert!(matches!(symbols[1].kind, SymbolKind::Type));
    }

    #[tokio::test]
    async fn empty_when_no_roots() {
        let idx = FileWalkSymbolIndex::new(Vec::new());
        let hits = idx.lookup("x", 10).await.unwrap();
        assert!(hits.is_empty());
    }
}
