#!/usr/bin/env bash
# Clone or update every repo listed in crates/tools/data/corpora.toml.
#
# Usage: scripts/sync-corpus.sh [TARGET_DIR]
#   TARGET_DIR defaults to $HOME/code/aiken-corpus
#
# After running, set:
#   AIKEN_MCP_CORPUS=/path/to/repo1:/path/to/repo2:...
# (the script prints the recommended value at the end)

set -euo pipefail

TARGET_DIR="${1:-$HOME/code/aiken-corpus}"
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
MANIFEST="$SCRIPT_DIR/../crates/tools/data/corpora.toml"

if [[ ! -f "$MANIFEST" ]]; then
  echo "manifest not found: $MANIFEST" >&2
  exit 1
fi

mkdir -p "$TARGET_DIR"
echo "syncing corpus into: $TARGET_DIR"
echo

# Crude TOML extraction: pull `name = "..."` and `url = "..."` line pairs in
# order. Each [[entries]] block has one of each at line scope; we walk the
# file and pair them up.
declare -a names urls
while IFS= read -r line; do
  if [[ "$line" =~ ^name\ =\ \"([^\"]+)\" ]]; then
    names+=("${BASH_REMATCH[1]}")
  elif [[ "$line" =~ ^url\ =\ \"([^\"]+)\" ]]; then
    urls+=("${BASH_REMATCH[1]}")
  fi
done < "$MANIFEST"

if [[ "${#names[@]}" -ne "${#urls[@]}" ]]; then
  echo "manifest parse error: $((${#names[@]})) names vs $((${#urls[@]})) urls" >&2
  echo "expected one name + one url per [[entries]] block" >&2
  exit 1
fi

declare -a synced_paths
for i in "${!names[@]}"; do
  name="${names[$i]}"
  url="${urls[$i]}"
  dest="$TARGET_DIR/$name"

  if [[ -d "$dest/.git" ]]; then
    echo "[update] $name"
    git -C "$dest" fetch --quiet --depth 1 origin
    git -C "$dest" reset --quiet --hard "$(git -C "$dest" rev-parse @{u})"
  else
    echo "[clone]  $name <- $url"
    git clone --depth 1 --quiet "$url" "$dest"
  fi
  synced_paths+=("$dest")
done

echo
echo "synced ${#synced_paths[@]} repos."
echo
echo "Set this in your shell or in the aiken-mcp registration env:"
echo
joined="$(printf "%s:" "${synced_paths[@]}")"
echo "  AIKEN_MCP_CORPUS=${joined%:}"
echo
echo "If using claude mcp:"
echo "  claude mcp remove -s user aiken"
echo "  claude mcp add -s user -e \"AIKEN_MCP_CORPUS=${joined%:}\" -- aiken /Users/geofflittle/code/aiken-mcp/target/release/aiken-mcp"
