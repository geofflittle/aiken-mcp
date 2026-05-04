//! Entry point: wires concrete impls into the `Server` and serves over stdio.

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Result;
use rmcp::{transport::stdio, ServiceExt};
use tracing_subscriber::EnvFilter;

mod server;

use server::{ServerDeps, AikenMcpServer};

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
    // Send all logs to stderr.
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_env("AIKEN_MCP_LOG").unwrap_or_else(|_| EnvFilter::new("info")))
        .with_writer(std::io::stderr)
        .with_ansi(false)
        .init();
    tracing::info!(version = env!("CARGO_PKG_VERSION"), "starting aiken-mcp");
}

fn build_deps() -> ServerDeps {
    let runner = Arc::new(aiken_mcp_cli::AikenCliRunner::new());

    let corpus_roots = std::env::var("AIKEN_MCP_CORPUS")
        .ok()
        .map(|raw| {
            raw.split(':')
                .filter(|s| !s.is_empty())
                .map(PathBuf::from)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let corpus = Arc::new(aiken_mcp_corpus::RipgrepCorpus::new(corpus_roots));

    let docs_cache = std::env::var("AIKEN_MCP_DOCS_CACHE")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            dirs_cache_dir().join("aiken-mcp/docs")
        });
    let docs_base = std::env::var("AIKEN_MCP_DOCS_BASE_URL")
        .unwrap_or_else(|_| "https://aiken-lang.org".to_string());
    let docs = Arc::new(aiken_mcp_docs::HttpDocsFetcher::new(docs_base, docs_cache));

    ServerDeps { runner, corpus, docs }
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
