#!/bin/bash
set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
TEST_DIR=$(mktemp -d)
SRAG_BIN="$PROJECT_ROOT/target/release/srag"

cleanup() {
    echo ""
    echo "cleaning up $TEST_DIR..."
    rm -rf "$TEST_DIR"
}
trap cleanup EXIT

echo "=== srag real codebase integration tests ==="
echo "testing against: $PROJECT_ROOT (the srag codebase itself)"
echo "test data directory: $TEST_DIR"
echo ""

cd "$PROJECT_ROOT"
cargo build --release 2>&1 | tail -1

if [[ ! -x "$SRAG_BIN" ]]; then
    echo "FAIL: srag binary not found at $SRAG_BIN"
    exit 1
fi

export XDG_DATA_HOME="$TEST_DIR"
export XDG_CONFIG_HOME="$TEST_DIR/config"
mkdir -p "$TEST_DIR/config/srag"
mkdir -p "$TEST_DIR/srag"

if [[ -d "$PROJECT_ROOT/python/.venv" ]]; then
    ln -s "$PROJECT_ROOT/python/.venv" "$TEST_DIR/srag/venv"
    echo "linked development python venv"
fi

PASS_COUNT=0
FAIL_COUNT=0

pass() {
    echo "PASS: $1"
    ((PASS_COUNT++))
}

fail() {
    echo "FAIL: $1"
    ((FAIL_COUNT++))
}

echo "--- test 1: index srag codebase ---"
cd "$PROJECT_ROOT"
output=$("$SRAG_BIN" index . --name srag 2>&1 | tail -5)
if echo "$output" | grep -q "files indexed\|chunks embedded"; then
    pass "indexed srag codebase"
    echo "  $output" | tail -1
else
    fail "indexing failed: $output"
fi

echo ""
echo "--- test 2: verify index statistics ---"
output=$("$SRAG_BIN" status --detailed 2>&1)
if echo "$output" | grep -q "srag" && echo "$output" | grep -q "files:"; then
    pass "status shows srag project"
    files=$(echo "$output" | grep -oP 'files:\s+\K\d+' | head -1)
    chunks=$(echo "$output" | grep -oP 'chunks:\s+\K\d+' | head -1)
    echo "  indexed $files files, $chunks chunks"
else
    fail "status output unexpected: $output"
fi

echo ""
echo "--- test 3: search for 'vector search' ---"
output=$("$SRAG_BIN" query -p srag -q "vector search implementation" 2>&1 | head -30)
if echo "$output" | grep -qi "hnsw\|vector\|search\|embed"; then
    pass "semantic search found relevant code"
else
    fail "semantic search returned unexpected results"
    echo "$output"
fi

echo ""
echo "--- test 4: search for 'MCP server' ---"
output=$("$SRAG_BIN" query -p srag -q "MCP server tool handlers" 2>&1 | head -30)
if echo "$output" | grep -qi "mcp\|tool\|server\|handler"; then
    pass "found MCP-related code"
else
    fail "MCP search returned unexpected results"
    echo "$output"
fi

echo ""
echo "--- test 5: search for 'tree-sitter chunking' ---"
output=$("$SRAG_BIN" query -p srag -q "tree-sitter AST parsing for code chunking" 2>&1 | head -30)
if echo "$output" | grep -qi "tree.sitter\|chunk\|ast\|parser"; then
    pass "found tree-sitter chunking code"
else
    fail "tree-sitter search returned unexpected results"
    echo "$output"
fi

echo ""
echo "--- test 6: search for 'prompt injection' ---"
output=$("$SRAG_BIN" query -p srag -q "prompt injection detection security" 2>&1 | head -30)
if echo "$output" | grep -qi "injection\|suspicious\|security\|scanner"; then
    pass "found security/injection code"
else
    fail "security search returned unexpected results"
    echo "$output"
fi

echo ""
echo "--- test 7: test file watcher starts ---"
"$SRAG_BIN" watch --foreground &
WATCHER_PID=$!
sleep 2
if kill -0 $WATCHER_PID 2>/dev/null; then
    pass "file watcher started successfully"
    kill $WATCHER_PID 2>/dev/null || true
    wait $WATCHER_PID 2>/dev/null || true
else
    fail "file watcher failed to start"
fi

echo ""
echo "--- test 8: test sync command ---"
output=$("$SRAG_BIN" sync 2>&1 | tail -3)
if echo "$output" | grep -qi "sync\|skipped\|indexed\|unchanged"; then
    pass "sync command works"
