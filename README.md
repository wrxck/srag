# srag

Semantic code search across all your repositories. Index your projects once, then query them via MCP - your AI coding assistant gains instant access to every pattern, implementation, and convention you've ever written.

## Why?

AI coding assistants are great at writing code, but they don't know *your* code. They'll suggest a new auth flow when you already have one. They'll invent a logging pattern when yours is right there. They'll ask questions you've already answered in another project.

srag fixes this. Index your repositories, and your assistant can search across all of them semantically - finding relevant implementations, patterns, and conventions without you having to explain anything.

## Use Cases

**Reuse your own implementations**
> "Add OAuth token refresh" → Agent finds your existing implementation across projects, copies the pattern including the edge cases you already handled

**Consistent code style**
> Agent queries `get_project_patterns` before writing code, sees you use `snake_case` for functions and `PascalCase` for types, follows suit automatically

**Find that thing you wrote**
> "How did I handle rate limiting?" → Semantic search finds the relevant code even if you don't remember which project or what you called it

**Cross-project knowledge**
> Working on project A, need to integrate with an API you've used before in project B → Agent finds your previous integration, reuses the error handling and retry logic

**Debug with context**
> "Why is this failing?" → Agent searches for similar error handling patterns in your codebase, finds how you solved the same class of problem before

**Onboard yourself**
> Returning to an old project? Query the index to understand the architecture, find where things are defined, see the patterns that were used

**Code review context**
> "Is this how we usually do it?" → Search for similar implementations to check consistency with established patterns

**Find duplication**
> Pass a code snippet to `find_similar_code` → Discover near-duplicates across your codebase that could be refactored

The real value compounds over time. The more projects you index, the more patterns your assistant can draw from. The quality scales with the model - better models make better use of the retrieved context.

## Install

You'll need Rust and Python 3.10+ installed first.

<details>
<summary><strong>Linux</strong></summary>

```bash
./install.sh
```

This builds the binary to `~/.local/bin/srag` and sets up the Python ML backend.

If `~/.local/bin` isn't in your PATH:

```bash
echo 'export PATH="$HOME/.local/bin:$PATH"' >> ~/.bashrc && source ~/.bashrc
```

</details>

<details>
<summary><strong>macOS</strong></summary>

```bash
./install.sh
```

The script will ask for sudo to install the binary to `/usr/local/bin`. Data goes to `~/Library/Application Support/srag`.

If you don't have Rust/Python yet:

```bash
brew install rust python@3.12
```

</details>

<details>
<summary><strong>Windows</strong></summary>

Use WSL (Windows Subsystem for Linux) and follow the Linux instructions. Native Windows support isn't there yet, but WSL works fine.

</details>

## Usage

```bash
# index a project
srag index /path/to/repo

# re-index all projects (incremental, skips unchanged files)
srag sync

# start file watcher for auto-reindexing
srag watch

# interactive chat
srag chat

# one-shot query
srag query -p myproject -q "what was that authentication we implemented in {project_name}?"

# show index stats
srag status --detailed
```

## MCP Server

`srag mcp` starts an MCP server over stdio for integration with AI tools (Claude Code, Cursor, etc).

It does not produce any terminal output when run directly, it communicates via JSON-RPC on stdin/stdout.

There is built in integration with Claude Code, which will find your MCP configuration (or create one if not) and the next time you use it, you can ask your agent to check for MCP tools from srag - it should find them. This is all part of the install script.

Now this is where it becomes quite a powerful tool, because not only will your agents be able to find code quicker by querying the embeddings, but you'll likely use less tokens whilst doing so because the format is more convenient for an LLM.

I've also found that agents tend to write more consistent code when they can reference your existing patterns rather than inventing new ones each time.

### Available MCP Tools

| Tool | Description |
|------|-------------|
| `list_projects` | List all indexed projects with their paths |
| `search_code` | Semantic search using vector similarity |
| `find_similar_code` | Find code similar to a snippet |
| `search_symbols` | Search for functions, classes, or symbols by name pattern |
| `get_file` | Get file contents or specific line ranges |
| `get_project_patterns` | Analyse project conventions (naming, structure, languages) |
| `text_search` | Full-text keyword search for exact terms |
| `find_callers` | Find all functions that call a specific function |
| `find_callees` | Find all functions called by a specific function |

### Testing the MCP server

```bash
npx @modelcontextprotocol/inspector srag mcp
```

## Configuration

Config lives at `~/.config/srag/config.toml` on Linux or `~/Library/Application Support/srag/config.toml` on macOS. You can tweak things like which LLM provider to use, context sizes, and file ignore patterns. There's a `config.example.toml` in the repo if you want to see what's available.

