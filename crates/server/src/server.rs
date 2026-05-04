//! MCP server adapter. Wraps `tools` handlers behind rmcp tool registrations.
//!
//! All MCP-specific concerns (tool routing, JSON schema, response wrapping)
//! live here. `tools` and `core` crates remain transport-agnostic.

use std::sync::Arc;

use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{
    CallToolResult, Content, Implementation, ProtocolVersion, ServerCapabilities, ServerInfo,
};
use rmcp::schemars::{self, JsonSchema};
use rmcp::{tool, tool_handler, tool_router, ErrorData as McpError, ServerHandler};
use serde::Deserialize;

use aiken_mcp_core::{AikenRunner, CorpusSearch, DocsFetcher};
use aiken_mcp_tools as tools;

#[derive(Clone)]
pub struct ServerDeps {
    pub runner: Arc<dyn AikenRunner>,
    pub corpus: Arc<dyn CorpusSearch>,
    pub docs: Arc<dyn DocsFetcher>,
}

#[derive(Clone)]
pub struct AikenMcpServer {
    deps: ServerDeps,
    #[allow(dead_code)] // populated via Self::tool_router(), read by tool_handler macro
    tool_router: ToolRouter<AikenMcpServer>,
}

// MCP-facing request types. They mirror the `tools` crate request structs
// but carry `JsonSchema` for tool-schema generation. Conversions to the
// transport-agnostic types live in `From` impls below.

#[derive(Debug, Deserialize, JsonSchema)]
pub struct CheckArgs {
    /// File path inside an Aiken project, or the project directory itself.
    /// The server walks upward from this path looking for an aiken.toml.
    pub path: String,
    /// Optional Aiken module filter passed via `aiken check -m`.
    #[serde(default)]
    pub module: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct BuildArgs {
    pub path: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct TestArgs {
    pub path: String,
    #[serde(default)]
    pub filter: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct FmtArgs {
    pub source: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SearchArgs {
    pub query: String,
    #[serde(default = "default_max_hits")]
    pub max_hits: usize,
}

fn default_max_hits() -> usize {
    20
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct DocsArgs {
    /// Path under the docs base URL, e.g. `language-tour/types`.
    pub path: String,
}

impl From<CheckArgs> for tools::CheckRequest {
    fn from(a: CheckArgs) -> Self {
        Self { path: a.path, module: a.module }
    }
}
impl From<BuildArgs> for tools::BuildRequest {
    fn from(a: BuildArgs) -> Self {
        Self { path: a.path }
    }
}
impl From<TestArgs> for tools::TestRequest {
    fn from(a: TestArgs) -> Self {
        Self { path: a.path, filter: a.filter }
    }
}
impl From<FmtArgs> for tools::FmtRequest {
    fn from(a: FmtArgs) -> Self {
        Self { source: a.source }
    }
}
impl From<SearchArgs> for tools::SearchRequest {
    fn from(a: SearchArgs) -> Self {
        Self { query: a.query, max_hits: a.max_hits }
    }
}
impl From<DocsArgs> for tools::DocsLookupRequest {
    fn from(a: DocsArgs) -> Self {
        Self { path: a.path }
    }
}

#[tool_router]
impl AikenMcpServer {
    pub fn new(deps: ServerDeps) -> Self {
        Self {
            deps,
            tool_router: Self::tool_router(),
        }
    }

    #[tool(description = "Run `aiken check` on the project containing the given path. Returns diagnostics + raw stdout/stderr.")]
    async fn aiken_check(
        &self,
        Parameters(args): Parameters<CheckArgs>,
    ) -> Result<CallToolResult, McpError> {
        json_call(tools::handle_check(self.deps.runner.clone(), args.into()).await)
    }

    #[tool(description = "Run `aiken build` on the project. Returns diagnostics + listed artifacts.")]
    async fn aiken_build(
        &self,
        Parameters(args): Parameters<BuildArgs>,
    ) -> Result<CallToolResult, McpError> {
        json_call(tools::handle_build(self.deps.runner.clone(), args.into()).await)
    }

    #[tool(description = "Run aiken tests via `aiken check`. Returns per-test pass/fail results.")]
    async fn aiken_test(
        &self,
        Parameters(args): Parameters<TestArgs>,
    ) -> Result<CallToolResult, McpError> {
        json_call(tools::handle_test(self.deps.runner.clone(), args.into()).await)
    }

    #[tool(description = "Format Aiken source via `aiken fmt --stdin`. Returns formatted source on success.")]
    async fn aiken_fmt(
        &self,
        Parameters(args): Parameters<FmtArgs>,
    ) -> Result<CallToolResult, McpError> {
        json_call(tools::handle_fmt(self.deps.runner.clone(), args.into()).await)
    }

    #[tool(description = "Search the configured Aiken reference corpus (set via AIKEN_MCP_CORPUS) for a query string. Returns line-level hits.")]
    async fn aiken_pattern_search(
        &self,
        Parameters(args): Parameters<SearchArgs>,
    ) -> Result<CallToolResult, McpError> {
        json_call(tools::handle_search(self.deps.corpus.clone(), args.into()).await)
    }

    #[tool(description = "Fetch a page from the Aiken docs site (default https://aiken-lang.org), with on-disk cache.")]
    async fn aiken_docs(
        &self,
        Parameters(args): Parameters<DocsArgs>,
    ) -> Result<CallToolResult, McpError> {
        json_call(tools::handle_docs_lookup(self.deps.docs.clone(), args.into()).await)
    }

    #[tool(description = "Get the installed Aiken CLI version.")]
    async fn aiken_version(&self) -> Result<CallToolResult, McpError> {
        json_call(tools::handle_version(self.deps.runner.clone()).await)
    }
}

#[tool_handler]
impl ServerHandler for AikenMcpServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(
            ServerCapabilities::builder().enable_tools().build(),
        )
        .with_server_info(Implementation::from_build_env())
        .with_protocol_version(ProtocolVersion::V_2024_11_05)
        .with_instructions(
            "Aiken tooling: aiken_check / aiken_build / aiken_test / aiken_fmt run the local Aiken CLI on a project. \
             aiken_pattern_search greps user-supplied reference Aiken codebases. \
             aiken_docs fetches pages from aiken-lang.org with caching. \
             aiken_version reports the local CLI version.".to_string(),
        )
    }
}

fn json_call<T: serde::Serialize>(
    result: aiken_mcp_core::CoreResult<T>,
) -> Result<CallToolResult, McpError> {
    match result {
        Ok(payload) => {
            let body = serde_json::to_string_pretty(&payload)
                .map_err(|e| McpError::internal_error(format!("serialize: {e}"), None))?;
            Ok(CallToolResult::success(vec![Content::text(body)]))
        }
        Err(e) => Ok(CallToolResult::success(vec![Content::text(format!(
            "{{\"error\": {} }}",
            serde_json::to_string(&e.to_string()).unwrap_or_else(|_| "\"unknown\"".to_string())
        ))])),
    }
}
