# SPDX-License-Identifier: GPL-3.0
# Copyright (c) 2026 Matt Hesketh <matt@matthesketh.pro>

import argparse
import sys

from .server import MlServer


def main():
    parser = argparse.ArgumentParser(description="srag ML service")
    parser.add_argument(
        "--host",
        default="127.0.0.1",
        help="Host to bind to (default: 127.0.0.1)",
    )
    parser.add_argument(
        "--port",
        type=int,
        default=0,
        help="Port to bind to (default: 0 for OS-assigned)",
    )
    parser.add_argument(
        "--port-file",
        default=None,
        help="File to write the assigned port to",
    )
    parser.add_argument(
        "--models-dir",
        default=None,
        help="Directory for model files",
    )
    parser.add_argument(
        "--auth-token",
        default=None,
        help="Auth token for request validation",
    )
    parser.add_argument(
        "--model-filename",
        default=None,
        help="LLM model filename (default: Llama-3.2-1B-Instruct-Q4_K_M.gguf)",
    )
    parser.add_argument(
        "--model-url",
        default=None,
        help="URL to download model from if not present",
    )
    parser.add_argument(
        "--llm-threads",
        type=int,
        default=0,
        help="Number of threads for LLM inference (0 = auto-detect)",
    )
    parser.add_argument(
        "--llm-context-size",
        type=int,
        default=4096,
        help="LLM context window size",
    )
    parser.add_argument(
        "--api-provider",
        default="local",
        choices=["local", "anthropic", "openai"],
        help="LLM Provider: local, anthropic, or openai",
    )
    parser.add_argument(
        "--api-model",
        default="claude-sonnet-4-20250514",
        help="Model name for external API",
    )
    parser.add_argument(
        "--api-max-tokens",
        type=int,
        default=2048,
        help="Max tokens for external API response",
    )
    parser.add_argument(
        "--redact-secrets",
        default="true",
        help="Whether to redact secrets before sending to external API",
    )
    parser.add_argument(
        "--api-key-file",
        default=None,
        help="Path to file containing API key",
    )
    args = parser.parse_args()

    server = MlServer(
        host=args.host,
        port=args.port,
        port_file=args.port_file,
        models_dir=args.models_dir,
        auth_token=args.auth_token,
        model_filename=args.model_filename,
        model_url=args.model_url,
        llm_threads=args.llm_threads,
        llm_context_size=args.llm_context_size,
        api_provider=args.api_provider,
        api_model=args.api_model,
        api_max_tokens=args.api_max_tokens,
        redact_secrets=args.redact_secrets.lower() == "true",
        api_key_file=args.api_key_file,
    )
    try:
        server.run()
    except KeyboardInterrupt:
        server.shutdown()
        sys.exit(0)


if __name__ == "__main__":
    main()