else
    fail "sync command failed: $output"
fi

echo ""
echo "--- test 9: test remove and re-index ---"
"$SRAG_BIN" remove srag -y 2>&1 | tail -1
output=$("$SRAG_BIN" status 2>&1)
if echo "$output" | grep -q "projects: 0"; then
    pass "project removed successfully"
else
    fail "project removal failed"
fi

"$SRAG_BIN" index . --name srag 2>&1 | tail -1
output=$("$SRAG_BIN" status 2>&1)
if echo "$output" | grep -q "projects: 1"; then
    pass "project re-indexed successfully"
else
    fail "re-indexing failed"
fi

echo ""
echo "--- test 10: test auto-index config ---"
rm -rf "$TEST_DIR/srag"

cat > "$TEST_DIR/config/srag/config.toml" <<EOF
[mcp]
auto_index_cwd = false
EOF

cd "$PROJECT_ROOT"
output=$(echo '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"test","version":"1.0"}}}' | timeout 3 "$SRAG_BIN" mcp 2>&1 || true)
if echo "$output" | grep -q "protocolVersion\|capabilities"; then
    pass "MCP server initialises correctly"
else
    fail "MCP server failed to initialise"
fi

echo ""
echo "--- test 11: index multiple projects ---"
mkdir -p "$TEST_DIR/projects/project_a/src"
mkdir -p "$TEST_DIR/projects/project_b/src"

cat > "$TEST_DIR/projects/project_a/Cargo.toml" << 'EOF'
[package]
name = "project_a"
EOF

cat > "$TEST_DIR/projects/project_a/src/lib.rs" << 'EOF'
pub fn authenticate_user(username: &str, password: &str) -> bool {
    username == "admin" && password == "secret"
}

pub fn validate_token(token: &str) -> Option<String> {
    if token.starts_with("Bearer ") {
        Some(token[7..].to_string())
    } else {
        None
    }
}
EOF

cat > "$TEST_DIR/projects/project_b/Cargo.toml" << 'EOF'
[package]
name = "project_b"
EOF

cat > "$TEST_DIR/projects/project_b/src/lib.rs" << 'EOF'
pub fn hash_password(password: &str) -> String {
    format!("hashed:{}", password)
}

pub fn verify_password(password: &str, hash: &str) -> bool {
    hash == format!("hashed:{}", password)
}
EOF

cat > "$TEST_DIR/config/srag/config.toml" <<EOF
[mcp]
auto_index_cwd = true
EOF

"$SRAG_BIN" index "$TEST_DIR/projects/project_a" --name project_a 2>&1 | tail -1
"$SRAG_BIN" index "$TEST_DIR/projects/project_b" --name project_b 2>&1 | tail -1

output=$("$SRAG_BIN" status 2>&1)
if echo "$output" | grep -q "projects: 2"; then
    pass "multiple projects indexed"
else
    fail "multi-project indexing failed"
fi

echo ""
echo "--- test 12: cross-project search ---"
output=$("$SRAG_BIN" query -p project_a -q "password authentication" 2>&1 | head -20)
if echo "$output" | grep -qi "authenticate\|password"; then
    pass "cross-project search works for project_a"
else
    fail "cross-project search failed for project_a"
fi

output=$("$SRAG_BIN" query -p project_b -q "password hashing" 2>&1 | head -20)
if echo "$output" | grep -qi "hash\|password"; then
    pass "cross-project search works for project_b"
else
    fail "cross-project search failed for project_b"
fi

echo ""
echo "--- test 13: test is_project_directory unit tests ---"
cd "$PROJECT_ROOT"
output=$(cargo test cli::mcp::helpers --release 2>&1 | tail -5)
if echo "$output" | grep -q "passed"; then
    test_count=$(echo "$output" | grep -oP '\d+ passed' | head -1)
    pass "helper unit tests: $test_count"
else
    fail "helper unit tests failed"
fi

echo ""
echo "--- test 14: test all unit tests ---"
output=$(cargo test --release 2>&1 | tail -10)
if echo "$output" | grep -q "passed" && ! echo "$output" | grep -q "failed"; then
    test_count=$(echo "$output" | grep -oP '\d+ passed' | tail -1)
    pass "all unit tests: $test_count"
else
    fail "some unit tests failed"
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
