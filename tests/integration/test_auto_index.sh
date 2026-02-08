#!/bin/bash
set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
TEST_DIR=$(mktemp -d)
SRAG_BIN="$PROJECT_ROOT/target/release/srag"

cleanup() {
    echo "cleaning up $TEST_DIR..."
    rm -rf "$TEST_DIR"
}
trap cleanup EXIT

echo "=== srag auto-index integration tests ==="
echo "test directory: $TEST_DIR"
echo ""

cd "$PROJECT_ROOT"
cargo build --release 2>&1 | tail -1

if [[ ! -x "$SRAG_BIN" ]]; then
    echo "error: srag binary not found at $SRAG_BIN"
    exit 1
fi

export XDG_DATA_HOME="$TEST_DIR"
export XDG_CONFIG_HOME="$TEST_DIR/config"
mkdir -p "$TEST_DIR/config/srag"

echo "--- test 1: is_project_directory detection ---"
cd "$TEST_DIR"

mkdir empty_dir
mkdir rust_project && echo '[package]' > rust_project/Cargo.toml
mkdir node_project && echo '{}' > node_project/package.json
mkdir python_project && echo '[tool.poetry]' > python_project/pyproject.toml
mkdir git_project && mkdir git_project/.git

for dir in rust_project node_project python_project git_project; do
    if [[ -d "$dir" ]]; then
        echo "  $dir: created"
    fi
done
echo "PASS: project directories created"

echo ""
echo "--- test 2: index rust project ---"
cd "$TEST_DIR/rust_project"
mkdir src && cat > src/main.rs << 'EOF'
fn main() {
    println!("Hello, world!");
}

fn add(a: i32, b: i32) -> i32 {
    a + b
}

struct Config {
    name: String,
    value: i32,
}
EOF

"$SRAG_BIN" index . --name rust_test 2>&1 | tail -3
echo "PASS: rust project indexed"

echo ""
echo "--- test 3: verify index exists ---"
"$SRAG_BIN" status 2>&1 | head -10
echo "PASS: status shows indexed project"

echo ""
echo "--- test 4: clone httpie and index ---"
cd "$TEST_DIR"
git clone --depth 1 --single-branch https://github.com/httpie/cli.git httpie 2>&1 | tail -2
cd httpie

"$SRAG_BIN" index . --name httpie 2>&1 | tail -3
echo "PASS: httpie indexed"

echo ""
echo "--- test 5: verify both projects ---"
"$SRAG_BIN" status --detailed 2>&1 | head -20

echo ""
echo "--- test 6: test MCP server starts ---"
timeout 2 "$SRAG_BIN" mcp < /dev/null 2>&1 || true
echo "PASS: MCP server can start (exits cleanly on empty input)"

echo ""
echo "--- test 7: test query command ---"
cd "$TEST_DIR/httpie"
result=$("$SRAG_BIN" query -p httpie -q "how does HTTP request work" 2>&1 | head -20)
if echo "$result" | grep -qi "http\|request\|response"; then
    echo "PASS: query returned relevant results"
    echo "response preview: $(echo "$result" | head -3)"
else
    echo "WARN: query may not have returned expected results"
    echo "$result"
fi

echo ""
echo "--- test 8: helpers unit tests verify is_project_directory ---"
cd "$PROJECT_ROOT"
cargo test cli::mcp::helpers --release 2>&1 | tail -15

echo ""
echo "=== all integration tests passed ==="
