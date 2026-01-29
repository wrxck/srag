# SPDX-License-Identifier: GPL-3.0
# Copyright (c) 2026 Matt Hesketh <matt@matthesketh.pro>

"""
Tests for external API client with mocks matching official API response formats.

Mock responses are based on official documentation:
- Anthropic: https://docs.anthropic.com/en/api/messages
- OpenAI: https://platform.openai.com/docs/api-reference/chat
"""

import pytest
from unittest.mock import MagicMock, patch

from srag_ml.api_client import ExternalApiClient
from .mock_responses import (
    MockAnthropicMessage,
    MockAnthropicTextBlock,
    MockAnthropicUsage,
    MockOpenAIChatCompletion,
    MockOpenAIChoice,
    MockOpenAIMessage,
    MockOpenAIUsage,
)


class TestAnthropicClient:
    """Tests for Anthropic API integration"""

    @pytest.fixture
    def mock_anthropic_module(self):
        """Create mock Anthropic module"""
        mock_module = MagicMock()
        mock_client = MagicMock()
        mock_module.Anthropic.return_value = mock_client
        return mock_module, mock_client

    def test_successful_generation(self, mock_anthropic_module):
        """Test successful message generation with proper response format"""
        mock_module, mock_client = mock_anthropic_module

        mock_response = MockAnthropicMessage(
            id="msg_01XFDUDYJgAACzvnptvVoYEL",
            content=[MockAnthropicTextBlock(text="The answer is 42.")],
            model="claude-sonnet-4-20250514",
            stop_reason="end_turn",
            usage=MockAnthropicUsage(input_tokens=25, output_tokens=10),
        )
        mock_client.messages.create.return_value = mock_response

        with patch.dict("sys.modules", {"anthropic": mock_module}):
            client = ExternalApiClient(
                provider="anthropic",
                model="claude-sonnet-4-20250514",
                api_key="sk-ant-test123",
                redact_secrets=False,
            )
            client._client = mock_client

            result = client.generate("What is the meaning of life?")

            assert result["text"] == "The answer is 42."
            assert result["tokens_used"] == 35

            mock_client.messages.create.assert_called_once()
            call_kwargs = mock_client.messages.create.call_args[1]
            assert call_kwargs["model"] == "claude-sonnet-4-20250514"
            assert call_kwargs["messages"][0]["role"] == "user"

    def test_max_tokens_stop_reason(self, mock_anthropic_module):
        """Test response when max_tokens is reached"""
        mock_module, mock_client = mock_anthropic_module

        mock_response = MockAnthropicMessage(
            stop_reason="max_tokens",
            content=[MockAnthropicTextBlock(text="This response was truncat")],
            usage=MockAnthropicUsage(input_tokens=10, output_tokens=100),
        )
        mock_client.messages.create.return_value = mock_response

        with patch.dict("sys.modules", {"anthropic": mock_module}):
            client = ExternalApiClient(
                provider="anthropic",
                model="claude-sonnet-4-20250514",
                api_key="sk-ant-test123",
                redact_secrets=False,
            )
            client._client = mock_client

            result = client.generate("Tell me a long story", max_tokens=100)
            assert result["text"] == "This response was truncat"
            assert result["tokens_used"] == 110

    def test_empty_content_response(self, mock_anthropic_module):
        """Test handling of empty content array"""
        mock_module, mock_client = mock_anthropic_module

        mock_response = MockAnthropicMessage(content=[])
        mock_client.messages.create.return_value = mock_response

        with patch.dict("sys.modules", {"anthropic": mock_module}):
            client = ExternalApiClient(
                provider="anthropic",
                model="claude-sonnet-4-20250514",
                api_key="sk-ant-test123",
                redact_secrets=False,
            )
            client._client = mock_client

            result = client.generate("Hello")
            assert result["text"] == ""

    def test_secret_redaction_in_prompt(self, mock_anthropic_module):
        """Test that secrets are redacted before sending to API"""
        mock_module, mock_client = mock_anthropic_module

        mock_response = MockAnthropicMessage(
            content=[
                MockAnthropicTextBlock(text="I see you have a database connection.")
            ]
        )
        mock_client.messages.create.return_value = mock_response

        with patch.dict("sys.modules", {"anthropic": mock_module}):
            client = ExternalApiClient(
                provider="anthropic",
                model="claude-sonnet-4-20250514",
                api_key="sk-ant-test123",
                redact_secrets=True,
            )
            client._client = mock_client

            prompt_with_secret = (
                "Connect to postgres://admin:secretpass123@db.example.com/mydb"
            )
            client.generate(prompt_with_secret)

            call_kwargs = mock_client.messages.create.call_args[1]
            sent_prompt = call_kwargs["messages"][0]["content"]

            assert "secretpass123" not in sent_prompt
            assert "[REDACTED]" in sent_prompt
            assert client.total_redactions == 1


