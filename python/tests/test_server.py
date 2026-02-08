# SPDX-License-Identifier: GPL-3.0
# Copyright (c) 2026 Matt Hesketh <matt@matthesketh.pro>

import json
import struct
import pytest
from unittest.mock import MagicMock, patch


class TestMlServer:
    @patch("srag_ml.server.Embedder")
    @patch("srag_ml.server.Reranker")
    @patch("srag_ml.server.LlmEngine")
    def test_init_local_provider(self, mock_llm, mock_reranker, mock_embedder):
        from srag_ml.server import MlServer

        server = MlServer(api_provider="local")
        assert server._api_client is None
        assert server._llm is not None

    @patch("srag_ml.server.Embedder")
    @patch("srag_ml.server.Reranker")
    @patch("srag_ml.server.ExternalApiClient")
    def test_init_external_provider(self, mock_api, mock_reranker, mock_embedder):
        from srag_ml.server import MlServer

        server = MlServer(api_provider="anthropic")
        assert server._api_client is not None
        assert server._llm is None

    @patch("srag_ml.server.Embedder")
    @patch("srag_ml.server.Reranker")
    @patch("srag_ml.server.LlmEngine")
    def test_handle_ping(self, mock_llm, mock_reranker, mock_embedder):
        from srag_ml.server import MlServer

        server = MlServer()
        result = server._handle_ping({})
        assert result == {"status": "ok"}

    @patch("srag_ml.server.Embedder")
    @patch("srag_ml.server.Reranker")
    @patch("srag_ml.server.LlmEngine")
    def test_handle_embed_success(self, mock_llm, mock_reranker, mock_embedder):
        from srag_ml.server import MlServer

        mock_embedder_instance = MagicMock()
        mock_embedder_instance.embed.return_value = [[0.1, 0.2, 0.3]]
        mock_embedder.return_value = mock_embedder_instance

        server = MlServer()
        result = server._handle_embed({"texts": ["hello"]})

        assert "vectors" in result
        mock_embedder_instance.embed.assert_called_once_with(["hello"])

    @patch("srag_ml.server.Embedder")
    @patch("srag_ml.server.Reranker")
    @patch("srag_ml.server.LlmEngine")
    def test_handle_embed_no_texts(self, mock_llm, mock_reranker, mock_embedder):
        from srag_ml.server import MlServer

        server = MlServer()
        with pytest.raises(ValueError, match="No texts"):
            server._handle_embed({})

    @patch("srag_ml.server.Embedder")
    @patch("srag_ml.server.Reranker")
    @patch("srag_ml.server.LlmEngine")
    def test_handle_embed_max_batch(self, mock_llm, mock_reranker, mock_embedder):
        from srag_ml.server import MlServer, MAX_EMBED_BATCH_SIZE

        server = MlServer()
        with pytest.raises(ValueError, match=f"maximum of {MAX_EMBED_BATCH_SIZE}"):
            server._handle_embed({"texts": ["t"] * 65})

    @patch("srag_ml.server.Embedder")
    @patch("srag_ml.server.Reranker")
    @patch("srag_ml.server.LlmEngine")
    def test_handle_generate_no_prompt(self, mock_llm, mock_reranker, mock_embedder):
        from srag_ml.server import MlServer

        server = MlServer()
        with pytest.raises(ValueError, match="No prompt"):
            server._handle_generate({})

    @patch("srag_ml.server.Embedder")
    @patch("srag_ml.server.Reranker")
    @patch("srag_ml.server.LlmEngine")
    def test_handle_generate_local(self, mock_llm, mock_reranker, mock_embedder):
        from srag_ml.server import MlServer

        mock_llm_instance = MagicMock()
        mock_llm_instance.generate.return_value = {
            "text": "response",
            "tokens_used": 10,
        }
        mock_llm.return_value = mock_llm_instance

        server = MlServer(api_provider="local")
        result = server._handle_generate({"prompt": "test"})

        assert result["text"] == "response"

    @patch("srag_ml.server.Embedder")
    @patch("srag_ml.server.Reranker")
    @patch("srag_ml.server.LlmEngine")
    def test_handle_rerank_no_query(self, mock_llm, mock_reranker, mock_embedder):
        from srag_ml.server import MlServer

        server = MlServer()
        with pytest.raises(ValueError, match="No query"):
            server._handle_rerank({"documents": ["doc"]})

    @patch("srag_ml.server.Embedder")
    @patch("srag_ml.server.Reranker")
    @patch("srag_ml.server.LlmEngine")
    def test_handle_rerank_no_documents(self, mock_llm, mock_reranker, mock_embedder):
        from srag_ml.server import MlServer

        server = MlServer()
        with pytest.raises(ValueError, match="No documents"):
            server._handle_rerank({"query": "test"})

    @patch("srag_ml.server.Embedder")
    @patch("srag_ml.server.Reranker")
    @patch("srag_ml.server.LlmEngine")
    def test_handle_rerank_success(self, mock_llm, mock_reranker, mock_embedder):
        from srag_ml.server import MlServer

        mock_reranker_instance = MagicMock()
        mock_reranker_instance.rerank.return_value = [(0, 0.9), (1, 0.5)]
        mock_reranker.return_value = mock_reranker_instance

        server = MlServer()
        result = server._handle_rerank({"query": "test", "documents": ["a", "b"]})

        assert "results" in result
        assert result["results"] == [(0, 0.9), (1, 0.5)]

    @patch("srag_ml.server.Embedder")
    @patch("srag_ml.server.Reranker")
    @patch("srag_ml.server.LlmEngine")
    def test_handle_model_status(self, mock_llm, mock_reranker, mock_embedder):
        from srag_ml.server import MlServer

        mock_embedder_instance = MagicMock()
        mock_embedder_instance.is_loaded = True
        mock_embedder.return_value = mock_embedder_instance

        mock_reranker_instance = MagicMock()
        mock_reranker_instance.is_loaded = False
        mock_reranker.return_value = mock_reranker_instance

        mock_llm_instance = MagicMock()
        mock_llm_instance.is_loaded = False
        mock_llm_instance.memory_estimate_mb.return_value = None
        mock_llm.return_value = mock_llm_instance

        server = MlServer()
        result = server._handle_model_status({})

        assert result["embedder_loaded"] is True
        assert result["llm_loaded"] is False
        assert result["reranker_loaded"] is False

    @patch("srag_ml.server.Embedder")
    @patch("srag_ml.server.Reranker")
    @patch("srag_ml.server.LlmEngine")
    def test_handle_shutdown(self, mock_llm, mock_reranker, mock_embedder):
        from srag_ml.server import MlServer

        server = MlServer()
        server._running = True
        result = server._handle_shutdown({})

        assert result["status"] == "shutting_down"
        assert server._running is False

    @patch("srag_ml.server.Embedder")
    @patch("srag_ml.server.Reranker")
    @patch("srag_ml.server.LlmEngine")
    def test_dispatch_unknown_method(self, mock_llm, mock_reranker, mock_embedder):
        from srag_ml.server import MlServer

        server = MlServer()
        with pytest.raises(ValueError, match="Unknown method"):
            server._dispatch("nonexistent_method", {})

    @patch("srag_ml.server.Embedder")
    @patch("srag_ml.server.Reranker")
    @patch("srag_ml.server.LlmEngine")
    def test_dispatch_routes_correctly(self, mock_llm, mock_reranker, mock_embedder):
        from srag_ml.server import MlServer

        server = MlServer()

        result = server._dispatch("ping", {})
        assert result == {"status": "ok"}


