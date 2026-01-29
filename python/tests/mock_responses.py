# SPDX-License-Identifier: GPL-3.0
# Copyright (c) 2026 Matt Hesketh <matt@matthesketh.pro>

"""
Mock response classes matching official SDK structures.

Based on official documentation:
- Anthropic: https://docs.anthropic.com/en/api/messages
- OpenAI: https://platform.openai.com/docs/api-reference/chat
"""

from dataclasses import dataclass
from typing import Optional


@dataclass
class MockAnthropicTextBlock:
    """Matches anthropic.types.TextBlock"""

    type: str = "text"
    text: str = ""


@dataclass
class MockAnthropicUsage:
    """Matches anthropic.types.Usage"""

    input_tokens: int = 0
    output_tokens: int = 0
    cache_creation_input_tokens: int = 0
    cache_read_input_tokens: int = 0


@dataclass
class MockAnthropicMessage:
    """
    Matches anthropic.types.Message response format.

    Official format:
    {
      "id": "msg_013Zva2CMHLNPoW1VJYjXo6D",
      "type": "message",
      "role": "assistant",
      "content": [{"type": "text", "text": "..."}],
      "model": "claude-sonnet-4-20250514",
      "stop_reason": "end_turn",
      "stop_sequence": null,
      "usage": {"input_tokens": 10, "output_tokens": 15, ...}
    }
    """

    id: str = "msg_test123"
    type: str = "message"
    role: str = "assistant"
    content: list = None
    model: str = "claude-sonnet-4-20250514"
    stop_reason: str = "end_turn"
    stop_sequence: Optional[str] = None
    usage: MockAnthropicUsage = None

    def __post_init__(self):
        if self.content is None:
            self.content = [MockAnthropicTextBlock(text="Hello! How can I help?")]
        if self.usage is None:
            self.usage = MockAnthropicUsage(input_tokens=10, output_tokens=15)


@dataclass
class MockOpenAIMessage:
    """Matches openai.types.chat.ChatCompletionMessage"""

    role: str = "assistant"
    content: str = "Hello! How can I help?"
    function_call: Optional[dict] = None
    tool_calls: Optional[list] = None


@dataclass
class MockOpenAIChoice:
    """Matches openai.types.chat.Choice"""

    index: int = 0
    message: MockOpenAIMessage = None
    finish_reason: str = "stop"
    logprobs: Optional[dict] = None

    def __post_init__(self):
        if self.message is None:
            self.message = MockOpenAIMessage()


@dataclass
class MockOpenAIUsage:
    """Matches openai.types.CompletionUsage"""

    prompt_tokens: int = 10
    completion_tokens: int = 15
    total_tokens: int = 25


@dataclass
class MockOpenAIChatCompletion:
    """
    Matches openai.types.chat.ChatCompletion response format.

    Official format:
    {
      "id": "chatcmpl-abc123",
      "object": "chat.completion",
      "created": 1677858242,
      "model": "gpt-4o",
      "choices": [{"index": 0, "message": {...}, "finish_reason": "stop"}],
      "usage": {"prompt_tokens": 10, "completion_tokens": 15, "total_tokens": 25},
      "system_fingerprint": "fp_44709d6fcb"
    }
    """

    id: str = "chatcmpl-test123"
    object: str = "chat.completion"
    created: int = 1700000000
    model: str = "gpt-4o"
    choices: list = None
    usage: MockOpenAIUsage = None
    system_fingerprint: str = "fp_test123"

    def __post_init__(self):
        if self.choices is None:
            self.choices = [MockOpenAIChoice()]
        if self.usage is None:
            self.usage = MockOpenAIUsage()


# Error classes for testin, you guessed it, error scenarios


class MockAnthropicRateLimitError(Exception):
    """
    Matches anthropic.RateLimitError.

    Official error format:
    {
      "type": "error",
      "error": {"type": "rate_limit_error", "message": "..."}
    }
    """

    def __init__(self, message="Number of requests has exceeded your rate limit"):
        self.message = message
        self.status_code = 429
        self.body = {
            "type": "error",
            "error": {"type": "rate_limit_error", "message": message},
        }
        super().__init__(message)


class MockOpenAIRateLimitError(Exception):
    """
    Matches openai.RateLimitError.

    Official error format:
    {
      "error": {"message": "...", "type": "tokens", "code": "rate_limit_exceeded"}
    }
    """

    def __init__(self, message="Rate limit reached for gpt-4o"):
        self.message = message
        self.status_code = 429
        self.body = {
            "error": {
                "message": message,
                "type": "tokens",
                "param": None,
                "code": "rate_limit_exceeded",
            }
        }
        super().__init__(message)
