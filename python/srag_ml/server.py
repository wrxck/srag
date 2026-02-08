# SPDX-License-Identifier: GPL-3.0
# Copyright (c) 2026 Matt Hesketh <matt@matthesketh.pro>

import hmac
import json
import logging
import os
import socket
import struct
import threading
import time
from pathlib import Path
from typing import Optional

from .embedder import Embedder
from .llm import LlmEngine
from .models import (
    get_models_dir,
    download_model,
    DEFAULT_MODEL_FILENAME,
    HUGGINGFACE_MODEL_URL,
)
from .reranker import Reranker
from .api_client import ExternalApiClient

# maximum number of texts allowed in a single embedding batch request
MAX_EMBED_BATCH_SIZE = 64

logger = logging.getLogger(__name__)
logging.basicConfig(level=logging.INFO, format="%(asctime)s [%(name)s] %(message)s")


class MlServer:
    """JSON-RPC server over TCP for ML operations."""

    def __init__(
        self,
        host: str = "127.0.0.1",
        port: int = 0,
        port_file: Optional[str] = None,
        models_dir: Optional[str] = None,
        auth_token: Optional[str] = None,
        model_filename: Optional[str] = None,
        model_url: Optional[str] = None,
        llm_threads: int = 0,
        llm_context_size: int = 4096,
        api_provider: str = "local",
        api_model: str = "claude-sonnet-4-20250514",
        api_max_tokens: int = 2048,
        redact_secrets: bool = True,
        api_key_file: Optional[str] = None,
    ):
        self._host = host
        self._port = port
        self._port_file = port_file
        self._auth_token = auth_token
        self._models_dir = get_models_dir(models_dir)
        self._model_filename = model_filename or DEFAULT_MODEL_FILENAME
        self._model_url = model_url or HUGGINGFACE_MODEL_URL
        self._api_provider = api_provider
        self._redact_secrets = redact_secrets

        self._embedder = Embedder(cache_dir=str(self._models_dir))
        self._reranker = Reranker(cache_dir=str(self._models_dir))

        if api_provider == "local":
            self._llm = LlmEngine(
                models_dir=str(self._models_dir),
                model_filename=self._model_filename,
                model_url=self._model_url,
                n_threads=llm_threads,
                n_ctx=llm_context_size,
            )
            self._api_client = None
            logger.info("using local LLM: %s", self._model_filename)
        else:
            self._llm = None
            self._api_client = ExternalApiClient(
                provider=api_provider,
                model=api_model,
                api_key_file=api_key_file,
                max_tokens=api_max_tokens,
                redact_secrets=redact_secrets,
            )
            logger.info("using external API: %s (%s)", api_provider, api_model)

        self._server_socket: Optional[socket.socket] = None
        self._running = False
        self._idle_check_interval = 30
        self._llm_idle_timeout = 300
        self._lock = threading.Lock()
        self._request_id = 0

    def run(self):
        self._server_socket = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
        self._server_socket.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
        self._server_socket.bind((self._host, self._port))
        self._server_socket.listen(5)
        self._server_socket.settimeout(1.0)
        self._running = True

        assigned_port = self._server_socket.getsockname()[1]

        if self._port_file:
            port_path = Path(self._port_file)
            port_path.parent.mkdir(parents=True, exist_ok=True)
            port_path.write_text(str(assigned_port))

        logger.info("ML service listening on %s:%d", self._host, assigned_port)

        idle_thread = threading.Thread(target=self._idle_monitor, daemon=True)
        idle_thread.start()

        while self._running:
            try:
                conn, _ = self._server_socket.accept()
                threading.Thread(
                    target=self._handle_connection, args=(conn,), daemon=True
                ).start()
            except socket.timeout:
                continue
            except OSError:
                if self._running:
                    raise
                break

    def shutdown(self):
        logger.info("shutting down ML service")
        self._running = False
        if self._server_socket:
            self._server_socket.close()
        if self._port_file and os.path.exists(self._port_file):
            os.unlink(self._port_file)
        self._embedder.unload()
        if self._llm:
            self._llm.unload()

    def _handle_connection(self, conn: socket.socket):
        conn.settimeout(30.0)  # 30 second timeout on client connections
        try:
            while self._running:
                data = self._recv_message(conn)
                if data is None:
                    break

                try:
                    request = json.loads(data)
                except json.JSONDecodeError as e:
                    self._send_error(conn, -32700, f"parse error: {e}", 0)
                    continue

                req_id = request.get("id", 0)
                method = request.get("method", "")
                params = request.get("params", {})

                if self._auth_token:
                    provided = request.get("_auth", "")
                    if not hmac.compare_digest(provided, self._auth_token):
                        self._send_error(conn, -32600, "auth token mismatch", req_id)
                        continue

                try:
                    result = self._dispatch(method, params)
                    self._send_result(conn, result, req_id)
                except Exception as e:
                    logger.exception("error handling method %s", method)
                    self._send_error(conn, -32603, str(e), req_id)
        except (ConnectionResetError, BrokenPipeError):
            pass
        finally:
            conn.close()

    def _dispatch(self, method: str, params: dict) -> dict:
        handlers = {
            "ping": self._handle_ping,
            "embed": self._handle_embed,
            "generate": self._handle_generate,
            "rerank": self._handle_rerank,
            "load_model": self._handle_load_model,
            "unload_model": self._handle_unload_model,
            "model_status": self._handle_model_status,
            "shutdown": self._handle_shutdown,
        }

        handler = handlers.get(method)
        if handler is None:
            raise ValueError(f"Unknown method: {method}")

        return handler(params)

    def _handle_ping(self, params: dict) -> dict:
        return {"status": "ok"}

    def _handle_embed(self, params: dict) -> dict:
        with self._lock:
            texts = params.get("texts", [])
            if not texts:
                raise ValueError("No texts provided")
            if len(texts) > MAX_EMBED_BATCH_SIZE:
                raise ValueError(
                    f"Batch size {len(texts)} exceeds maximum of {MAX_EMBED_BATCH_SIZE} texts per request"
                )

            vectors = self._embedder.embed(texts)
            return {"vectors": vectors}

    def _handle_generate(self, params: dict) -> dict:
        with self._lock:
            prompt = params.get("prompt", "")
            if not prompt:
                raise ValueError("No prompt provided")

            max_tokens = params.get("max_tokens", 1024)
            if not isinstance(max_tokens, (int, float)) or max_tokens < 1 or max_tokens > 32768:
                max_tokens = 1024
            temperature = params.get("temperature", 0.1)
            if not isinstance(temperature, (int, float)) or temperature < 0.0 or temperature > 2.0:
                temperature = 0.1
            stop = params.get("stop", None)

            if self._api_client is not None:
                result = self._api_client.generate(
                    prompt=prompt,
                    max_tokens=max_tokens,
                    temperature=temperature,
                    stop=stop,
                )
            else:
                result = self._llm.generate(
                    prompt=prompt,
                    max_tokens=max_tokens,
                    temperature=temperature,
                    stop=stop,
                )
            return result

    def _handle_rerank(self, params: dict) -> dict:
        with self._lock:
            query = params.get("query", "")
            documents = params.get("documents", [])
            top_k = params.get("top_k", 10)

            if not query:
                raise ValueError("No query provided")
            if not documents:
                raise ValueError("No documents provided")

            results = self._reranker.rerank(query, documents, top_k)
            return {"results": results}

    def _handle_load_model(self, params: dict) -> dict:
        with self._lock:
            model_type = params.get("type", "embedder")
            if model_type == "embedder":
                self._embedder.load()
            elif model_type == "llm":
                if self._llm is None:
                    raise ValueError("Local LLM not available - using external API")
                model_path = params.get("path")
                self._llm.load(model_path)
            else:
                raise ValueError(f"Unknown model type: {model_type}")
            return {"status": "loaded"}

    def _handle_unload_model(self, params: dict) -> dict:
        with self._lock:
            model_type = params.get("type", "llm")
            if model_type == "embedder":
                self._embedder.unload()
            elif model_type == "llm":
                if self._llm:
                    self._llm.unload()
            return {"status": "unloaded"}

    def _handle_model_status(self, params: dict) -> dict:
        llm_loaded = self._llm.is_loaded if self._llm else False
        llm_memory = self._llm.memory_estimate_mb() if self._llm else None

        return {
            "embedder_loaded": self._embedder.is_loaded,
            "llm_loaded": llm_loaded,
            "reranker_loaded": self._reranker.is_loaded,
            "embedder_memory_mb": 90.0 if self._embedder.is_loaded else None,
            "process_memory_mb": llm_memory,
            "reranker_memory_mb": 100.0 if self._reranker.is_loaded else None,
            "api_provider": self._api_provider,
            "api_redactions": (
                self._api_client.total_redactions if self._api_client else 0
            ),
        }

    def _handle_shutdown(self, params: dict) -> dict:
        self._running = False
        return {"status": "shutting_down"}

    def _idle_monitor(self):
        """periodically check if the LLM should be unloaded due to inactivity."""
        while self._running:
            time.sleep(self._idle_check_interval)
            if self._llm is None:
                continue
            if self._llm.is_loaded and self._llm.idle_seconds > self._llm_idle_timeout:
                logger.info("LLM idle for %.0fs, unloading", self._llm.idle_seconds)
                self._llm.unload()

    def _recv_message(self, conn: socket.socket) -> Optional[bytes]:
        """read a length-prefixed message (4-byte big-endian u32 + payload)."""
        header = self._recv_exact(conn, 4)
        if header is None:
            return None
        length = struct.unpack(">I", header)[0]
        if length > 10 * 1024 * 1024:
            raise ValueError(f"Message too large: {length}")
        return self._recv_exact(conn, length)

    def _recv_exact(self, conn: socket.socket, n: int) -> Optional[bytes]:
        data = bytearray()
        while len(data) < n:
            chunk = conn.recv(n - len(data))
            if not chunk:
                return None
            data.extend(chunk)
        return bytes(data)

    def _send_message(self, conn: socket.socket, data: bytes):
        """send a length-prefixed message."""
        header = struct.pack(">I", len(data))
        conn.sendall(header + data)

    def _send_result(self, conn: socket.socket, result: dict, req_id: int):
        response = {
            "jsonrpc": "2.0",
            "result": result,
            "id": req_id,
        }
        self._send_message(conn, json.dumps(response).encode())

    def _send_error(self, conn: socket.socket, code: int, message: str, req_id: int):
        response = {
            "jsonrpc": "2.0",
            "error": {"code": code, "message": message},
            "id": req_id,
        }
        self._send_message(conn, json.dumps(response).encode())