class TestMessageFraming:
    @patch("srag_ml.server.Embedder")
    @patch("srag_ml.server.Reranker")
    @patch("srag_ml.server.LlmEngine")
    def test_send_message_format(self, mock_llm, mock_reranker, mock_embedder):
        from srag_ml.server import MlServer

        server = MlServer()
        mock_conn = MagicMock()

        data = b"test message"
        server._send_message(mock_conn, data)

        mock_conn.sendall.assert_called_once()
        sent_data = mock_conn.sendall.call_args[0][0]
        length = struct.unpack(">I", sent_data[:4])[0]
        assert length == len(data)
        assert sent_data[4:] == data

    @patch("srag_ml.server.Embedder")
    @patch("srag_ml.server.Reranker")
    @patch("srag_ml.server.LlmEngine")
    def test_recv_message_oversized(self, mock_llm, mock_reranker, mock_embedder):
        from srag_ml.server import MlServer

        server = MlServer()
        mock_conn = MagicMock()

        oversized_length = 11 * 1024 * 1024
        mock_conn.recv.return_value = struct.pack(">I", oversized_length)

        def recv_exact(conn, n):
            return conn.recv(n)

        server._recv_exact = recv_exact

        with pytest.raises(ValueError, match="Message too large"):
            server._recv_message(mock_conn)


