# SPDX-License-Identifier: GPL-3.0
# Copyright (c) 2026 Matt Hesketh <matt@matthesketh.pro>

import pytest
from unittest.mock import MagicMock, patch


class TestEmbedder:
    def test_initial_state(self):
        from srag_ml.embedder import Embedder

        embedder = Embedder()
        assert not embedder.is_loaded
        assert embedder._model is None

    def test_is_loaded_property(self):
        from srag_ml.embedder import Embedder

        embedder = Embedder()
        assert not embedder.is_loaded

        embedder._model = MagicMock()
        assert embedder.is_loaded

    def test_unload(self):
        from srag_ml.embedder import Embedder

        embedder = Embedder()
        embedder._model = MagicMock()
        assert embedder.is_loaded

        embedder.unload()
        assert not embedder.is_loaded
        assert embedder._model is None

    def test_cache_dir_passed(self):
        from srag_ml.embedder import Embedder

        embedder = Embedder(cache_dir="/custom/cache")
        assert embedder._cache_dir == "/custom/cache"

    def test_model_name_constant(self):
        from srag_ml.embedder import Embedder

        assert Embedder.MODEL_NAME == "BAAI/bge-small-en-v1.5"

    def test_dimension_constant(self):
        from srag_ml.embedder import Embedder

        assert Embedder.DIMENSION == 384

    @patch("fastembed.TextEmbedding")
    def test_load_creates_model(self, mock_text_embedding):
        from srag_ml.embedder import Embedder

        mock_model = MagicMock()
        mock_text_embedding.return_value = mock_model

        embedder = Embedder()
        embedder.load()

        mock_text_embedding.assert_called_once_with(model_name="BAAI/bge-small-en-v1.5")
        assert embedder.is_loaded

    @patch("fastembed.TextEmbedding")
    def test_load_with_cache_dir(self, mock_text_embedding):
        from srag_ml.embedder import Embedder

        embedder = Embedder(cache_dir="/my/cache")
        embedder.load()

        mock_text_embedding.assert_called_once_with(
            model_name="BAAI/bge-small-en-v1.5", cache_dir="/my/cache"
        )

    @patch("fastembed.TextEmbedding")
    def test_load_idempotent(self, mock_text_embedding):
        from srag_ml.embedder import Embedder

        embedder = Embedder()
        embedder.load()
        embedder.load()

        mock_text_embedding.assert_called_once()

    @patch("fastembed.TextEmbedding")
    def test_embed_loads_model(self, mock_text_embedding):
        from srag_ml.embedder import Embedder
        import numpy as np

        mock_model = MagicMock()
        mock_model.embed.return_value = [np.array([0.1, 0.2, 0.3])]
        mock_text_embedding.return_value = mock_model

        embedder = Embedder()
        result = embedder.embed(["hello"])

        assert embedder.is_loaded
        assert len(result) == 1
        assert result[0] == [0.1, 0.2, 0.3]

    @patch("fastembed.TextEmbedding")
    def test_embed_batch(self, mock_text_embedding):
        from srag_ml.embedder import Embedder
        import numpy as np

        mock_model = MagicMock()
        mock_model.embed.return_value = [
            np.array([0.1, 0.2]),
            np.array([0.3, 0.4]),
        ]
        mock_text_embedding.return_value = mock_model

        embedder = Embedder()
        result = embedder.embed(["hello", "world"])

        assert len(result) == 2
        mock_model.embed.assert_called_once_with(["hello", "world"])

    @patch("fastembed.TextEmbedding")
    def test_embed_returns_list_of_lists(self, mock_text_embedding):
        from srag_ml.embedder import Embedder
        import numpy as np

        mock_model = MagicMock()
        mock_model.embed.return_value = [np.array([1.0, 2.0, 3.0])]
        mock_text_embedding.return_value = mock_model

        embedder = Embedder()
        result = embedder.embed(["test"])

        assert isinstance(result, list)
        assert isinstance(result[0], list)
        assert all(isinstance(x, float) for x in result[0])
