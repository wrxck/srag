# SPDX-License-Identifier: GPL-3.0
# Copyright (c) 2026 Matt Hesketh <matt@matthesketh.pro>

import logging
from typing import Optional

logger = logging.getLogger(__name__)


class Reranker:
    """wraps fastembed TextCrossEncoder for re-ranking search results."""

    MODEL_NAME = "Xenova/ms-marco-MiniLM-L-6-v2"

    def __init__(self, cache_dir: Optional[str] = None):
        self._model = None
        self._cache_dir = cache_dir

    @property
    def is_loaded(self) -> bool:
        return self._model is not None

    def load(self):
        if self._model is not None:
            return

        logger.info("loading reranker model: %s", self.MODEL_NAME)
        from fastembed.rerank.cross_encoder.text_cross_encoder import TextCrossEncoder

        kwargs = {"model_name": self.MODEL_NAME}
        if self._cache_dir:
            kwargs["cache_dir"] = self._cache_dir

        self._model = TextCrossEncoder(**kwargs)
        logger.info("reranker model loaded")

    def rerank(
        self, query: str, documents: list[str], top_k: int = 10
    ) -> list[tuple[int, float]]:
        """re-rank documents by relevance to query, returns [(index, score)]."""
        self.load()
        scores = list(self._model.rerank(query, documents))
        indexed = [(i, float(s)) for i, s in enumerate(scores)]
        indexed.sort(key=lambda x: x[1], reverse=True)
        return indexed[:top_k]

    def unload(self):
        self._model = None
        logger.info("reranker model unloaded")
