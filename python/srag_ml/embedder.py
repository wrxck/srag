# SPDX-License-Identifier: GPL-3.0
# Copyright (c) 2026 Matt Hesketh <matt@matthesketh.pro>

import logging
from typing import Optional

logger = logging.getLogger(__name__)


class Embedder:
    """wraps fastembed for text embedding using BAAI/bge-small-en-v1.5."""

    MODEL_NAME = "BAAI/bge-small-en-v1.5"
    DIMENSION = 384

    def __init__(self, cache_dir: Optional[str] = None):
        self._model = None
        self._cache_dir = cache_dir

    @property
    def is_loaded(self) -> bool:
        return self._model is not None

    def load(self):
        if self._model is not None:
            return

        logger.info("loading embedding model: %s", self.MODEL_NAME)
        from fastembed import TextEmbedding

        kwargs = {"model_name": self.MODEL_NAME}
        if self._cache_dir:
            kwargs["cache_dir"] = self._cache_dir

        self._model = TextEmbedding(**kwargs)
        logger.info("embedding model loaded")

    def embed(self, texts: list[str]) -> list[list[float]]:
        """embed a batch of texts, returns list of 384-dim vectors."""
        self.load()
        embeddings = list(self._model.embed(texts))
        return [e.tolist() for e in embeddings]

    def unload(self):
        self._model = None
        logger.info("embedding model unloaded")