For external LLM providers (Anthropic, OpenAI), just drop your API key in the config directory as `api_key.txt` or set the appropriate environment variable.

## How it works

The Rust CLI handles file discovery, tree-sitter based code chunking, and the SQLite + HNSW vector index. A Python sidecar process manages the ML bits - embeddings, reranking, and LLM inference.

When you query, it does hybrid search (vector similarity + full-text) with reciprocal rank fusion, then reranks the results before passing them to the LLM. The chunking is language-aware, so it extracts functions, classes, and other meaningful units rather than just splitting on line counts.

There's also prompt injection detection and secret redaction built in, so you're not accidentally leaking API keys into your queries.

## Uninstall

```bash
./uninstall.sh
```

This removes the binary, Python environment, and offers to clean up the Claude MCP config. It'll show you exactly what's being removed before doing anything.

## Development

```bash
./test.sh    # run all tests (rust + python)
./lint.sh    # run linters (clippy, rustfmt, ruff)
./format.sh  # auto-format code
```

## Contributing

Contributions are welcome - see [CONTRIBUTING.md](CONTRIBUTING.md) for details.

## Alternatives

There are several other semantic code search tools worth considering. Here's how srag compares:

| Feature | srag | [grepai](https://github.com/yoanbernabeu/grepai) | [CodeGrok](https://github.com/peterdemin/CodeGrok) | [DeepContext](https://github.com/DeepContext/deepcontext) |
|---------|------|--------|----------|-------------|
| **Search** |
| Semantic vector search | Yes | Yes | Yes | Yes |
| Full-text keyword search | Yes | Yes | No | Yes (BM25) |
| Hybrid search (vector + FTS) | Yes, RRF fusion | Yes | No | Yes |
| Reranking | Yes | No | No | Yes (Jina) |
| **Indexing** |
| AST-aware chunking | Yes, tree-sitter | Yes | Yes, tree-sitter | Yes, tree-sitter |
| Incremental (hash-based) | Yes, blake3 | Yes | Yes, mtime | Yes, SHA-256 |
| File watcher | Yes | Yes | No | No |
| Auto-index on first query | Yes | No | No | No |
| **Languages** | 9 (AST) + line-based | 12+ | 9 | 2 |
| **Security** |
| Prompt injection detection | Yes | No | No | No |
| Secret redaction | Yes | No | No | No |
| **Integration** |
| MCP server | Yes | Yes | Yes | Yes |
| Multi-project support | Yes | Yes, workspaces | No | No |
| Cross-project search | Yes | Yes | No | No |
| **Privacy** |
| Fully local option | Yes | Yes (Ollama) | Yes | No (needs Jina API) |
| **Other** |
| Pattern analysis | Yes | No | No | No |
| Similar code finder | Yes | No | No | No |
| Interactive chat | Yes | No | No | No |
| Call graph tracing | Yes | Yes | No | No |

### When to use what

**Choose srag if you:**
- Need security features (prompt injection detection, secret redaction)
- Want hybrid search with reranking for better accuracy
- Have multiple projects and want cross-project search
- Prefer auto-indexing without manual setup
- Want to analyse coding patterns across your codebase
- Need call graph tracing (find callers/callees of functions)

**Choose [grepai](https://github.com/yoanbernabeu/grepai) if you:**
- Want a single binary with no Python dependency
- Need the broadest language support (12+)
- Want pre-built AI agent skills

**Choose [CodeGrok](https://github.com/peterdemin/CodeGrok) if you:**
- Want GPU-accelerated embedding generation
- Need detailed symbol metadata (signatures, docstrings)
- Prefer a simpler, focused tool

**Choose [DeepContext](https://github.com/DeepContext/deepcontext) if you:**
- Want Jina's embedding models
- Need automatic test/generated file exclusion
- Are working primarily with TypeScript/Python

### Supported Languages

srag uses tree-sitter for AST-aware code chunking:

| Language | Extensions |
|----------|------------|
| Rust | `.rs` |
| Python | `.py` |
| JavaScript | `.js`, `.jsx`, `.mjs` |
| TypeScript | `.ts`, `.tsx` |
| Go | `.go` |
| C | `.c`, `.h` |
| C++ | `.cpp`, `.hpp`, `.cc`, `.cxx` |
| Java | `.java` |
| Ruby | `.rb` |

**Line-based chunking** is used for config and documentation files: Markdown, JSON, YAML, TOML, Shell scripts, SQL, HTML, CSS, and environment files.

## License

GPL-3.0 - see [LICENSE](LICENSE) for details.
