# aiken-mcp

MCP server for Aiken development. Exposes the Aiken CLI, a reference-corpus
search, and Aiken docs fetching as MCP tools so an LLM client can iterate on
Aiken code with tight feedback.

## Tools

| Name | Purpose |
|---|---|
| `aiken_check` | Run `aiken check` on a project. Returns diagnostics. |
| `aiken_build` | Run `aiken build`. Returns diagnostics + artifacts. |
| `aiken_test` | Run aiken tests (via `aiken check`). Returns pass/fail per test. |
| `aiken_fmt` | Format inline source via `aiken fmt --stdin`. |
| `aiken_pattern_search` | Ripgrep over user-supplied reference Aiken codebases. |
| `aiken_docs` | Fetch a page from `aiken-lang.org` with on-disk cache. |
| `aiken_version` | Report the installed Aiken CLI version. |
| `aiken_hover` | LSP hover at file/line/column via `aiken lsp --stdio`. |
| `aiken_completions` | LSP completions. |
| `aiken_definition` | LSP go-to-definition. |
| `aiken_budget` | Per-test mem/cpu vs Plutus tx limit (%). |
| `aiken_symbol_lookup` | Index `pub fn`/`pub type`/`pub const`/`validator` + preceding `///` docs across the corpus. Query matches name OR doc text. |
| `aiken_blueprint` | Parse `plutus.json` (CIP-57): validators, hashes, schemas, sizes. |
| `aiken_uplc` | `aiken uplc decode <target>`. |
| `aiken_new` | `aiken new` scaffolder. |
| `aiken_explain` | Static error → fix lookup. |
| `aiken_corpus_list` | List curated high-expertise Aiken codebases (from `corpora.toml`). |

## Architecture

Workspace of six crates:

```
crates/
├── core/     # domain types + traits (no transport, no CLI)
├── cli/      # AikenRunner impl wrapping the `aiken` CLI subprocess
├── corpus/   # CorpusSearch impl wrapping ripgrep
├── docs/     # DocsFetcher impl with reqwest + on-disk cache
├── tools/    # tool handlers (depend only on core traits)
└── server/   # bin crate. rmcp stdio transport. Wires tools + impls.
```

Decoupling:

- `core` knows nothing about MCP, HTTP, subprocesses, or filesystem layout.
  Exposes traits (`AikenRunner`, `CorpusSearch`, `DocsFetcher`) + domain types.
- `cli`, `corpus`, `docs` are independent impls of the core traits. Swap any
  for a fake in tests or alternate impl in production.
- `tools` handlers take trait objects (`Arc<dyn AikenRunner>`, etc.). No
  knowledge of how those traits are realized.
- `server` is the only crate that touches `rmcp`. All MCP-specific concerns
  (tool routing, JSON Schema, CallToolResult) stay isolated here.

Adding a new tool: add a handler in `tools/`, add a `#[tool]` registration in
`server/src/server.rs`. No churn to `core` unless a new trait is needed.

## Building

```sh
cd ~/code/aiken-mcp
cargo build --release
```

Binary lands at `target/release/aiken-mcp`.

## Registering with Claude Code

User scope (every Claude session, every project):

```sh
claude mcp add -s user aiken /Users/geofflittle/code/aiken-mcp/target/release/aiken-mcp
```

Or hand-edit `~/.claude.json`:

```json
{
  "mcpServers": {
    "aiken": {
      "command": "/Users/geofflittle/code/aiken-mcp/target/release/aiken-mcp",
      "env": {
        "AIKEN_MCP_CORPUS": "/Users/geofflittle/code/midnight-reserve-contracts:/Users/geofflittle/code/aiken-stdlib",
        "AIKEN_MCP_LOG": "info"
      }
    }
  }
}
```

Project scope: drop a `.mcp.json` at the project root with the same shape
(overrides user scope when both exist).

## Curated corpus

`crates/tools/data/corpora.toml` is a hand-curated list of high-expertise
Aiken codebases (aiken-lang, microproofs, Anastasia Labs, SundaeSwap,
Spectrum, etc.). The MCP exposes it via `aiken_corpus_list` (filterable by
tag).

Patterns are not separately catalogued. Instead, `aiken_symbol_lookup` reads
the `///` doc comments authors already write above their public symbols and
matches queries against both names and doc text. This lets you search by
topic (e.g. "merkle proof") without hand-curating a pattern catalog.

To clone everything in the manifest:

```sh
scripts/sync-corpus.sh ~/code/aiken-corpus
```

Add new repos by editing `corpora.toml` and rebuilding.

## Configuration (env vars)

| Var | Default | Purpose |
|---|---|---|
| `AIKEN_MCP_CORPUS` | empty | Colon-separated absolute paths to reference Aiken codebases |
| `AIKEN_MCP_DOCS_BASE_URL` | `https://aiken-lang.org` | Override docs base URL |
| `AIKEN_MCP_DOCS_CACHE` | `~/Library/Caches/aiken-mcp/docs` (macOS) or `$XDG_CACHE_HOME/aiken-mcp/docs` | Disk cache directory |
| `AIKEN_MCP_LOG` | `info` | tracing-subscriber `EnvFilter` directives |

## Requirements

- Rust 1.75+
- Aiken CLI on PATH (`aikup install` or equivalent)
- `rg` (ripgrep) on PATH if `aiken_pattern_search` is used

## Development

```sh
cargo build --workspace
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all
```

## License

MIT OR Apache-2.0
