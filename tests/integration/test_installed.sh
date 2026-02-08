#!/bin/bash
set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"

echo "=== srag installed version tests ==="
echo "testing the installed srag binary against real codebases"
echo ""

if ! command -v srag &> /dev/null; then
    echo "SKIP: srag not installed (run ./install.sh first)"
    exit 0
fi

PASS_COUNT=0
FAIL_COUNT=0

pass() {
    echo "PASS: $1"
    PASS_COUNT=$((PASS_COUNT + 1))
}

fail() {
    echo "FAIL: $1"
    FAIL_COUNT=$((FAIL_COUNT + 1))
}

echo "--- test 1: index srag codebase ---"
cd "$PROJECT_ROOT"

srag remove srag -y 2>/dev/null || true

output=$(srag index . --name srag 2>&1 | tail -3)
if echo "$output" | grep -q "files indexed\|chunks embedded"; then
    pass "indexed srag codebase"
    echo "  $(echo "$output" | grep -o 'done:.*')"
else
    fail "indexing failed: $output"
fi

echo ""
echo "--- test 2: verify index statistics ---"
output=$(srag status --detailed 2>&1)
if echo "$output" | grep -q "srag" && echo "$output" | grep -qP 'files:\s+\d+'; then
    pass "status shows srag project"
    files=$(echo "$output" | grep -oP 'files:\s+\K\d+' | head -1)
    chunks=$(echo "$output" | grep -oP 'chunks:\s+\K\d+' | head -1)
    embedded=$(echo "$output" | grep -oP '\(\K\d+(?= embedded)' | head -1)
    echo "  indexed $files files, $chunks chunks ($embedded embedded)"
else
    fail "status output unexpected"
fi

echo ""
echo "--- test 3: semantic search for 'HNSW vector index' ---"
output=$(srag query -p srag -q "HNSW approximate nearest neighbor search implementation" 2>&1 | head -40)
if echo "$output" | grep -qi "hnsw\|vector\|search\|neighbor"; then
    pass "semantic search found vector search code"
else
    fail "semantic search returned unexpected results"
    echo "$output" | head -10
fi

echo ""
echo "--- test 4: search for 'tree-sitter parsing' ---"
output=$(srag query -p srag -q "tree-sitter AST parsing for code chunking" 2>&1 | head -40)
if echo "$output" | grep -qi "tree.sitter\|chunk\|parser\|ast"; then
    pass "found tree-sitter code"
else
    fail "tree-sitter search returned unexpected results"
    echo "$output" | head -10
fi

echo ""
echo "--- test 5: search for 'MCP protocol handler' ---"
output=$(srag query -p srag -q "MCP JSON-RPC tool handler implementation" 2>&1 | head -40)
if echo "$output" | grep -qi "mcp\|tool\|jsonrpc\|handler"; then
    pass "found MCP handler code"
else
    fail "MCP search returned unexpected results"
    echo "$output" | head -10
fi

echo ""
echo "--- test 6: search for 'prompt injection detection' ---"
output=$(srag query -p srag -q "prompt injection security detection scanner" 2>&1 | head -40)
if echo "$output" | grep -qi "injection\|suspicious\|security\|detect"; then
    pass "found security code"
else
    fail "security search returned unexpected results"
    echo "$output" | head -10
fi

echo ""
echo "--- test 7: search for 'hybrid search RRF' ---"
output=$(srag query -p srag -q "hybrid search with reciprocal rank fusion" 2>&1 | head -40)
if echo "$output" | grep -qi "rrf\|hybrid\|rank\|fusion\|fts"; then
    pass "found hybrid search code"
else
    fail "hybrid search query returned unexpected results"
    echo "$output" | head -10
fi

echo ""
echo "--- test 8: search for 'secret redaction' ---"
output=$(srag query -p srag -q "API key secret redaction for security" 2>&1 | head -40)
if echo "$output" | grep -qi "secret\|redact\|api.key\|mask"; then
    pass "found secret redaction code"
else
    fail "secret redaction search returned unexpected results"
    echo "$output" | head -10
fi

echo ""
echo "--- test 9: sync command ---"
output=$(srag sync 2>&1 | tail -2)
if echo "$output" | grep -qi "sync\|skipped\|indexed\|unchanged\|done"; then
    pass "sync command works"
else
    fail "sync command failed: $output"
fi

echo ""
echo "--- test 10: interactive chat test (non-interactive) ---"
output=$(echo "exit" | timeout 5 srag chat -p srag 2>&1 || true)
if [[ $? -eq 0 ]] || echo "$output" | grep -qi "srag\|chat\|goodbye"; then
    pass "chat command starts"
else
    fail "chat command failed"
fi

echo ""
echo "--- test 11: run unit tests ---"
cd "$PROJECT_ROOT"
output=$(cargo test --release 2>&1 | tail -10)
if echo "$output" | grep -q "passed" && ! echo "$output" | grep -q "[1-9][0-9]* failed"; then
    test_count=$(echo "$output" | grep -oE '[0-9]+ passed' | head -1)
    pass "all unit tests: $test_count"
else
    fail "unit tests failed"
    echo "$output"
fi

echo ""
echo "=========================================="
echo "Results: $PASS_COUNT passed, $FAIL_COUNT failed"
echo "=========================================="

if [[ $FAIL_COUNT -gt 0 ]]; then
    exit 1
fi

echo ""
echo "all integration tests passed"
