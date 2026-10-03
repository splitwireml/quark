import os

import pytest


def pytest_configure(config):
    config.addinivalue_line(
        "markers", "python_only: needs the Python backend's internals; skipped when QUARK_TEST_BACKEND=rust"
    )


def pytest_collection_modifyitems(config, items):
    if os.environ.get("QUARK_TEST_BACKEND") != "rust":
        return
    skip = pytest.mark.skip(reason="python_only: needs the Python backend")
    for item in items:
        if "python_only" in item.keywords:
            item.add_marker(skip)
