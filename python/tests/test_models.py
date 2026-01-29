# SPDX-License-Identifier: GPL-3.0
# Copyright (c) 2026 Matt Hesketh <matt@matthesketh.pro>

import os
import pytest
import tempfile
from pathlib import Path
from unittest.mock import patch


class TestModels:
    def test_get_models_dir_default(self):
        from srag_ml.models import get_models_dir

        with patch.dict(os.environ, {}, clear=True):
            with patch.dict(os.environ, {"HOME": "/home/testuser"}):
                result = get_models_dir()
                assert "srag" in str(result)
                assert "models" in str(result)

    def test_get_models_dir_with_override(self):
        from srag_ml.models import get_models_dir

        result = get_models_dir("/custom/path")
        assert result == Path("/custom/path")

    def test_get_models_dir_respects_xdg_data_home(self):
        from srag_ml.models import get_models_dir

        with patch.dict(os.environ, {"XDG_DATA_HOME": "/xdg/data"}):
            result = get_models_dir()
            assert str(result).startswith("/xdg/data")

    def test_model_exists_true(self):
        from srag_ml.models import model_exists

        with tempfile.TemporaryDirectory() as tmpdir:
            models_dir = Path(tmpdir)
            model_file = models_dir / "test.gguf"
            model_file.touch()
            assert model_exists(models_dir, "test.gguf")

    def test_model_exists_false(self):
        from srag_ml.models import model_exists

        with tempfile.TemporaryDirectory() as tmpdir:
            models_dir = Path(tmpdir)
            assert not model_exists(models_dir, "nonexistent.gguf")

    def test_model_exists_default_filename(self):
        from srag_ml.models import model_exists, DEFAULT_MODEL_FILENAME

        with tempfile.TemporaryDirectory() as tmpdir:
            models_dir = Path(tmpdir)
            model_file = models_dir / DEFAULT_MODEL_FILENAME
            model_file.touch()
            assert model_exists(models_dir)

    def test_sha256_file(self):
        from srag_ml.models import _sha256_file

        with tempfile.TemporaryDirectory() as tmpdir:
            test_file = Path(tmpdir) / "test.txt"
            test_file.write_text("hello world")

            result = _sha256_file(test_file)

            assert len(result) == 64
            assert all(c in "0123456789abcdef" for c in result)
            assert (
                result
                == "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9"
            )

    def test_sha256_file_empty(self):
        from srag_ml.models import _sha256_file

        with tempfile.TemporaryDirectory() as tmpdir:
            test_file = Path(tmpdir) / "empty.txt"
            test_file.write_bytes(b"")

            result = _sha256_file(test_file)

            assert (
                result
                == "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
            )

    def test_sha256_file_binary(self):
        from srag_ml.models import _sha256_file

        with tempfile.TemporaryDirectory() as tmpdir:
            test_file = Path(tmpdir) / "binary.bin"
            test_file.write_bytes(bytes(range(256)))

            result = _sha256_file(test_file)

            assert len(result) == 64

    def test_default_model_filename_constant(self):
        from srag_ml.models import DEFAULT_MODEL_FILENAME

        assert DEFAULT_MODEL_FILENAME == "Llama-3.2-1B-Instruct-Q4_K_M.gguf"

    def test_huggingface_url_constant(self):
        from srag_ml.models import HUGGINGFACE_MODEL_URL

        assert "huggingface.co" in HUGGINGFACE_MODEL_URL
        assert "Llama-3.2-1B-Instruct" in HUGGINGFACE_MODEL_URL

    def test_expected_sha256_format(self):
        from srag_ml.models import EXPECTED_SHA256

        assert len(EXPECTED_SHA256) == 64
        assert all(c in "0123456789abcdef" for c in EXPECTED_SHA256)

    def test_download_model_skips_existing(self):
        from srag_ml.models import download_model

        with tempfile.TemporaryDirectory() as tmpdir:
            models_dir = Path(tmpdir)
            model_file = models_dir / "existing.gguf"
            model_file.write_text("existing model")

            result = download_model(models_dir, "existing.gguf")

            assert result == model_file

    def test_download_model_creates_directory(self):
        from srag_ml.models import download_model

        with tempfile.TemporaryDirectory() as tmpdir:
            models_dir = Path(tmpdir) / "nested" / "models"
            model_file = models_dir / "test.gguf"
            model_file.parent.mkdir(parents=True, exist_ok=True)
            model_file.write_text("test")

            result = download_model(models_dir, "test.gguf")
            assert models_dir.exists()
