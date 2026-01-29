# SPDX-License-Identifier: GPL-3.0
# Copyright (c) 2026 Matt Hesketh <matt@matthesketh.pro>

import logging
import os
import sys
import time
from pathlib import Path
from typing import Optional

logger = logging.getLogger(__name__)


class LlmEngine:
    """wraps llama-cpp-python for local LLM inference."""

    DEFAULT_MODEL = "Llama-3.2-1B-Instruct-Q4_K_M.gguf"
    DEFAULT_URL = "https://huggingface.co/bartowski/Llama-3.2-1B-Instruct-GGUF/resolve/main/Llama-3.2-1B-Instruct-Q4_K_M.gguf"

    def __init__(
        self,
        models_dir: Optional[str] = None,
        model_filename: Optional[str] = None,
        model_url: Optional[str] = None,
        n_threads: int = 0,
        n_ctx: int = 4096,
    ):
        self._model = None
        self._models_dir = models_dir
        self._model_filename = model_filename or self.DEFAULT_MODEL
        self._model_url = model_url or self.DEFAULT_URL
        self._n_threads = n_threads if n_threads > 0 else None
        self._n_ctx = n_ctx
        self._last_used: float = 0
        self._model_path: Optional[str] = None

    @property
    def is_loaded(self) -> bool:
        return self._model is not None

    def load(self, model_path: Optional[str] = None):
        if self._model is not None:
            self._last_used = time.time()
            return

        if model_path is None:
            if self._models_dir:
                model_path = os.path.join(self._models_dir, self._model_filename)
            else:
                logger.error("no model path provided and no models directory set")
                raise RuntimeError("No model path available")

        if not os.path.exists(model_path):
            logger.info("model not found, attempting download: %s", model_path)
            self._download_model(model_path)

        logger.info(
            "loading LLM model: %s (threads=%s, ctx=%d)",
            model_path,
            self._n_threads or "auto",
            self._n_ctx,
        )
        from llama_cpp import Llama

        self._model = Llama(
            model_path=model_path,
            n_ctx=self._n_ctx,
            n_threads=self._n_threads,
            verbose=False,
        )
        self._model_path = model_path
        self._last_used = time.time()
        logger.info("LLM model loaded")

    def generate(
        self,
        prompt: str,
        max_tokens: int = 1024,
        temperature: float = 0.1,
        stop: Optional[list[str]] = None,
    ) -> dict:
        """generate text from a prompt."""
        if self._model is None:
            self.load()

        self._last_used = time.time()

        result = self._model.create_chat_completion(
            messages=[{"role": "user", "content": prompt}],
            max_tokens=max_tokens,
            temperature=temperature,
            stop=stop,
        )

        text = result["choices"][0]["message"]["content"]
        tokens_used = result.get("usage", {}).get("total_tokens", 0)

        return {"text": text, "tokens_used": tokens_used}

    def unload(self):
        self._model = None
        self._model_path = None
        logger.info("LLM model unloaded")

    @property
    def idle_seconds(self) -> float:
        if self._last_used == 0:
            return 0
        return time.time() - self._last_used

    def memory_estimate_mb(self) -> Optional[float]:
        if not self.is_loaded:
            return None
        # approximate based on model size
        return 1500.0

    def _download_model(self, dest_path: str):
        """download the model file with progress indication."""
        import urllib.request

        dest = Path(dest_path)
        dest.parent.mkdir(parents=True, exist_ok=True)
        tmp_dest = dest.with_suffix(".download")

        logger.info("downloading model from %s", self._model_url)
        print(f"\ndownloading LLM model: {self._model_filename}", file=sys.stderr)
        print(f"source: {self._model_url}", file=sys.stderr)

        def _progress(block_num, block_size, total_size):
            downloaded = block_num * block_size
            if total_size > 0:
                pct = min(100, downloaded * 100 // total_size)
                mb_done = downloaded / (1024 * 1024)
                mb_total = total_size / (1024 * 1024)
                bar_width = 30
                filled = int(bar_width * pct / 100)
                bar = "=" * filled + "-" * (bar_width - filled)
                print(
                    f"\r  [{bar}] {mb_done:.0f}/{mb_total:.0f} MB ({pct}%)",
                    end="",
                    file=sys.stderr,
                    flush=True,
                )

        try:
            urllib.request.urlretrieve(
                self._model_url, str(tmp_dest), reporthook=_progress
            )
            print(file=sys.stderr)  # newline after progress
            tmp_dest.rename(dest)
            logger.info("model downloaded: %s", dest)
            print(f"model downloaded: {dest}", file=sys.stderr)
        except Exception as e:
            if tmp_dest.exists():
                tmp_dest.unlink()
            logger.error("Failed to download model: %s", e)
            raise RuntimeError(f"Failed to download model: {e}")
