#!/usr/bin/env bash
set -euo pipefail

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
NC='\033[0m'

error() { echo -e "${RED}error:${NC} $1" >&2; }
ok() { echo -e "${GREEN}ok${NC} - $1"; }
warn() { echo -e "${YELLOW}warning:${NC} $1"; }

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
failed=false
rust_passed=0
rust_failed=0
python_passed=0
python_failed=0

echo "Running tests..."
echo ""

echo "Rust tests..."
if output=$(cargo test --workspace --manifest-path "$SCRIPT_DIR/Cargo.toml" 2>&1); then
    rust_passed=$(echo "$output" | grep -oE "[0-9]+ passed" | cut -d' ' -f1 | awk '{s+=$1} END {print s+0}')
    ok "rust ($rust_passed passed)"
else
    error "rust tests failed"
    echo "$output"
    failed=true
fi

echo ""
echo "Python tests..."
if [ -d "$SCRIPT_DIR/python/.venv" ]; then
    source "$SCRIPT_DIR/python/.venv/bin/activate"
    if command -v pytest &>/dev/null; then
        if output=$(python -m pytest "$SCRIPT_DIR/python/tests" -v 2>&1); then
            python_passed=$(echo "$output" | grep -oE "[0-9]+ passed" | tail -1 | cut -d' ' -f1 || echo "0")
            ok "python ($python_passed passed)"
        else
            error "python tests failed"
            echo "$output" | tail -30
            failed=true
        fi
    else
        warn "pytest not found - install with: pip install pytest"
    fi
else
    warn "python venv not found - run ./install.sh first"
fi

echo ""
echo "----------------------------------------"
if [ "$failed" = true ]; then
    error "some tests failed"
    exit 1
else
    total=$((rust_passed + python_passed))
    ok "all tests passed ($total total)"
fi
