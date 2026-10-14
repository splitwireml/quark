import os
from pathlib import Path

import pytest

PENDING = Path(__file__).with_name("rust_pending.txt")


def pytest_configure(config):
    config.addinivalue_line(
        "markers", "python_only: needs the Python backend's internals; skipped when QUARK_TEST_BACKEND=rust"
    )
    config.addinivalue_line(
        "markers", "rust_only: needs the Rust backend; skipped unless QUARK_TEST_BACKEND=rust"
    )


def pending_names():
    if not PENDING.exists():
        return set()
    lines = (line.split("#", 1)[0].strip() for line in PENDING.read_text().splitlines())
    return {line for line in lines if line}


def pytest_collection_modifyitems(config, items):
    if os.environ.get("QUARK_TEST_BACKEND") != "rust":
        skip = pytest.mark.skip(reason="rust_only: needs QUARK_TEST_BACKEND=rust")
        for item in items:
            if "rust_only" in item.keywords:
                item.add_marker(skip)
        return
    skip = pytest.mark.skip(reason="python_only: needs the Python backend")
    pending = pending_names()
    xfail = pytest.mark.xfail(strict=True, reason="not ported to Rust yet")
    for item in items:
        if "python_only" in item.keywords:
            item.add_marker(skip)
        elif item.originalname in pending:
            item.add_marker(xfail)
