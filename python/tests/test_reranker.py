# SPDX-License-Identifier: GPL-3.0
# Copyright (c) 2026 Matt Hesketh <matt@matthesketh.pro>

import pytest
from unittest.mock import MagicMock, patch


class TestReranker:
    def test_initial_state(self):
        from srag_ml.reranker import Reranker

        reranker = Reranker()
        assert not reranker.is_loaded
        assert reranker._model is None

    def test_is_loaded_property(self):
        from srag_ml.reranker import Reranker

        reranker = Reranker()
        assert not reranker.is_loaded

        reranker._model = MagicMock()
        assert reranker.is_loaded

    def test_unload(self):
        from srag_ml.reranker import Reranker

        reranker = Reranker()
        reranker._model = MagicMock()
        assert reranker.is_loaded

        reranker.unload()
        assert not reranker.is_loaded
        assert reranker._model is None

    def test_cache_dir_passed(self):
        from srag_ml.reranker import Reranker

        reranker = Reranker(cache_dir="/custom/cache")
        assert reranker._cache_dir == "/custom/cache"

    def test_model_name_constant(self):
        from srag_ml.reranker import Reranker

        assert Reranker.MODEL_NAME == "Xenova/ms-marco-MiniLM-L-6-v2"

    @patch("fastembed.rerank.cross_encoder.text_cross_encoder.TextCrossEncoder")
    def test_load_creates_model(self, mock_encoder):
        from srag_ml.reranker import Reranker

        mock_model = MagicMock()
        mock_encoder.return_value = mock_model

        reranker = Reranker()
        reranker.load()

        mock_encoder.assert_called_once_with(model_name="Xenova/ms-marco-MiniLM-L-6-v2")
        assert reranker.is_loaded

    @patch("fastembed.rerank.cross_encoder.text_cross_encoder.TextCrossEncoder")
    def test_load_with_cache_dir(self, mock_encoder):
        from srag_ml.reranker import Reranker

        reranker = Reranker(cache_dir="/my/cache")
        reranker.load()

        mock_encoder.assert_called_once_with(
            model_name="Xenova/ms-marco-MiniLM-L-6-v2", cache_dir="/my/cache"
        )

    @patch("fastembed.rerank.cross_encoder.text_cross_encoder.TextCrossEncoder")
    def test_load_idempotent(self, mock_encoder):
        from srag_ml.reranker import Reranker

        reranker = Reranker()
        reranker.load()
        reranker.load()

        mock_encoder.assert_called_once()

    @patch("fastembed.rerank.cross_encoder.text_cross_encoder.TextCrossEncoder")
    def test_rerank_loads_model(self, mock_encoder):
        from srag_ml.reranker import Reranker

        mock_model = MagicMock()
        mock_model.rerank.return_value = [0.9, 0.5, 0.3]
        mock_encoder.return_value = mock_model

        reranker = Reranker()
        result = reranker.rerank("query", ["doc1", "doc2", "doc3"])

        assert reranker.is_loaded
        mock_model.rerank.assert_called_once_with("query", ["doc1", "doc2", "doc3"])

    @patch("fastembed.rerank.cross_encoder.text_cross_encoder.TextCrossEncoder")
    def test_rerank_returns_sorted_tuples(self, mock_encoder):
        from srag_ml.reranker import Reranker

        mock_model = MagicMock()
        mock_model.rerank.return_value = [0.3, 0.9, 0.5]
        mock_encoder.return_value = mock_model

        reranker = Reranker()
        result = reranker.rerank("query", ["doc1", "doc2", "doc3"])

        assert len(result) == 3
        assert result[0][0] == 1
        assert result[0][1] == 0.9
        assert result[1][0] == 2
        assert result[1][1] == 0.5
        assert result[2][0] == 0
        assert result[2][1] == 0.3

    @patch("fastembed.rerank.cross_encoder.text_cross_encoder.TextCrossEncoder")
    def test_rerank_respects_top_k(self, mock_encoder):
        from srag_ml.reranker import Reranker

        mock_model = MagicMock()
        mock_model.rerank.return_value = [0.1, 0.2, 0.3, 0.4, 0.5]
        mock_encoder.return_value = mock_model

        reranker = Reranker()
        result = reranker.rerank("query", ["a", "b", "c", "d", "e"], top_k=2)

        assert len(result) == 2

    @patch("fastembed.rerank.cross_encoder.text_cross_encoder.TextCrossEncoder")
    def test_rerank_single_document(self, mock_encoder):
        from srag_ml.reranker import Reranker

        mock_model = MagicMock()
        mock_model.rerank.return_value = [0.8]
        mock_encoder.return_value = mock_model

        reranker = Reranker()
        result = reranker.rerank("query", ["single doc"])

        assert len(result) == 1
        assert result[0] == (0, 0.8)

    @patch("fastembed.rerank.cross_encoder.text_cross_encoder.TextCrossEncoder")
    def test_rerank_returns_float_scores(self, mock_encoder):
        from srag_ml.reranker import Reranker

        mock_model = MagicMock()
        mock_model.rerank.return_value = [0.5]
        mock_encoder.return_value = mock_model

        reranker = Reranker()
        result = reranker.rerank("q", ["d"])

        assert isinstance(result[0][1], float)
