# SPDX-License-Identifier: GPL-3.0
# Copyright (c) 2026 Matt Hesketh <matt@matthesketh.pro>

"""
external API client for Anthropic and OpenAI.

handles API calls with secret redaction for safety.
"""

import logging
import os
from typing import Optional

from .secrets import redact_secrets

logger = logging.getLogger(__name__)


class ExternalApiClient:
    """client for external LLM APIs with secret redaction."""

    def __init__(
        self,
        provider: str,
        model: str,
        api_key: Optional[str] = None,
        api_key_file: Optional[str] = None,
        max_tokens: int = 2048,
        redact_secrets: bool = True,
    ):
        self._provider = provider
        self._model = model
        self._max_tokens = max_tokens
        self._redact_secrets = redact_secrets
        self._api_key = api_key
        self._client = None
        self._total_redactions = 0

        if not self._api_key and api_key_file:
            self._api_key = self._read_key_file(api_key_file)

    def _read_key_file(self, path: str) -> Optional[str]:
        if not path or not os.path.exists(path):
            return None
        try:
            with open(path, "r") as f:
                return f.read().strip()
        except Exception as e:
            logger.warning("failed to read API key file: %s", e)
            return None

    def _ensure_client(self):
        if self._client is not None:
            return

        if not self._api_key:
            raise RuntimeError(
                f"no API key configured for {self._provider}. "
                f"run 'srag config api-key' to set one."
            )

        if self._provider == "anthropic":
            try:
                import anthropic

                self._client = anthropic.Anthropic(api_key=self._api_key)
            except ImportError:
                raise RuntimeError(
                    "anthropic package not installed. " "run: pip install anthropic"
                )
        elif self._provider == "openai":
            try:
                import openai

                self._client = openai.OpenAI(api_key=self._api_key)
            except ImportError:
                raise RuntimeError(
                    "openai package not installed. " "run: pip install openai"
                )
        else:
            raise ValueError(f"Unknown provider: {self._provider}")

        logger.info("initialized %s client with model %s", self._provider, self._model)

    def generate(
        self,
        prompt: str,
        max_tokens: Optional[int] = None,
        temperature: float = 0.1,
        stop: Optional[list[str]] = None,
    ) -> dict:
        """generate text using external API with secret redaction."""
        self._ensure_client()

        safe_prompt = prompt
        redaction_count = 0

        if self._redact_secrets:
            safe_prompt, redaction_count = redact_secrets(prompt)
            if redaction_count > 0:
                self._total_redactions += redaction_count
                logger.info(
                    "redacted %d secrets from prompt (total: %d)",
                    redaction_count,
                    self._total_redactions,
                )

        tokens = max_tokens or self._max_tokens

        if self._provider == "anthropic":
            return self._generate_anthropic(safe_prompt, tokens, temperature)
        elif self._provider == "openai":
            return self._generate_openai(safe_prompt, tokens, temperature, stop)
        else:
            raise ValueError(f"Unknown provider: {self._provider}")

    def _generate_anthropic(
        self,
        prompt: str,
        max_tokens: int,
        temperature: float,
    ) -> dict:
        response = self._client.messages.create(
            model=self._model,
            max_tokens=max_tokens,
            temperature=temperature,
            messages=[{"role": "user", "content": prompt}],
        )

        text = response.content[0].text if response.content else ""
        tokens_used = response.usage.input_tokens + response.usage.output_tokens

        return {"text": text, "tokens_used": tokens_used}

    def _generate_openai(
        self,
        prompt: str,
        max_tokens: int,
        temperature: float,
        stop: Optional[list[str]] = None,
    ) -> dict:
        response = self._client.chat.completions.create(
            model=self._model,
            max_tokens=max_tokens,
            temperature=temperature,
            stop=stop,
            messages=[{"role": "user", "content": prompt}],
        )

        text = response.choices[0].message.content if response.choices else ""
        tokens_used = response.usage.total_tokens if response.usage else 0

        return {"text": text, "tokens_used": tokens_used}

    @property
    def total_redactions(self) -> int:
        return self._total_redactions
