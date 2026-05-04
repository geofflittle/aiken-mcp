//! Entry point: wires concrete impls into the `Server` and serves over stdio.

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Result;
use rmcp::{transport::stdio, ServiceExt};
use tracing_subscriber::EnvFilter;

mod server;

use server::{AikenMcpServer, ServerDeps};

#[tokio::main]
async fn main() -> Result<()> {
    init_tracing();

    let deps = build_deps();
    let service = AikenMcpServer::new(deps).serve(stdio()).await?;
    service.waiting().await?;
    Ok(())
}

fn init_tracing() {
    // MCP servers must NOT write to stdout (it's the transport channel).
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_env("AIKEN_MCP_LOG").unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .with_writer(std::io::stderr)
        .with_ansi(false)
        .init();
    tracing::info!(version = env!("CARGO_PKG_VERSION"), "starting aiken-mcp");
}

fn build_deps() -> ServerDeps {
    let runner = Arc::new(aiken_mcp_cli::AikenCliRunner::new());

    let corpus_roots = parse_path_list("AIKEN_MCP_CORPUS");
    let corpus = Arc::new(aiken_mcp_corpus::RipgrepCorpus::new(corpus_roots.clone()));

    let docs_cache = std::env::var("AIKEN_MCP_DOCS_CACHE")
        .map(PathBuf::from)
        .unwrap_or_else(|_| dirs_cache_dir().join("aiken-mcp/docs"));
    let docs_base = std::env::var("AIKEN_MCP_DOCS_BASE_URL")
        .unwrap_or_else(|_| "https://aiken-lang.org".to_string());
    let docs = Arc::new(aiken_mcp_docs::HttpDocsFetcher::new(docs_base, docs_cache));

    let lsp = Arc::new(aiken_mcp_lsp::AikenLspClient::new());

    let symbol_roots = if corpus_roots.is_empty() {
        parse_path_list("AIKEN_MCP_SYMBOLS")
    } else {
        corpus_roots
    };
    let symbols = Arc::new(aiken_mcp_symbols::FileWalkSymbolIndex::new(symbol_roots));

    let blueprint = Arc::new(aiken_mcp_blueprint::JsonBlueprintReader::new());

    ServerDeps {
        runner,
        corpus,
        docs,
        lsp,
        symbols,
        blueprint,
    }
}

fn parse_path_list(var: &str) -> Vec<PathBuf> {
    std::env::var(var)
        .ok()
        .map(|raw| {
            raw.split(':')
                .filter(|s| !s.is_empty())
                .map(PathBuf::from)
                .collect()
        })
        .unwrap_or_default()
}

fn dirs_cache_dir() -> PathBuf {
    if let Ok(xdg) = std::env::var("XDG_CACHE_HOME") {
        return PathBuf::from(xdg);
    }
    if let Ok(home) = std::env::var("HOME") {
        #[cfg(target_os = "macos")]
        return PathBuf::from(home).join("Library/Caches");
        #[cfg(not(target_os = "macos"))]
        return PathBuf::from(home).join(".cache");
    }
    PathBuf::from("/tmp")
}
