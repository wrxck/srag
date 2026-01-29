# Contributing to srag

Thanks for your interest in contributing. Here's what you need to know.

## Getting Started

1. Fork the repo and clone it locally
2. Make sure you have Rust (1.75+) and Python (3.10+) installed
3. Run `./install.sh` to build and set up the project
4. Run `cargo test --workspace` to make sure everything works

## Making Changes

- Create a branch for your work
- Keep commits focused - one logical change per commit
- Write tests for new functionality
- Run `./lint.sh` and `./format.sh` before committing
- For Python changes, run `pytest` in the `python/` directory

## Code Style

**Rust:**
- Follow standard Rust conventions
- Run `cargo fmt` before committing
- No clippy warnings in new code

**Python:**
- Follow PEP 8
- Type hints are appreciated but not required
- Keep it simple

## Pull Requests

- Keep PRs focused on a single change
- Describe what you changed and why
- Link any relevant issues
- Make sure CI passes

## What to Work On

Check the issues for things that need doing. If you want to work on something that doesn't have an issue, open one first so we can discuss.

Native Windows support (without WSL) would be particularly welcome.

## Questions?

Open an issue if you're stuck or unsure about something.
