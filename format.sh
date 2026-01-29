#!/usr/bin/env bash
set -euo pipefail

RED='\033[0;31m'
GREEN='\033[0;32m'
NC='\033[0m'

error() { echo -e "${RED}error:${NC} $1" >&2; }
ok() { echo -e "${GREEN}ok${NC} - $1"; }

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"

echo "Running formatters..."
echo ""

echo "Rust (cargo fmt)..."
if cargo fmt --all --manifest-path "$SCRIPT_DIR/Cargo.toml" 2>&1; then
    ok "rustfmt"
else
    error "rustfmt failed"
    exit 1
fi

echo ""
echo "Python..."
if [ -d "$SCRIPT_DIR/python/.venv" ]; then
    source "$SCRIPT_DIR/python/.venv/bin/activate"
    if command -v ruff &>/dev/null; then
        if ruff format "$SCRIPT_DIR/python/srag_ml" "$SCRIPT_DIR/python/tests" 2>&1; then
            ok "ruff format"
        else
            error "ruff format failed"
            exit 1
        fi
    elif command -v black &>/dev/null; then
        if black "$SCRIPT_DIR/python/srag_ml" "$SCRIPT_DIR/python/tests" 2>&1; then
            ok "black"
        else
            error "black failed"
            exit 1
        fi
    else
        echo "  (no python formatter found - install ruff or black)"
    fi
else
    echo "  (python venv not found - run ./install.sh first)"
fi

echo ""
ok "formatting complete"
