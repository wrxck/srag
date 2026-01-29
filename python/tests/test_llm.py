# SPDX-License-Identifier: GPL-3.0
# Copyright (c) 2026 Matt Hesketh <matt@matthesketh.pro>

import pytest
import time
from unittest.mock import MagicMock, patch


class TestLlmEngine:
    def test_initial_state(self):
        from srag_ml.llm import LlmEngine

        engine = LlmEngine()
        assert not engine.is_loaded
        assert engine._model is None
        assert engine._last_used == 0

    def test_is_loaded_property(self):
        from srag_ml.llm import LlmEngine

        engine = LlmEngine()
        assert not engine.is_loaded

        engine._model = MagicMock()
        assert engine.is_loaded

    def test_unload(self):
        from srag_ml.llm import LlmEngine

        engine = LlmEngine()
        engine._model = MagicMock()
        engine._model_path = "/path/to/model"
        assert engine.is_loaded

        engine.unload()
        assert not engine.is_loaded
        assert engine._model is None
        assert engine._model_path is None

    def test_custom_parameters(self):
        from srag_ml.llm import LlmEngine

        engine = LlmEngine(
            models_dir="/custom/models",
            model_filename="custom.gguf",
            model_url="https://example.com/model.gguf",
            n_threads=4,
            n_ctx=8192,
        )
        assert engine._models_dir == "/custom/models"
        assert engine._model_filename == "custom.gguf"
        assert engine._model_url == "https://example.com/model.gguf"
        assert engine._n_threads == 4
        assert engine._n_ctx == 8192

    def test_default_constants(self):
        from srag_ml.llm import LlmEngine

        assert LlmEngine.DEFAULT_MODEL == "Llama-3.2-1B-Instruct-Q4_K_M.gguf"
        assert "huggingface.co" in LlmEngine.DEFAULT_URL

    def test_idle_seconds_when_not_used(self):
        from srag_ml.llm import LlmEngine

        engine = LlmEngine()
        assert engine.idle_seconds == 0

    def test_idle_seconds_after_use(self):
        from srag_ml.llm import LlmEngine

        engine = LlmEngine()
        engine._last_used = time.time() - 10
        assert 9 < engine.idle_seconds < 12

    def test_memory_estimate_when_not_loaded(self):
        from srag_ml.llm import LlmEngine

        engine = LlmEngine()
        assert engine.memory_estimate_mb() is None

    def test_memory_estimate_when_loaded(self):
        from srag_ml.llm import LlmEngine

        engine = LlmEngine()
        engine._model = MagicMock()
        mem = engine.memory_estimate_mb()
        assert mem is not None
        assert mem > 0

    def test_n_threads_zero_becomes_none(self):
        from srag_ml.llm import LlmEngine

        engine = LlmEngine(n_threads=0)
        assert engine._n_threads is None

    def test_n_threads_positive_preserved(self):
        from srag_ml.llm import LlmEngine

        engine = LlmEngine(n_threads=8)
        assert engine._n_threads == 8

    @patch("llama_cpp.Llama")
    @patch("os.path.exists")
    def test_load_creates_model(self, mock_exists, mock_llama):
        from srag_ml.llm import LlmEngine

        mock_exists.return_value = True
        mock_model = MagicMock()
        mock_llama.return_value = mock_model

        engine = LlmEngine(models_dir="/models")
        engine.load()

        mock_llama.assert_called_once()
        assert engine.is_loaded
        assert engine._last_used > 0

    @patch("llama_cpp.Llama")
    @patch("os.path.exists")
    def test_load_idempotent(self, mock_exists, mock_llama):
        from srag_ml.llm import LlmEngine

        mock_exists.return_value = True
        engine = LlmEngine(models_dir="/models")
        engine.load()
        engine.load()

        mock_llama.assert_called_once()

    @patch("llama_cpp.Llama")
    @patch("os.path.exists")
    def test_load_updates_last_used(self, mock_exists, mock_llama):
        from srag_ml.llm import LlmEngine

        mock_exists.return_value = True
        engine = LlmEngine(models_dir="/models")
        engine._model = MagicMock()
        engine._last_used = 100

        engine.load()
        assert engine._last_used > 100

    def test_load_raises_without_path(self):
        from srag_ml.llm import LlmEngine

        engine = LlmEngine()
        with pytest.raises(RuntimeError, match="No model path"):
            engine.load()

    @patch("llama_cpp.Llama")
    @patch("os.path.exists")
    def test_generate_loads_model(self, mock_exists, mock_llama):
        from srag_ml.llm import LlmEngine

        mock_exists.return_value = True
        mock_model = MagicMock()
        mock_model.create_chat_completion.return_value = {
            "choices": [{"message": {"content": "response"}}],
            "usage": {"total_tokens": 50},
        }
        mock_llama.return_value = mock_model

        engine = LlmEngine(models_dir="/models")
        result = engine.generate("test prompt")

        assert engine.is_loaded
        assert result["text"] == "response"
        assert result["tokens_used"] == 50

    @patch("llama_cpp.Llama")
    @patch("os.path.exists")
    def test_generate_passes_parameters(self, mock_exists, mock_llama):
        from srag_ml.llm import LlmEngine

        mock_exists.return_value = True
        mock_model = MagicMock()
        mock_model.create_chat_completion.return_value = {
            "choices": [{"message": {"content": "test"}}],
        }
        mock_llama.return_value = mock_model

        engine = LlmEngine(models_dir="/models")
        engine.generate("prompt", max_tokens=100, temperature=0.5, stop=["END"])

        mock_model.create_chat_completion.assert_called_once()
        call_kwargs = mock_model.create_chat_completion.call_args[1]
        assert call_kwargs["max_tokens"] == 100
        assert call_kwargs["temperature"] == 0.5
        assert call_kwargs["stop"] == ["END"]

    @patch("llama_cpp.Llama")
    @patch("os.path.exists")
    def test_generate_updates_last_used(self, mock_exists, mock_llama):
        from srag_ml.llm import LlmEngine

        mock_exists.return_value = True
        mock_model = MagicMock()
        mock_model.create_chat_completion.return_value = {
            "choices": [{"message": {"content": "test"}}],
        }
        mock_llama.return_value = mock_model

        engine = LlmEngine(models_dir="/models")
        engine.load()
        old_time = engine._last_used

        time.sleep(0.01)
        engine.generate("prompt")

        assert engine._last_used > old_time