class TestOpenAIClient:
    """Tests for OpenAI API integration"""

    @pytest.fixture
    def mock_openai_module(self):
        """Create mock OpenAI module"""
        mock_module = MagicMock()
        mock_client = MagicMock()
        mock_module.OpenAI.return_value = mock_client
        return mock_module, mock_client

    def test_successful_generation(self, mock_openai_module):
        """Test successful chat completion with proper response format"""
        mock_module, mock_client = mock_openai_module

        mock_response = MockOpenAIChatCompletion(
            id="chatcmpl-abc123xyz",
            model="gpt-4o",
            choices=[
                MockOpenAIChoice(
                    message=MockOpenAIMessage(content="Hello! I'm here to help."),
                    finish_reason="stop",
                )
            ],
            usage=MockOpenAIUsage(
                prompt_tokens=15, completion_tokens=10, total_tokens=25
            ),
        )
        mock_client.chat.completions.create.return_value = mock_response

        with patch.dict("sys.modules", {"openai": mock_module}):
            client = ExternalApiClient(
                provider="openai",
                model="gpt-4o",
                api_key="sk-test123",
                redact_secrets=False,
            )
            client._client = mock_client

            result = client.generate("Hello!")

            assert result["text"] == "Hello! I'm here to help."
            assert result["tokens_used"] == 25

    def test_length_finish_reason(self, mock_openai_module):
        """Test response when length limit is reached"""
        mock_module, mock_client = mock_openai_module

        mock_response = MockOpenAIChatCompletion(
            choices=[
                MockOpenAIChoice(
                    message=MockOpenAIMessage(content="This is a truncated resp"),
                    finish_reason="length",
                )
            ],
            usage=MockOpenAIUsage(total_tokens=150),
        )
        mock_client.chat.completions.create.return_value = mock_response

        with patch.dict("sys.modules", {"openai": mock_module}):
            client = ExternalApiClient(
                provider="openai",
                model="gpt-4o",
                api_key="sk-test123",
                redact_secrets=False,
            )
            client._client = mock_client

            result = client.generate("Write a long essay", max_tokens=100)
            assert result["text"] == "This is a truncated resp"

    def test_empty_choices_response(self, mock_openai_module):
        """Test handling of empty choices array"""
        mock_module, mock_client = mock_openai_module

        mock_response = MockOpenAIChatCompletion(choices=[])
        mock_client.chat.completions.create.return_value = mock_response

        with patch.dict("sys.modules", {"openai": mock_module}):
            client = ExternalApiClient(
                provider="openai",
                model="gpt-4o",
                api_key="sk-test123",
                redact_secrets=False,
            )
            client._client = mock_client

            result = client.generate("Hello")
            assert result["text"] == ""

    def test_secret_redaction_multiple_secrets(self, mock_openai_module):
        """Test redaction of multiple different secret types"""
        mock_module, mock_client = mock_openai_module

        mock_response = MockOpenAIChatCompletion(
            choices=[
                MockOpenAIChoice(message=MockOpenAIMessage(content="I found secrets."))
            ]
        )
        mock_client.chat.completions.create.return_value = mock_response

        with patch.dict("sys.modules", {"openai": mock_module}):
            client = ExternalApiClient(
                provider="openai",
                model="gpt-4o",
                api_key="sk-test123",
                redact_secrets=True,
            )
            client._client = mock_client

            prompt = """
            AWS_ACCESS_KEY_ID=AKIAIOSFODNN7EXAMPLE
            github_token: ghp_ABCDEFGHIJKLMNOPQRSTUVWXYZabcdef1234
            DATABASE_URL=postgres://user:password@localhost/db
            """
            client.generate(prompt)

            call_kwargs = mock_client.chat.completions.create.call_args[1]
            sent_prompt = call_kwargs["messages"][0]["content"]

            assert "AKIAIOSFODNN7EXAMPLE" not in sent_prompt
            assert "password" not in sent_prompt
            assert client.total_redactions >= 2


class TestErrorHandling:
    """Tests for API error handling"""

    def test_missing_api_key_raises_error(self):
        """Test that missing API key raises appropriate error"""
        client = ExternalApiClient(
            provider="anthropic",
            model="claude-sonnet-4-20250514",
            api_key=None,
            redact_secrets=False,
        )

        with pytest.raises(RuntimeError, match="no API key configured"):
            client._ensure_client()

    def test_invalid_provider_raises_error(self):
        """Test that invalid provider raises appropriate error"""
        client = ExternalApiClient(
            provider="invalid_provider",
            model="some-model",
            api_key="test-key",
            redact_secrets=False,
        )

        with pytest.raises(ValueError, match="Unknown provider"):
            client._ensure_client()

    def test_api_key_from_file(self, tmp_path):
        """Test reading API key from file"""
        key_file = tmp_path / "api.key"
        key_file.write_text("sk-ant-test-key-from-file")

        client = ExternalApiClient(
            provider="anthropic",
            model="claude-sonnet-4-20250514",
            api_key_file=str(key_file),
            redact_secrets=False,
        )

        assert client._api_key == "sk-ant-test-key-from-file"

    def test_missing_api_key_file_handled(self):
        """Test that missing API key file is handled gracefully"""
        client = ExternalApiClient(
            provider="anthropic",
            model="claude-sonnet-4-20250514",
            api_key_file="/nonexistent/path/api.key",
            redact_secrets=False,
        )

        assert client._api_key is None
