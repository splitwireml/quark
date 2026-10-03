"""Test client factory: the Python app by default, the Rust dev server when QUARK_TEST_BACKEND=rust."""
import os
import queue
import re
import subprocess
import sys
import threading
import warnings
from contextlib import contextmanager
from pathlib import Path

import httpx

warnings.filterwarnings("ignore", message="Using `httpx` with `starlette.testclient` is deprecated.*")
from fastapi.testclient import TestClient

from backend.app import create_app

ROOT = Path(__file__).resolve().parent.parent
LISTENING = re.compile(r"QUARK_LISTENING[ =:]+(?:https?://)?(\S+)")


def make_client(path, raise_server_exceptions=True):
    """Context manager yielding a client for a fresh backend rooted at `path`.

    `raise_server_exceptions` only applies to the Python app; a Rust server always answers 500.
    """
    if os.environ.get("QUARK_TEST_BACKEND") == "rust":
        return _rust_client(Path(path))
    return TestClient(create_app(path), raise_server_exceptions=raise_server_exceptions)


@contextmanager
def _rust_client(path):
    binary = os.environ.get("QUARK_DEV_SERVER") or str(
        ROOT / "target" / "debug" / ("quark-dev-server.exe" if sys.platform == "win32" else "quark-dev-server")
    )
    server = subprocess.Popen(
        [binary, "--data-dir", str(path), "--cache-dir", str(path / ".quark-cache"), "--port", "0"],
        stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True,
    )
    seen, ready = [], queue.Queue()

    def drain():
        for line in server.stdout:
            match = LISTENING.match(line)
            if match:
                ready.put(match.group(1))
                break
            seen.append(line)
        else:
            ready.put(None)
        for _ in server.stdout:  # keep reading so the server never blocks on a full pipe
            pass

    reader = threading.Thread(target=drain, daemon=True)
    reader.start()
    try:
        try:
            addr = ready.get(timeout=30)
        except queue.Empty:
            addr = None
        if addr is None:
            raise RuntimeError("quark-dev-server never printed QUARK_LISTENING (30s timeout or early exit):\n" + "".join(seen))
        with httpx.Client(base_url=f"http://{addr}", timeout=60) as client:
            yield client
    finally:
        server.terminate()
        try:
            server.wait(5)
        except subprocess.TimeoutExpired:
            server.kill()
            server.wait()
        reader.join(1)
        server.stdout.close()
