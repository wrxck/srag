#!/usr/bin/env bash
set -euo pipefail

RED='\033[0;31m'
GREEN='\033[0;32m'
NC='\033[0m'

error() { echo -e "${RED}error:${NC} $1" >&2; }
ok() { echo -e "${GREEN}ok${NC} - $1"; }

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
failed=false

echo "Running linters..."
echo ""

echo "Rust (clippy)..."
if cargo clippy --workspace --manifest-path "$SCRIPT_DIR/Cargo.toml" -- -D warnings 2>&1; then
    ok "clippy"
else
    error "clippy found issues"
    failed=true
fi

echo ""
echo "Rust (fmt check)..."
if cargo fmt --all --manifest-path "$SCRIPT_DIR/Cargo.toml" -- --check 2>&1; then
    ok "rustfmt"
else
    error "rustfmt found unformatted code (run ./format.sh)"
    failed=true
fi

echo ""
echo "Python..."
if [ -d "$SCRIPT_DIR/python/.venv" ]; then
    source "$SCRIPT_DIR/python/.venv/bin/activate"
    if command -v ruff &>/dev/null; then
        if ruff check "$SCRIPT_DIR/python/srag_ml" 2>&1; then
            ok "ruff"
        else
            error "ruff found issues"
            failed=true
        fi
    elif command -v flake8 &>/dev/null; then
        if flake8 "$SCRIPT_DIR/python/srag_ml" --max-line-length=100 2>&1; then
            ok "flake8"
        else
            error "flake8 found issues"
            failed=true
        fi
    else
        echo "  (no python linter found - install ruff or flake8)"
    fi
else
    echo "  (python venv not found - run ./install.sh first)"
fi

echo ""
if [ "$failed" = true ]; then
    error "linting failed"
    exit 1
else
    ok "all lints passed"
fi
