#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

error() { echo -e "${RED}error:${NC} $1" >&2; }
warn() { echo -e "${YELLOW}warning:${NC} $1"; }
ok() { echo -e "${GREEN}ok${NC} - $1"; }

cleanup() {
    if [ "${INSTALL_FAILED:-false}" = true ]; then
        error "installation failed - see errors above"
        [ -d "$VENV_DIR" ] && [ "${VENV_CREATED:-false}" = true ] && rm -rf "$VENV_DIR"
    fi
}
trap cleanup EXIT

OS="$(uname -s)"
case "$OS" in
    Linux*)  OS_TYPE="linux" ;;
    Darwin*) OS_TYPE="macos" ;;
    MINGW*|MSYS*|CYGWIN*) error "Windows detected - use WSL instead"; exit 1 ;;
    *) error "unsupported OS: $OS"; exit 1 ;;
esac

echo "Detected OS: $OS_TYPE"

if [ "$OS_TYPE" = "macos" ]; then
    DATA_DIR="$HOME/Library/Application Support/srag"
    CONFIG_DIR="$HOME/Library/Application Support/srag"
    BIN_DIR="/usr/local/bin"
else
    DATA_DIR="${XDG_DATA_HOME:-$HOME/.local/share}/srag"
    CONFIG_DIR="${XDG_CONFIG_HOME:-$HOME/.config}/srag"
    BIN_DIR="$HOME/.local/bin"
fi

VENV_DIR="$DATA_DIR/venv"
PYTHON_DIR="$SCRIPT_DIR/python"

check_command() { command -v "$1" &>/dev/null; }

