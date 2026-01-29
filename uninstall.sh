#!/usr/bin/env bash
set -euo pipefail

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

error() { echo -e "${RED}error:${NC} $1" >&2; }
warn() { echo -e "${YELLOW}warning:${NC} $1"; }
ok() { echo -e "${GREEN}ok${NC} - $1"; }

OS="$(uname -s)"
case "$OS" in
    Linux*)  OS_TYPE="linux" ;;
    Darwin*) OS_TYPE="macos" ;;
    MINGW*|MSYS*|CYGWIN*) error "Windows detected - use WSL instead"; exit 1 ;;
    *) error "unsupported OS: $OS"; exit 1 ;;
esac

if [ "$OS_TYPE" = "macos" ]; then
    DATA_DIR="$HOME/Library/Application Support/srag"
    CONFIG_DIR="$HOME/Library/Application Support/srag"
    BIN_PATH="/usr/local/bin/srag"
    RUNTIME_DIR="/tmp/srag"
else
    DATA_DIR="${XDG_DATA_HOME:-$HOME/.local/share}/srag"
    CONFIG_DIR="${XDG_CONFIG_HOME:-$HOME/.config}/srag"
    BIN_PATH="$HOME/.local/bin/srag"
    RUNTIME_DIR="${XDG_RUNTIME_DIR:-/tmp}/srag"
fi

echo "------------------------------------------------------------------------"
echo "srag uninstall"
echo "------------------------------------------------------------------------"
echo ""

found_something=false
echo "Will remove:"
echo ""
if [ -f "$BIN_PATH" ]; then
    echo "  - Binary:  $BIN_PATH"
    found_something=true
fi
if [ -d "$DATA_DIR" ]; then
    size=$(du -sh "$DATA_DIR" 2>/dev/null | cut -f1 || echo "unknown size")
    echo "  - Data:    $DATA_DIR ($size)"
    found_something=true
fi
if [ -d "$CONFIG_DIR" ] && [ "$CONFIG_DIR" != "$DATA_DIR" ]; then
    echo "  - Config:  $CONFIG_DIR"
    found_something=true
fi
if [ -d "$RUNTIME_DIR" ]; then
    echo "  - Runtime: $RUNTIME_DIR"
    found_something=true
fi

if [ "$found_something" = false ]; then
    echo "  (nothing found - srag may not be installed)"
    echo ""
    exit 0
fi

echo ""
read -p "Proceed with uninstall? [y/N] " -n 1 -r
echo ""

if [[ ! $REPLY =~ ^[Yy]$ ]]; then
    echo "Cancelled"
    exit 0
fi

echo ""

if [ -f "$BIN_PATH" ]; then
    if pgrep -f "srag watch" >/dev/null 2>&1; then
        echo "Stopping watcher process..."
        if "$BIN_PATH" watch --stop 2>/dev/null; then
            ok "watcher stopped"
        else
            warn "could not stop watcher - it may still be running"
            echo "  try: pkill -f 'srag watch'"
        fi
        sleep 1
    fi

    if pgrep -f "srag mcp" >/dev/null 2>&1; then
        warn "srag mcp server is running - it will be orphaned"
        echo "  restart any tools using it (Claude Code, etc.)"
    fi
fi

if [ -f "$BIN_PATH" ]; then
    echo "Removing binary..."
    if [ "$OS_TYPE" = "macos" ]; then
        if sudo rm -f "$BIN_PATH" 2>/dev/null; then
            ok "binary removed"
        else
            error "failed to remove binary (sudo required)"
            echo "  try: sudo rm -f $BIN_PATH"
        fi
    else
        if rm -f "$BIN_PATH" 2>/dev/null; then
            ok "binary removed"
        else
            error "failed to remove binary"
            echo "  check permissions on $BIN_PATH"
        fi
    fi
fi

if [ -d "$RUNTIME_DIR" ]; then
    echo "Removing runtime files..."
    if rm -rf "$RUNTIME_DIR" 2>/dev/null; then
        ok "runtime removed"
    else
        warn "could not remove runtime dir"
    fi
fi

if [ -d "$DATA_DIR" ]; then
    echo "Removing data directory..."
    if rm -rf "$DATA_DIR" 2>/dev/null; then
        ok "data removed"
    else
        error "failed to remove data directory"
        echo "  try: rm -rf \"$DATA_DIR\""
    fi
fi

if [ -d "$CONFIG_DIR" ] && [ "$CONFIG_DIR" != "$DATA_DIR" ]; then
    echo "Removing config directory..."
    if rm -rf "$CONFIG_DIR" 2>/dev/null; then
        ok "config removed"
    else
        warn "could not remove config dir"
    fi
fi

remove_claude_mcp() {
    local claude_configs=(
        "$HOME/.claude.json"
        "${XDG_CONFIG_HOME:-$HOME/.config}/claude/settings.json"
        "$HOME/.claude/settings.json"
    )

    if ! command -v jq &>/dev/null; then
        echo ""
        warn "jq not found - cannot auto-remove Claude MCP config"
        echo "  manually remove 'srag' from mcpServers in your Claude config"
        return
    fi

    for config_file in "${claude_configs[@]}"; do
        if [ -f "$config_file" ]; then
            if ! jq empty "$config_file" 2>/dev/null; then
                warn "invalid JSON in $config_file - skipping"
                continue
            fi

            local has_srag=$(jq '.mcpServers | has("srag")' "$config_file" 2>/dev/null || echo "false")
            if [ "$has_srag" = "true" ]; then
                echo ""
                echo "Found srag in Claude config: $config_file"
                read -p "Remove srag from Claude MCP config? [y/N] " -n 1 -r
                echo ""

                if [[ $REPLY =~ ^[Yy]$ ]]; then
                    local backup="${config_file}.bak.$(date +%s)"
                    if ! cp "$config_file" "$backup" 2>/dev/null; then
                        error "failed to create backup"; return
                    fi

                    if ! jq 'del(.mcpServers.srag)' "$config_file" > "${config_file}.tmp" 2>/dev/null; then
                        error "jq failed - restoring backup"
                        cp "$backup" "$config_file"
                        return
                    fi
                    mv "${config_file}.tmp" "$config_file"

                    local remaining=$(jq '.mcpServers | length' "$config_file" 2>/dev/null || echo "0")
                    if [ "$remaining" = "0" ]; then
                        jq 'del(.mcpServers)' "$config_file" > "${config_file}.tmp" 2>/dev/null
                        mv "${config_file}.tmp" "$config_file"
                    fi

                    ok "removed from Claude config (backup: $backup)"
                fi
                break
            fi
        fi
    done
}

remove_claude_mcp

echo ""
echo "------------------------------------------------------------------------"
ok "Uninstall complete"
echo "------------------------------------------------------------------------"
