# SPDX-License-Identifier: GPL-3.0
# Copyright (c) 2026 Matt Hesketh <matt@matthesketh.pro>

import hashlib
import logging
import os
from pathlib import Path
from typing import Optional

logger = logging.getLogger(__name__)

HUGGINGFACE_MODEL_URL = (
    "https://huggingface.co/bartowski/Llama-3.2-1B-Instruct-GGUF"
    "/resolve/main/Llama-3.2-1B-Instruct-Q4_K_M.gguf"
)
DEFAULT_MODEL_FILENAME = "Llama-3.2-1B-Instruct-Q4_K_M.gguf"
EXPECTED_SHA256 = "6f85a640a97cf2bf5b8e764087b1e83da0fdb51d7c9fab7d0fece9385611df83"


def get_models_dir(override_dir: Optional[str] = None) -> Path:
    if override_dir:
        return Path(override_dir)

    data_dir = os.environ.get("XDG_DATA_HOME", os.path.expanduser("~/.local/share"))
    return Path(data_dir) / "srag" / "models"


def model_exists(models_dir: Path, filename: str = DEFAULT_MODEL_FILENAME) -> bool:
    return (models_dir / filename).exists()


def _sha256_file(path: Path) -> str:
    h = hashlib.sha256()
    with open(path, "rb") as f:
        while True:
            chunk = f.read(1024 * 1024)
            if not chunk:
                break
            h.update(chunk)
    return h.hexdigest()


def download_model(
    models_dir: Path,
    filename: str = DEFAULT_MODEL_FILENAME,
    url: str = HUGGINGFACE_MODEL_URL,
    expected_sha256: str = EXPECTED_SHA256,
) -> Path:
    """download the GGUF model file if not already present."""
    models_dir.mkdir(parents=True, exist_ok=True)
    dest = models_dir / filename

    if dest.exists():
        logger.info("model already exists: %s", dest)
        return dest

    logger.info("downloading model from %s", url)
    logger.info("destination: %s", dest)

    import urllib.request

    tmp_dest = dest.with_suffix(".download")

    def _progress(block_num, block_size, total_size):
        downloaded = block_num * block_size
        if total_size > 0:
            pct = min(100, downloaded * 100 // total_size)
            mb_done = downloaded / (1024 * 1024)
            mb_total = total_size / (1024 * 1024)
            print(f"\r  {mb_done:.0f}/{mb_total:.0f} MB ({pct}%)", end="", flush=True)

    urllib.request.urlretrieve(url, str(tmp_dest), reporthook=_progress)
    print()

    if expected_sha256:
        logger.info("verifying model checksum...")
        actual = _sha256_file(tmp_dest)
        if actual != expected_sha256:
            tmp_dest.unlink()
            raise RuntimeError(
                f"model checksum mismatch: expected {expected_sha256}, "
                f"got {actual}. the download may be corrupted."
            )
        logger.info("checksum verified: %s", actual)

    tmp_dest.rename(dest)
    logger.info("model downloaded: %s", dest)
    return dest
