# aiken-mcp

An MCP server that gives LLM coding assistants (Claude Code, Cursor, etc.) hands-on tools for writing [Aiken](https://aiken-lang.org) smart contracts.

## What it gives the LLM

| Capability | Tools |
|---|---|
| Compile + test + budget | `aiken_check` (diagnostics, per-test pass/fail, mem/cpu, % of tx limit, `clean: true` for stale-cache retry), `aiken_build` |
| Format + scaffold | `aiken_fmt`, `aiken_new` |
| Type-aware queries (LSP) | `aiken_hover`, `aiken_completions`, `aiken_definition` |
| Inspect artifacts | `aiken_blueprint` (CIP-57 plutus.json), `aiken_uplc` |
| Learn from real code | `aiken_corpus_list`, `aiken_pattern_search`, `aiken_symbol_lookup` |
| Reference + recovery | `aiken_docs` (aiken-lang.org with cache), `aiken_explain` (canonical error fixes) |
| Meta | `aiken_version` |

The corpus tools index curated high-quality Aiken codebases (aiken-lang/stdlib, microproofs, Anastasia Labs, SundaeSwap, Spectrum, etc.) so the LLM can find idiomatic patterns by name or doc text.

Recommended loop after every edit: `aiken_check` + `aiken_fmt`. Before referencing a symbol from another module or stdlib: `aiken_symbol_lookup`.

## Quick start

### 1. Install prerequisites

- Rust 1.75+
- [Aiken CLI](https://aiken-lang.org/installation-instructions) on PATH
- [ripgrep](https://github.com/BurntSushi/ripgrep) on PATH (only needed for `aiken_pattern_search`)

### 2. Install the server

```sh
git clone https://github.com/geofflittle/aiken-mcp
cd aiken-mcp
cargo install --path crates/server
```

Binary lands at `~/.cargo/bin/aiken-mcp` (already on PATH if you have a normal Rust setup). Re-run with `--force` to upgrade after pulling.

### 3. Register with your client

**Claude Code (recommended):**

```sh
claude mcp add -s user aiken aiken-mcp
```

**Manual config** (any MCP client). Add to your client config:

```json
{
  "mcpServers": {
    "aiken": {
      "command": "aiken-mcp",
      "env": {
        "AIKEN_MCP_CORPUS": "/path/to/aiken-stdlib:/path/to/another-repo"
      }
    }
  }
}
```

If `~/.cargo/bin` is not on PATH for your client, use the absolute path `/Users/<you>/.cargo/bin/aiken-mcp`.

### 4. (Optional) Clone the curated corpus

For best results with `aiken_symbol_lookup` and `aiken_pattern_search`, clone the reference codebases:

```sh
scripts/sync-corpus.sh ~/code/aiken-corpus
```

Then point `AIKEN_MCP_CORPUS` at that directory (colon-separated paths).

### 5. Verify

Restart your client. Ask it: *"Use aiken_version to check the Aiken CLI is wired up."*

## Configuration

All optional. Set via the `env` block in your client config.

| Var | Default | Purpose |
|---|---|---|
| `AIKEN_MCP_CORPUS` | empty | Colon-separated paths to reference Aiken codebases |
| `AIKEN_MCP_DOCS_BASE_URL` | `https://aiken-lang.org` | Override docs source |
| `AIKEN_MCP_DOCS_CACHE` | `~/Library/Caches/aiken-mcp/docs` (macOS), `$XDG_CACHE_HOME/aiken-mcp/docs` (Linux) | Docs disk cache |
| `AIKEN_MCP_LOG` | `info` | `tracing-subscriber` filter |

## Architecture

Workspace of 9 crates, ports-and-adapters layout. `core` defines traits, side-effect crates implement them, `server` wires everything behind an rmcp stdio transport.

```
crates/
├── core/        traits + domain types (no I/O)
├── cli/         AikenRunner over the `aiken` subprocess
├── corpus/      CorpusSearch over ripgrep
├── docs/        DocsFetcher over reqwest + on-disk cache
├── lsp/         LspClient over `aiken lsp --stdio`
├── symbols/     SymbolIndex by walking .ak files
├── blueprint/   BlueprintReader for plutus.json
├── tools/       handler functions over the core traits
└── server/      bin. rmcp stdio. Wires deps + registers tools.
```

Adding a new tool: write a handler in `tools/`, register a `#[tool]` method in `server/src/server.rs`. No churn elsewhere unless a new trait is needed.

## Development

```sh
cargo build --workspace
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all
```

## License

MIT OR Apache-2.0
