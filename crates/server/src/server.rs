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

use aiken_mcp_core::{
    AikenRunner, BlueprintReader, CorpusSearch, DocsFetcher, LspClient, SymbolIndex,
};
use aiken_mcp_tools as tools;

#[derive(Clone)]
pub struct ServerDeps {
    pub runner: Arc<dyn AikenRunner>,
    pub corpus: Arc<dyn CorpusSearch>,
    pub docs: Arc<dyn DocsFetcher>,
    pub lsp: Arc<dyn LspClient>,
    pub symbols: Arc<dyn SymbolIndex>,
    pub blueprint: Arc<dyn BlueprintReader>,
}

#[derive(Clone)]
pub struct AikenMcpServer {
    deps: ServerDeps,
    #[allow(dead_code)] // populated via Self::tool_router(), read by tool_handler macro
    tool_router: ToolRouter<AikenMcpServer>,
}

// MCP-facing argument types. Mirror the `tools` crate request structs but
// carry `JsonSchema` for tool-schema generation. Conversions live below.

#[derive(Debug, Deserialize, JsonSchema)]
pub struct CheckArgs {
    /// File path inside an Aiken project, or the project directory itself.
    pub path: String,
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
    pub path: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct PositionArgs {
    /// Absolute path to the .ak file.
    pub file: String,
    /// Zero-based line number.
    pub line: u32,
    /// Zero-based UTF-16 column.
    pub column: u32,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct BudgetArgs {
    pub path: String,
    #[serde(default)]
    pub filter: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SymbolLookupArgs {
    pub query: String,
    #[serde(default = "default_max_hits")]
    pub max_hits: usize,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct BlueprintArgs {
    pub path: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct UplcArgs {
    pub path: String,
    /// Path to a CBOR/UPLC artifact to decode.
    pub target: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct NewProjectArgs {
    pub parent_dir: String,
    pub name: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ExplainArgs {
    pub error: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct CorpusListArgs {
    /// Optional case-insensitive tag filter.
    #[serde(default)]
    pub tag: Option<String>,
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
impl From<PositionArgs> for tools::HoverRequest {
    fn from(a: PositionArgs) -> Self {
        Self { file: a.file, line: a.line, column: a.column }
    }
}
impl From<PositionArgs> for tools::CompletionsRequest {
    fn from(a: PositionArgs) -> Self {
        Self { file: a.file, line: a.line, column: a.column }
    }
}
impl From<PositionArgs> for tools::DefinitionRequest {
    fn from(a: PositionArgs) -> Self {
        Self { file: a.file, line: a.line, column: a.column }
    }
}
impl From<BudgetArgs> for tools::BudgetRequest {
    fn from(a: BudgetArgs) -> Self {
        Self { path: a.path, filter: a.filter }
    }
}
impl From<SymbolLookupArgs> for tools::SymbolLookupRequest {
    fn from(a: SymbolLookupArgs) -> Self {
        Self { query: a.query, max_hits: a.max_hits }
    }
}
impl From<BlueprintArgs> for tools::BlueprintRequest {
    fn from(a: BlueprintArgs) -> Self {
        Self { path: a.path }
    }
}
impl From<UplcArgs> for tools::UplcRequest {
    fn from(a: UplcArgs) -> Self {
        Self { path: a.path, target: a.target }
    }
}
impl From<NewProjectArgs> for tools::NewProjectRequest {
    fn from(a: NewProjectArgs) -> Self {
        Self { parent_dir: a.parent_dir, name: a.name }
    }
}
impl From<ExplainArgs> for tools::ExplainRequest {
    fn from(a: ExplainArgs) -> Self {
        Self { error: a.error }
    }
}
impl From<CorpusListArgs> for tools::CorpusListRequest {
    fn from(a: CorpusListArgs) -> Self {
        Self { tag: a.tag }
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
    async fn aiken_check(&self, Parameters(args): Parameters<CheckArgs>) -> Result<CallToolResult, McpError> {
        json_call(tools::handle_check(self.deps.runner.clone(), args.into()).await)
    }

    #[tool(description = "Run `aiken build` on the project. Returns diagnostics + listed artifacts.")]
    async fn aiken_build(&self, Parameters(args): Parameters<BuildArgs>) -> Result<CallToolResult, McpError> {
        json_call(tools::handle_build(self.deps.runner.clone(), args.into()).await)
    }

    #[tool(description = "Run aiken tests via `aiken check`. Returns per-test pass/fail results with mem/cpu when available.")]
    async fn aiken_test(&self, Parameters(args): Parameters<TestArgs>) -> Result<CallToolResult, McpError> {
        json_call(tools::handle_test(self.deps.runner.clone(), args.into()).await)
    }

    #[tool(description = "Format Aiken source via `aiken fmt --stdin`. Returns formatted source on success.")]
    async fn aiken_fmt(&self, Parameters(args): Parameters<FmtArgs>) -> Result<CallToolResult, McpError> {
        json_call(tools::handle_fmt(self.deps.runner.clone(), args.into()).await)
    }

    #[tool(description = "Ripgrep over user-supplied reference Aiken codebases (set via AIKEN_MCP_CORPUS).")]
    async fn aiken_pattern_search(&self, Parameters(args): Parameters<SearchArgs>) -> Result<CallToolResult, McpError> {
        json_call(tools::handle_search(self.deps.corpus.clone(), args.into()).await)
    }

    #[tool(description = "Fetch a page from the Aiken docs site (default https://aiken-lang.org), with on-disk cache.")]
    async fn aiken_docs(&self, Parameters(args): Parameters<DocsArgs>) -> Result<CallToolResult, McpError> {
        json_call(tools::handle_docs_lookup(self.deps.docs.clone(), args.into()).await)
    }

    #[tool(description = "Get the installed Aiken CLI version.")]
    async fn aiken_version(&self) -> Result<CallToolResult, McpError> {
        json_call(tools::handle_version(self.deps.runner.clone()).await)
    }

    #[tool(description = "LSP hover at file/line/column. Returns markdown when available. (line/column are zero-based.)")]
    async fn aiken_hover(&self, Parameters(args): Parameters<PositionArgs>) -> Result<CallToolResult, McpError> {
        json_call(tools::handle_hover(self.deps.lsp.clone(), args.into()).await)
    }

    #[tool(description = "LSP completions at file/line/column.")]
    async fn aiken_completions(&self, Parameters(args): Parameters<PositionArgs>) -> Result<CallToolResult, McpError> {
        json_call(tools::handle_completions(self.deps.lsp.clone(), args.into()).await)
    }

    #[tool(description = "LSP go-to-definition at file/line/column.")]
    async fn aiken_definition(&self, Parameters(args): Parameters<PositionArgs>) -> Result<CallToolResult, McpError> {
        json_call(tools::handle_definition(self.deps.lsp.clone(), args.into()).await)
    }

    #[tool(description = "Run aiken tests and report Plutus exec budget per test (mem, cpu, % of tx limit).")]
    async fn aiken_budget(&self, Parameters(args): Parameters<BudgetArgs>) -> Result<CallToolResult, McpError> {
        json_call(tools::handle_budget(self.deps.runner.clone(), args.into()).await)
    }

    #[tool(description = "Lookup pub fn / pub type / pub const / validator declarations across the configured Aiken corpus. Query matches symbol name OR doc-comment text, so search by topic (e.g. 'merkle proof') or by symbol name interchangeably. Each result includes the preceding `///` doc comment when present.")]
    async fn aiken_symbol_lookup(&self, Parameters(args): Parameters<SymbolLookupArgs>) -> Result<CallToolResult, McpError> {
        json_call(tools::handle_symbol_lookup(self.deps.symbols.clone(), args.into()).await)
    }

    #[tool(description = "Parse `plutus.json` (CIP-57 blueprint) from the project. Returns validators, hashes, parameter schemas, compiled-script size.")]
    async fn aiken_blueprint(&self, Parameters(args): Parameters<BlueprintArgs>) -> Result<CallToolResult, McpError> {
        json_call(tools::handle_blueprint(self.deps.blueprint.clone(), args.into()).await)
    }

    #[tool(description = "Decode a UPLC artifact via `aiken uplc decode`.")]
    async fn aiken_uplc(&self, Parameters(args): Parameters<UplcArgs>) -> Result<CallToolResult, McpError> {
        json_call(tools::handle_uplc(self.deps.runner.clone(), args.into()).await)
    }

    #[tool(description = "Scaffold a new Aiken project via `aiken new`.")]
    async fn aiken_new(&self, Parameters(args): Parameters<NewProjectArgs>) -> Result<CallToolResult, McpError> {
        json_call(tools::handle_new_project(self.deps.runner.clone(), args.into()).await)
    }

    #[tool(description = "Look up a canonical explanation + fix for a common Aiken error string.")]
    async fn aiken_explain(&self, Parameters(args): Parameters<ExplainArgs>) -> Result<CallToolResult, McpError> {
        json_call(tools::handle_explain(args.into()).await)
    }

    #[tool(description = "List curated high-expertise Aiken codebases. Optional tag filter (e.g. `dex`, `bridge`, `merkle`, `patterns`). Returns repo url + author + tags + study notes.")]
    async fn aiken_corpus_list(&self, Parameters(args): Parameters<CorpusListArgs>) -> Result<CallToolResult, McpError> {
        json_call(tools::handle_corpus_list(args.into()).await)
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
            "Aiken tooling: aiken_check / aiken_build / aiken_test / aiken_fmt / aiken_budget / aiken_uplc / aiken_new wrap the Aiken CLI. \
             aiken_hover / aiken_completions / aiken_definition use `aiken lsp --stdio` for type-aware queries. \
             aiken_pattern_search greps user-supplied reference Aiken codebases (AIKEN_MCP_CORPUS). \
             aiken_symbol_lookup indexes pub fn/type/const/validator declarations + their doc comments across the corpus. \
             aiken_corpus_list returns the curated repo manifest. \
             aiken_blueprint parses plutus.json (CIP-57). \
             aiken_docs fetches pages from aiken-lang.org with caching. \
             aiken_explain looks up canonical fixes for common Aiken error strings. \
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