check_deps() {
    local missing=()

    if ! check_command cargo; then
        missing+=("rust")
    elif [ -n "$(rustc --version 2>/dev/null | cut -d' ' -f2)" ]; then
        local v=$(rustc --version | cut -d' ' -f2)
        local major=$(echo "$v" | cut -d. -f1)
        local minor=$(echo "$v" | cut -d. -f2)
        [ "$major" -lt 1 ] || ([ "$major" -eq 1 ] && [ "$minor" -lt 75 ]) && warn "rust $v found, 1.75+ recommended"
    fi

    if ! check_command python3; then
        missing+=("python3")
    else
        local pv=$(python3 -c 'import sys; print(f"{sys.version_info.major}.{sys.version_info.minor}")' 2>/dev/null)
        local pmaj=$(echo "$pv" | cut -d. -f1)
        local pmin=$(echo "$pv" | cut -d. -f2)
        if [ "$pmaj" -lt 3 ] || ([ "$pmaj" -eq 3 ] && [ "$pmin" -lt 10 ]); then
            error "python $pv found, but 3.10+ required"; missing+=("python3.10+")
        fi
        python3 -c "import venv" &>/dev/null || missing+=("python3-venv")
    fi

    check_command jq || warn "jq not found - Claude Code auto-config will be skipped"

    if [ ${#missing[@]} -gt 0 ]; then
        error "missing: ${missing[*]}"
        if [ "$OS_TYPE" = "macos" ]; then
            echo "  brew install rust python@3.12 jq"
        else
            echo "  Ubuntu: sudo apt install rustc cargo python3 python3-venv jq"
            echo "  Fedora: sudo dnf install rust cargo python3 jq"
        fi
        exit 1
    fi
}

check_disk_space() {
    local avail=$(df -m "$HOME" | tail -1 | awk '{print $4}')
    if [ "$avail" -lt 2000 ]; then
        error "need 2GB disk space, only ${avail}MB available"; exit 1
    fi
}

check_network() {
    if ! curl -sf --connect-timeout 5 "https://huggingface.co" >/dev/null 2>&1; then
        curl -sf --connect-timeout 5 "https://github.com" >/dev/null 2>&1 || { error "no network"; exit 1; }
        warn "huggingface.co unreachable - model downloads may fail"
    fi
}

echo ""; echo "Checking dependencies..."; check_deps; ok "dependencies"
echo ""; echo "Checking disk space..."; check_disk_space; ok "disk space"
echo ""; echo "Checking network..."; check_network; ok "network"

echo ""
mkdir -p "$DATA_DIR/models" "$DATA_DIR/logs" "$CONFIG_DIR" || { error "failed to create directories"; exit 1; }
[ "$OS_TYPE" = "linux" ] && mkdir -p "$BIN_DIR"

echo "Building srag binary..."
if ! cargo build --release --manifest-path "$SCRIPT_DIR/Cargo.toml" 2>&1; then
    INSTALL_FAILED=true
    error "cargo build failed"
    echo "  - install build-essential/base-devel"
    echo "  - try: rustup update"
    echo "  - try: rm -rf target/ and retry"
    exit 1
fi
ok "build"

echo ""; echo "Installing binary..."
if [ -f "$BIN_DIR/srag" ] && pgrep -x srag >/dev/null 2>&1; then
    echo "Stopping running srag processes..."
    pkill -x srag 2>/dev/null || true
    sleep 1
fi
if [ "$OS_TYPE" = "macos" ]; then
    sudo cp "$SCRIPT_DIR/target/release/srag" "$BIN_DIR/srag" || { INSTALL_FAILED=true; error "sudo cp failed"; exit 1; }
    sudo chmod +x "$BIN_DIR/srag"
else
    cp "$SCRIPT_DIR/target/release/srag" "$BIN_DIR/srag" || { INSTALL_FAILED=true; error "cp failed"; exit 1; }
    chmod +x "$BIN_DIR/srag"
fi
ok "installed to $BIN_DIR/srag"

echo ""
if [ -d "$VENV_DIR" ]; then
    echo "Using existing venv"
else
    echo "Creating Python venv..."
    VENV_CREATED=true
    python3 -m venv "$VENV_DIR" || { INSTALL_FAILED=true; error "venv creation failed - apt install python3-venv"; exit 1; }
    ok "venv created"
fi

echo ""; echo "Installing Python dependencies..."
source "$VENV_DIR/bin/activate"
pip install --upgrade pip --quiet 2>&1 || warn "pip upgrade failed"
pip install -e "$PYTHON_DIR" 2>&1 || { INSTALL_FAILED=true; error "pip install failed"; exit 1; }
ok "python deps"

echo ""; echo "Downloading ML models (takes a while first time)..."
python3 -c "
import sys
from srag_ml.embedder import Embedder
from srag_ml.reranker import Reranker
from srag_ml.models import download_model
from pathlib import Path
models_dir = Path('$DATA_DIR/models')
try:
    print('  embedder...'); Embedder(cache_dir=str(models_dir)).load()
    print('  reranker...'); Reranker(cache_dir=str(models_dir)).load()
    print('  LLM...'); download_model(models_dir)
except Exception as e:
    print(f'failed: {e}', file=sys.stderr); sys.exit(1)
" || { INSTALL_FAILED=true; error "model download failed - check network/disk space"; exit 1; }
ok "models"

configure_claude_mcp() {
    local srag_bin="$BIN_DIR/srag"
    local config_file=""
    for f in "$HOME/.claude.json" "${XDG_CONFIG_HOME:-$HOME/.config}/claude/settings.json" "$HOME/.claude/settings.json"; do
        [ -f "$f" ] && config_file="$f" && break
    done

    echo ""; echo "------------------------------------------------------------------------"
    echo "Claude Code Integration"
    echo "------------------------------------------------------------------------"
    echo "Add srag as MCP server for codebase search."
    echo ""

    local already=false
    if [ -n "$config_file" ] && check_command jq; then
        [ "$(jq '.mcpServers | has("srag")' "$config_file" 2>/dev/null)" = "true" ] && already=true
    fi

    if [ "$already" = true ]; then
        echo "srag already configured in $config_file"
        read -p "Update config? [y/N] " -n 1 -r; echo ""
        [[ ! $REPLY =~ ^[Yy]$ ]] && return
    else
        [ -z "$config_file" ] && config_file="$HOME/.claude.json"
        echo "Will use: $config_file"
        read -p "Configure Claude? [y/n] " -n 1 -r; echo ""
        [[ ! $REPLY =~ ^[Yy]$ ]] && { echo "Skip. Add manually: \"mcpServers\":{\"srag\":{\"command\":\"$srag_bin\",\"args\":[\"mcp\"]}}"; return; }
    fi

    if ! check_command jq; then
        warn "jq not found - add manually"; return
    fi

    if [ -f "$config_file" ]; then
        jq empty "$config_file" 2>/dev/null || { error "invalid JSON in $config_file"; return; }
        cp "$config_file" "${config_file}.bak"
        local has_mcp=$(jq 'has("mcpServers")' "$config_file")
        if [ "$has_mcp" = "true" ]; then
            jq --arg cmd "$srag_bin" '.mcpServers.srag = {"command": $cmd, "args": ["mcp"]}' "$config_file" > "${config_file}.tmp"
        else
            jq --arg cmd "$srag_bin" '. + {"mcpServers": {"srag": {"command": $cmd, "args": ["mcp"]}}}' "$config_file" > "${config_file}.tmp"
        fi
        mv "${config_file}.tmp" "$config_file"
    else
        mkdir -p "$(dirname "$config_file")"
        echo "{\"mcpServers\":{\"srag\":{\"command\":\"$srag_bin\",\"args\":[\"mcp\"]}}}" | jq . > "$config_file"
        chmod 600 "$config_file"
    fi
    ok "Claude MCP configured - restart Claude Code"
}

configure_claude_mcp

echo ""; echo "------------------------------------------------------------------------"
ok "Install complete!"
echo "------------------------------------------------------------------------"

if [[ ":$PATH:" != *":$BIN_DIR:"* ]]; then
    warn "$BIN_DIR not in PATH"
    if [ "$OS_TYPE" = "macos" ]; then
        echo "  echo 'export PATH=\"/usr/local/bin:\$PATH\"' >> ~/.zshrc && source ~/.zshrc"
    else
        echo "  echo 'export PATH=\"\$HOME/.local/bin:\$PATH\"' >> ~/.bashrc && source ~/.bashrc"
    fi
fi

echo ""; echo "Get started:"; echo "  srag index /path/to/project"; echo "  srag chat"