class TestAuthToken:
    @patch("srag_ml.server.Embedder")
    @patch("srag_ml.server.Reranker")
    @patch("srag_ml.server.LlmEngine")
    def test_auth_token_stored(self, mock_llm, mock_reranker, mock_embedder):
        from srag_ml.server import MlServer

        server = MlServer(auth_token="secret123")
        assert server._auth_token == "secret123"

    @patch("srag_ml.server.Embedder")
    @patch("srag_ml.server.Reranker")
    @patch("srag_ml.server.LlmEngine")
    def test_auth_token_none_by_default(self, mock_llm, mock_reranker, mock_embedder):
        from srag_ml.server import MlServer

        server = MlServer()
        assert server._auth_token is None

    @patch("srag_ml.server.Embedder")
    @patch("srag_ml.server.Reranker")
    @patch("srag_ml.server.LlmEngine")
    def test_auth_uses_constant_time_comparison(
        self, mock_llm, mock_reranker, mock_embedder
    ):
        import hmac

        from srag_ml.server import MlServer

        server = MlServer(auth_token="correct_token")
        assert hmac.compare_digest("correct_token", server._auth_token)
        assert not hmac.compare_digest("wrong_token", server._auth_token)


class TestLoadUnloadModel:
    @patch("srag_ml.server.Embedder")
    @patch("srag_ml.server.Reranker")
    @patch("srag_ml.server.LlmEngine")
    def test_load_embedder(self, mock_llm, mock_reranker, mock_embedder):
        from srag_ml.server import MlServer

        mock_embedder_instance = MagicMock()
        mock_embedder.return_value = mock_embedder_instance

        server = MlServer()
        result = server._handle_load_model({"type": "embedder"})

        assert result["status"] == "loaded"
        mock_embedder_instance.load.assert_called_once()

    @patch("srag_ml.server.Embedder")
    @patch("srag_ml.server.Reranker")
    @patch("srag_ml.server.LlmEngine")
    def test_load_llm(self, mock_llm, mock_reranker, mock_embedder):
        from srag_ml.server import MlServer

        mock_llm_instance = MagicMock()
        mock_llm.return_value = mock_llm_instance

        server = MlServer(api_provider="local")
        result = server._handle_load_model({"type": "llm"})

        assert result["status"] == "loaded"
        mock_llm_instance.load.assert_called_once()

    @patch("srag_ml.server.Embedder")
    @patch("srag_ml.server.Reranker")
    @patch("srag_ml.server.ExternalApiClient")
    def test_load_llm_external_api_error(self, mock_api, mock_reranker, mock_embedder):
        from srag_ml.server import MlServer

        server = MlServer(api_provider="anthropic")
        with pytest.raises(ValueError, match="Local LLM not available"):
            server._handle_load_model({"type": "llm"})

    @patch("srag_ml.server.Embedder")
    @patch("srag_ml.server.Reranker")
    @patch("srag_ml.server.LlmEngine")
    def test_load_unknown_type(self, mock_llm, mock_reranker, mock_embedder):
        from srag_ml.server import MlServer

        server = MlServer()
        with pytest.raises(ValueError, match="Unknown model type"):
            server._handle_load_model({"type": "unknown"})

    @patch("srag_ml.server.Embedder")
    @patch("srag_ml.server.Reranker")
    @patch("srag_ml.server.LlmEngine")
    def test_unload_embedder(self, mock_llm, mock_reranker, mock_embedder):
        from srag_ml.server import MlServer

        mock_embedder_instance = MagicMock()
        mock_embedder.return_value = mock_embedder_instance

        server = MlServer()
        result = server._handle_unload_model({"type": "embedder"})

        assert result["status"] == "unloaded"
        mock_embedder_instance.unload.assert_called_once()

    @patch("srag_ml.server.Embedder")
    @patch("srag_ml.server.Reranker")
    @patch("srag_ml.server.LlmEngine")
    def test_unload_llm(self, mock_llm, mock_reranker, mock_embedder):
        from srag_ml.server import MlServer

        mock_llm_instance = MagicMock()
        mock_llm.return_value = mock_llm_instance

        server = MlServer(api_provider="local")
        result = server._handle_unload_model({"type": "llm"})

        assert result["status"] == "unloaded"
        mock_llm_instance.unload.assert_called_once()
